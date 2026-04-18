use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, Json},
    routing::{delete, get, post, put},
    Router,
};
use matrix_sdk::{
    config::SyncSettings,
    room::{Room, RoomMemberships},
    ruma::{
        events::room::message::{RoomMessageEventContent, OriginalSyncRoomMessageEvent},
        room_id,
    },
    Client,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::{cors::CorsLayer, services::ServeDir, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Clone)]
struct AppState {
    version: String,
    vm_configs: Arc<RwLock<Vec<VmConfig>>>,
    task_queues: Arc<RwLock<Vec<TaskQueue>>>,
    matrix_client: Option<Client>,
    matrix_room_id: Option<String>,
    room_members: Arc<RwLock<Vec<MatrixUser>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MatrixUser {
    user_id: String,
    display_name: Option<String>,
    avatar_url: Option<String>,
    is_bot: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct VmConfig {
    id: String,
    name: String,
    agent_id: String,
    enabled: bool,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TaskQueue {
    id: String,
    vm_id: String,
    name: String,
    tasks: Vec<Task>,
    loop_enabled: bool,
    enabled: bool,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Task {
    id: String,
    description: String,
    keystrokes: Option<String>,
    delay_ms: Option<u64>,
    completed: bool,
}

#[derive(Serialize)]
struct HealthResponse {
    status: String,
    version: String,
    matrix_connected: bool,
    matrix_room_id: Option<String>,
    room_members_count: usize,
}

#[derive(Deserialize)]
struct CreateVmConfig {
    name: String,
    agent_id: String,
}

#[derive(Deserialize)]
struct UpdateVmConfig {
    name: Option<String>,
    agent_id: Option<String>,
    enabled: Option<bool>,
}

#[derive(Deserialize)]
struct CreateTaskQueue {
    vm_id: String,
    name: String,
    tasks: Option<Vec<CreateTask>>,
    loop_enabled: Option<bool>,
}

#[derive(Deserialize)]
struct CreateTask {
    description: String,
    keystrokes: Option<String>,
    delay_ms: Option<u64>,
}

#[derive(Deserialize)]
struct UpdateTaskQueue {
    name: Option<String>,
    tasks: Option<Vec<CreateTask>>,
    loop_enabled: Option<bool>,
    enabled: Option<bool>,
}

#[derive(Clone)]
struct MatrixState {
    room_members: Arc<RwLock<Vec<MatrixUser>>>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "npc_ui_backend=debug,tower_http=debug,matrix_sdk=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load Matrix credentials from environment
    let matrix_homeserver = std::env::var("MATRIX_HOMESERVER").ok();
    let matrix_user = std::env::var("MATRIX_USER").ok();
    let matrix_password = std::env::var("MATRIX_PASSWORD").ok();
    let matrix_room_id = std::env::var("MATRIX_ROOM_ID").ok();

    let (matrix_client, matrix_room_id_opt, room_members) = if let (Some(homeserver), Some(user), Some(password)) = 
        (&matrix_homeserver, &matrix_user, &matrix_password) 
    {
        tracing::info!("Connecting to Matrix homeserver: {}", homeserver);
        
        let homeserver_url = if homeserver.starts_with("http") {
            homeserver.clone()
        } else {
            format!("https://{}", homeserver)
        };

        let room_members: Arc<RwLock<Vec<MatrixUser>>> = Arc::new(RwLock::new(Vec::new()));

        match Client::builder()
            .homeserver_url(homeserver_url)
            .build()
            .await
        {
            Ok(client) => {
                // Login
                match client.matrix_auth().login_username(&user, &password).send().await {
                    Ok(response) => {
                        tracing::info!("Matrix login successful: {}", response.user_id);
                        
                        // Spawn sync task if room_id is provided
                        if let Some(room_id_str) = &matrix_room_id {
                            match matrix_sdk::ruma::OwnedRoomId::try_from(room_id_str.as_str()) {
                                Ok(room_id) => {
                                    let sync_client = client.clone();
                                    let room_id_clone = room_id_str.clone();
                                    let matrix_state = MatrixState {
                                        room_members: room_members.clone(),
                                    };
                                    tokio::spawn(async move {
                                        sync_matrix_room(sync_client, room_id, room_id_clone, matrix_state).await;
                                    });
                                    tracing::info!("Matrix sync started for room: {}", room_id_str);
                                }
                                Err(e) => {
                                    tracing::error!("Invalid room ID '{}': {}", room_id_str, e);
                                }
                            }
                        }
                        
                        (Some(client), matrix_room_id, room_members)
                    }
                    Err(e) => {
                        tracing::error!("Matrix login failed: {}", e);
                        (None, None, Arc::new(RwLock::new(Vec::new())))
                    }
                }
            }
            Err(e) => {
                tracing::error!("Matrix client build failed: {}", e);
                (None, None, Arc::new(RwLock::new(Vec::new())))
            }
        }
    } else {
        tracing::warn!("Matrix credentials not provided, running without Matrix integration");
        (None, None, Arc::new(RwLock::new(Vec::new())))
    };

    let state = Arc::new(AppState {
        version: env!("CARGO_PKG_VERSION").to_string(),
        vm_configs: Arc::new(RwLock::new(Vec::new())),
        task_queues: Arc::new(RwLock::new(Vec::new())),
        matrix_client,
        matrix_room_id: matrix_room_id_opt,
        room_members,
    });

    let app = Router::new()
        // API routes
        .route("/health", get(health_handler))
        .route("/api/v1/status", get(status_handler))
        // Agent routes (Matrix room members)
        .route("/api/v1/agents", get(list_agents))
        // VM Config routes
        .route("/api/v1/vm-configs", get(list_vm_configs))
        .route("/api/v1/vm-configs", post(create_vm_config))
        .route("/api/v1/vm-configs/:id", get(get_vm_config))
        .route("/api/v1/vm-configs/:id", put(update_vm_config))
        .route("/api/v1/vm-configs/:id", delete(delete_vm_config))
        // Task Queue routes
        .route("/api/v1/task-queues", get(list_task_queues))
        .route("/api/v1/task-queues", post(create_task_queue))
        .route("/api/v1/task-queues/:id", get(get_task_queue))
        .route("/api/v1/task-queues/:id", put(update_task_queue))
        .route("/api/v1/task-queues/:id", delete(delete_task_queue))
        .route("/api/v1/task-queues/:id/tasks", post(add_task_to_queue))
        .route("/api/v1/task-queues/:queue_id/tasks/:task_id", delete(delete_task_from_queue))
        // Static files - serve frontend
        .nest_service("/", ServeDir::new("/app/frontend/static").append_index_html_on_directories(true))
        .with_state(state)
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .unwrap();
    
    tracing::info!("Listening on {}", listener.local_addr().unwrap());
    
    axum::serve(listener, app).await.unwrap();
    
    Ok(())
}

async fn sync_matrix_room(
    client: Client, 
    room_id: matrix_sdk::ruma::OwnedRoomId, 
    room_id_str: String,
    matrix_state: MatrixState,
) {
    let mut sync_settings = SyncSettings::new();
    sync_settings = sync_settings.timeout(std::time::Duration::from_secs(30));

    // Get the room
    let room = match client.get_room(&room_id) {
        Some(r) => r,
        None => {
            tracing::error!("Room not found: {}", room_id);
            return;
        }
    };

    // Initial member sync
    if let Ok(members) = room.members(matrix_sdk::room::RoomMemberships::JOIN).await {
        let mut members_list: Vec<MatrixUser> = members
            .iter()
            .filter(|m| {
                // Filter out the bot itself
                m.user_id() != client.user_id().unwrap_or(&matrix_sdk::ruma::user_id!("@unknown:unknown").to_owned())
            })
            .map(|m| MatrixUser {
                user_id: m.user_id().to_string(),
                display_name: m.display_name().map(|d| d.to_string()),
                avatar_url: m.avatar_url().map(|u| u.to_string()),
                is_bot: m.user_id().localpart().contains("bot") || 
                        m.display_name().map(|d| d.to_lowercase().contains("bot")).unwrap_or(false),
            })
            .collect();
        
        // Sort: bots first, then by display name
        members_list.sort_by(|a, b| {
            match (a.is_bot, b.is_bot) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => {
                    let a_name = a.display_name.as_deref().unwrap_or(&a.user_id);
                    let b_name = b.display_name.as_deref().unwrap_or(&b.user_id);
                    a_name.cmp(b_name)
                }
            }
        });

        tracing::info!("Loaded {} room members", members_list.len());
        
        // Update shared state
        let mut stored_members = matrix_state.room_members.write().await;
        *stored_members = members_list.clone();
        
        for member in &members_list {
            tracing::info!("  - {} ({})", 
                member.display_name.as_deref().unwrap_or(&member.user_id),
                member.user_id
            );
        }
    }

    // Set up room message handler
    room.add_event_handler(
        |ev: OriginalSyncRoomMessageEvent, room: Room| async move {
            let msg_body = ev.content.body();
            let sender = ev.sender.to_string();
            tracing::info!("Message from {}: {}", sender, msg_body);
            
            // TODO: Process commands from chat messages
            // For example: "list vms", "create queue", etc.
        },
    );

    // Initial sync
    if let Err(e) = client.sync_once(sync_settings.clone()).await {
        tracing::error!("Initial sync failed: {}", e);
        return;
    }

    // Continuous sync loop
    loop {
        // Re-fetch members on each sync to catch joins/leaves
        if let Ok(members) = room.members(matrix_sdk::room::RoomMemberships::JOIN).await {
            let mut members_list: Vec<MatrixUser> = members
                .iter()
                .filter(|m| {
                    m.user_id() != client.user_id().unwrap_or(&matrix_sdk::ruma::user_id!("@unknown:unknown").to_owned())
                })
                .map(|m| MatrixUser {
                    user_id: m.user_id().to_string(),
                    display_name: m.display_name().map(|d| d.to_string()),
                    avatar_url: m.avatar_url().map(|u| u.to_string()),
                    is_bot: m.user_id().localpart().contains("bot") || 
                            m.display_name().map(|d| d.to_lowercase().contains("bot")).unwrap_or(false),
                })
                .collect();
            
            members_list.sort_by(|a, b| {
                match (a.is_bot, b.is_bot) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => {
                        let a_name = a.display_name.as_deref().unwrap_or(&a.user_id);
                        let b_name = b.display_name.as_deref().unwrap_or(&b.user_id);
                        a_name.cmp(b_name)
                    }
                }
            });

            let mut stored_members = matrix_state.room_members.write().await;
            *stored_members = members_list;
        }

        if let Err(e) = client.sync_once(sync_settings.clone()).await {
            tracing::error!("Sync error: {}", e);
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            continue;
        }
    }
}

async fn health_handler(State(state): State<Arc<AppState>>) -> Json<HealthResponse> {
    let members = state.room_members.read().await;
    Json(HealthResponse {
        status: "healthy".to_string(),
        version: state.version.clone(),
        matrix_connected: state.matrix_client.is_some(),
        matrix_room_id: state.matrix_room_id.clone(),
        room_members_count: members.len(),
    })
}

async fn status_handler(State(state): State<Arc<AppState>>) -> Json<HealthResponse> {
    let members = state.room_members.read().await;
    Json(HealthResponse {
        status: "ok".to_string(),
        version: state.version.clone(),
        matrix_connected: state.matrix_client.is_some(),
        matrix_room_id: state.matrix_room_id.clone(),
        room_members_count: members.len(),
    })
}

// Agent handlers (Matrix room members)
async fn list_agents(State(state): State<Arc<AppState>>) -> Json<Vec<MatrixUser>> {
    let members = state.room_members.read().await;
    Json(members.clone())
}

// VM Config handlers
async fn list_vm_configs(State(state): State<Arc<AppState>>) -> Json<Vec<VmConfig>> {
    let configs = state.vm_configs.read().await;
    Json(configs.clone())
}

async fn create_vm_config(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateVmConfig>,
) -> (StatusCode, Json<VmConfig>) {
    let now = chrono::Utc::now().to_rfc3339();
    let config = VmConfig {
        id: uuid::Uuid::new_v4().to_string(),
        name: payload.name,
        agent_id: payload.agent_id,
        enabled: true,
        created_at: now.clone(),
        updated_at: now,
    };
    
    let mut configs = state.vm_configs.write().await;
    configs.push(config.clone());
    
    (StatusCode::CREATED, Json(config))
}

async fn get_vm_config(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<VmConfig>, StatusCode> {
    let configs = state.vm_configs.read().await;
    configs
        .iter()
        .find(|c| c.id == id)
        .map(|c| Json(c.clone()))
        .ok_or(StatusCode::NOT_FOUND)
}

async fn update_vm_config(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateVmConfig>,
) -> Result<Json<VmConfig>, StatusCode> {
    let mut configs = state.vm_configs.write().await;
    let config = configs
        .iter_mut()
        .find(|c| c.id == id)
        .ok_or(StatusCode::NOT_FOUND)?;
    
    if let Some(name) = payload.name {
        config.name = name;
    }
    if let Some(agent_id) = payload.agent_id {
        config.agent_id = agent_id;
    }
    if let Some(enabled) = payload.enabled {
        config.enabled = enabled;
    }
    config.updated_at = chrono::Utc::now().to_rfc3339();
    
    Ok(Json(config.clone()))
}

async fn delete_vm_config(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    let mut configs = state.vm_configs.write().await;
    let pos = configs
        .iter()
        .position(|c| c.id == id)
        .ok_or(StatusCode::NOT_FOUND)?;
    configs.remove(pos);
    Ok(StatusCode::NO_CONTENT)
}

// Task Queue handlers
async fn list_task_queues(State(state): State<Arc<AppState>>) -> Json<Vec<TaskQueue>> {
    let queues = state.task_queues.read().await;
    Json(queues.clone())
}

async fn create_task_queue(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateTaskQueue>,
) -> (StatusCode, Json<TaskQueue>) {
    let now = chrono::Utc::now().to_rfc3339();
    let tasks = payload.tasks.unwrap_or_default().into_iter().map(|t| {
        Task {
            id: uuid::Uuid::new_v4().to_string(),
            description: t.description,
            keystrokes: t.keystrokes,
            delay_ms: t.delay_ms,
            completed: false,
        }
    }).collect();
    
    let queue = TaskQueue {
        id: uuid::Uuid::new_v4().to_string(),
        vm_id: payload.vm_id,
        name: payload.name,
        tasks,
        loop_enabled: payload.loop_enabled.unwrap_or(false),
        enabled: true,
        created_at: now.clone(),
        updated_at: now,
    };
    
    let mut queues = state.task_queues.write().await;
    queues.push(queue.clone());
    
    (StatusCode::CREATED, Json(queue))
}

async fn get_task_queue(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<TaskQueue>, StatusCode> {
    let queues = state.task_queues.read().await;
    queues
        .iter()
        .find(|q| q.id == id)
        .map(|q| Json(q.clone()))
        .ok_or(StatusCode::NOT_FOUND)
}

async fn update_task_queue(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateTaskQueue>,
) -> Result<Json<TaskQueue>, StatusCode> {
    let mut queues = state.task_queues.write().await;
    let queue = queues
        .iter_mut()
        .find(|q| q.id == id)
        .ok_or(StatusCode::NOT_FOUND)?;
    
    if let Some(name) = payload.name {
        queue.name = name;
    }
    if let Some(tasks) = payload.tasks {
        queue.tasks = tasks.into_iter().map(|t| {
            Task {
                id: uuid::Uuid::new_v4().to_string(),
                description: t.description,
                keystrokes: t.keystrokes,
                delay_ms: t.delay_ms,
                completed: false,
            }
        }).collect();
    }
    if let Some(loop_enabled) = payload.loop_enabled {
        queue.loop_enabled = loop_enabled;
    }
    if let Some(enabled) = payload.enabled {
        queue.enabled = enabled;
    }
    queue.updated_at = chrono::Utc::now().to_rfc3339();
    
    Ok(Json(queue.clone()))
}

async fn delete_task_queue(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    let mut queues = state.task_queues.write().await;
    let pos = queues
        .iter()
        .position(|q| q.id == id)
        .ok_or(StatusCode::NOT_FOUND)?;
    queues.remove(pos);
    Ok(StatusCode::NO_CONTENT)
}

async fn add_task_to_queue(
    State(state): State<Arc<AppState>>,
    Path(queue_id): Path<String>,
    Json(payload): Json<CreateTask>,
) -> Result<Json<TaskQueue>, StatusCode> {
    let mut queues = state.task_queues.write().await;
    let queue = queues
        .iter_mut()
        .find(|q| q.id == queue_id)
        .ok_or(StatusCode::NOT_FOUND)?;
    
    let task = Task {
        id: uuid::Uuid::new_v4().to_string(),
        description: payload.description,
        keystrokes: payload.keystrokes,
        delay_ms: payload.delay_ms,
        completed: false,
    };
    
    queue.tasks.push(task);
    queue.updated_at = chrono::Utc::now().to_rfc3339();
    
    Ok(Json(queue.clone()))
}

async fn delete_task_from_queue(
    State(state): State<Arc<AppState>>,
    Path((queue_id, task_id)): Path<(String, String)>,
) -> Result<StatusCode, StatusCode> {
    let mut queues = state.task_queues.write().await;
    let queue = queues
        .iter_mut()
        .find(|q| q.id == queue_id)
        .ok_or(StatusCode::NOT_FOUND)?;
    
    let pos = queue
        .tasks
        .iter()
        .position(|t| t.id == task_id)
        .ok_or(StatusCode::NOT_FOUND)?;
    
    queue.tasks.remove(pos);
    queue.updated_at = chrono::Utc::now().to_rfc3339();
    
    Ok(StatusCode::NO_CONTENT)
}
