use axum::{
    Router,
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{delete, get, post, put},
};
use matrix_sdk::{
    Client, OwnedServerName, RoomMemberships, ServerName,
    ruma::{OwnedRoomOrAliasId, RoomOrAliasId},
};
use reqwest::Client as HttpClient;
use serde::{Deserialize, Serialize};
use std::{sync::Arc, time::Duration};
use tokio::{sync::RwLock, time::sleep};
use tokio_util::sync::CancellationToken;
use tower_http::{cors::CorsLayer, services::ServeDir, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Clone)]
struct AppState {
    version: &'static str,
    home_server: &'static str,
    room_id: &'static str,
    user_id: &'static str,
    vmware_gateway_url: String,
    vm_configs: Arc<RwLock<Vec<VmConfig>>>,
    task_queues: Arc<RwLock<Vec<TaskQueue>>>,
    client: Client,
    http_client: HttpClient,
    matrix_state: MatrixState,
}

#[derive(Clone)]
struct MatrixState {
    room_members: Arc<RwLock<Box<[MatrixUser]>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
struct MatrixUser {
    user_id: String,
    display_name: Option<String>,
}

#[derive(Serialize)]
struct HealthResponse {
    version: &'static str,
    home_server: &'static str,
    room_id: &'static str,
    user_id: &'static str,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct VmConfig {
    id: String,
    name: String,
    user_id: String,
    enabled: bool,
    created_at: String,
    updated_at: String,
}

#[derive(Deserialize)]
struct CreateVmConfig {
    name: String,
    user_id: String,
    enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct VmwareVmList {
    vms: Vec<String>,
    count: usize,
    pattern: String,
}

#[derive(Deserialize)]
struct UpdateVmConfig {
    name: Option<String>,
    user_id: Option<String>,
    enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TaskQueue {
    id: String,
    vm_id: String,
    name: String,
    tasks: Vec<Task>,
    enabled: bool,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Task {
    description: String,
    keystrokes: Option<String>,
    delay_ms: Option<u64>,
}

#[derive(Deserialize)]
struct CreateTaskQueue {
    vm_id: String,
    name: String,
    tasks: Vec<Task>,
    enabled: bool,
}

#[derive(Deserialize)]
struct UpdateTaskQueue {
    vm_id: Option<String>,
    name: Option<String>,
    tasks: Option<Vec<Task>>,
    enabled: Option<bool>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                "npc_ui_backend=debug,tower_http=debug,matrix_sdk=debug".into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load Matrix credentials from environment
    let home_server: &str = std::env::var("MATRIX_URL")?.leak();
    let user_id: &str = std::env::var("MATRIX_USER")?.leak();
    let password: &str = std::env::var("MATRIX_PASSWORD")?.leak();
    let room_id: &str = std::env::var("MATRIX_ROOM_ID")?.leak();
    let vmware_gateway_url = std::env::var("VMWARE_GATEWAY_URL")
        .unwrap_or_else(|_| "http://vmware-gateway-service.npc.svc.cluster.local".to_string());

    let owned_room_id = RoomOrAliasId::parse(room_id)?;
    let owned_server_name = ServerName::parse(home_server)?;

    let client = Client::builder()
        .server_name(&owned_server_name)
        .build()
        .await?;
    let response = client
        .matrix_auth()
        .login_username(user_id, password)
        .send()
        .await?;
    tracing::info!("Matrix login successful: {}", response.user_id);

    let matrix_state = MatrixState {
        room_members: Arc::new(RwLock::new(Default::default())),
    };

    let token = CancellationToken::new();

    let background_job = tokio::spawn(sync_matrix_room(
        token.clone(),
        client.clone(),
        owned_room_id,
        owned_server_name,
        matrix_state.clone(),
    ));

    let http_client = HttpClient::builder()
        .timeout(Duration::from_secs(10))
        .build()?;

    let state = AppState {
        version: env!("CARGO_PKG_VERSION"),
        home_server,
        user_id,
        room_id,
        vmware_gateway_url,
        vm_configs: Arc::new(RwLock::new(Vec::new())),
        task_queues: Arc::new(RwLock::new(Vec::new())),
        client,
        http_client,
        matrix_state,
    };

    let app = Router::new()
        .route("/api/v1/agents", get(list_agents))
        .route("/api/v1/vms", get(list_vms))
        .route("/api/v1/vm-configs", get(list_vm_configs))
        .route("/api/v1/vm-configs", post(create_vm_config))
        .route("/api/v1/vm-configs/:id", get(get_vm_config))
        .route("/api/v1/vm-configs/:id", put(update_vm_config))
        .route("/api/v1/vm-configs/:id", delete(delete_vm_config))
        .route("/api/v1/task-queues", get(list_task_queues))
        .route("/api/v1/task-queues", post(create_task_queue))
        .route("/api/v1/task-queues/:id", get(get_task_queue))
        .route("/api/v1/task-queues/:id", put(update_task_queue))
        .route("/api/v1/task-queues/:id", delete(delete_task_queue))
        .nest_service(
            "/",
            ServeDir::new("/app/frontend/static").append_index_html_on_directories(true),
        )
        .layer(TraceLayer::new_for_http())
        .route("/health", get(status_handler))
        .route("/api/v1/status", get(status_handler))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;

    tracing::info!(
        "Listening on {}",
        listener
            .local_addr()
            .map(|addr| addr.to_string())
            .unwrap_or_else(|_| { "<Unknown>".to_string() })
    );

    let serve = axum::serve(listener, app).with_graceful_shutdown(token.cancelled_owned());
    serve.await?;
    background_job.await??;
    Ok(())
}

async fn sync_matrix_room(
    token: CancellationToken,
    client: Client,
    room_id: OwnedRoomOrAliasId,
    server_id: OwnedServerName,
    matrix_state: MatrixState,
) -> anyhow::Result<()> {
    let _drop = token.drop_guard_ref();
    let room = client
        .join_room_by_id_or_alias(&room_id, &[server_id])
        .await?;
    tracing::info!("Joined room: {}", room_id);

    loop {
        if let None = token
            .run_until_cancelled(sleep(Duration::from_secs(5)))
            .await
        {
            break;
        };

        let Ok(members) = room.members(RoomMemberships::all()).await else {
            tracing::error!("Failed to get room members");
            continue;
        };
        let mut members: Box<[MatrixUser]> = members
            .iter()
            .map(|m| MatrixUser {
                user_id: m.user_id().as_str().to_owned(),
                display_name: m.display_name().map(ToOwned::to_owned),
            })
            .collect();
        members.sort();

        // Update shared state
        let mut stored_members = matrix_state.room_members.write().await;
        if *stored_members != members {
            tracing::info!(?members, "Updated members");
            *stored_members = members;
        }
    }
    Ok(())
}

async fn status_handler(State(state): State<AppState>) -> Json<HealthResponse> {
    let AppState {
        version,
        home_server,
        room_id,
        user_id,
        ..
    } = state;
    Json(HealthResponse {
        version,
        home_server,
        room_id,
        user_id,
    })
}

// Agent handlers (Matrix room members)
async fn list_agents(State(state): State<AppState>) -> Json<Box<[MatrixUser]>> {
    let members = state.matrix_state.room_members.read().await;
    Json(members.clone())
}

// VMware VM handlers
async fn list_vms(State(state): State<AppState>) -> Result<Json<Vec<String>>, StatusCode> {
    let url = format!("{}/api/vms", state.vmware_gateway_url);
    
    let response = state.http_client
        .get(&url)
        .send()
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch VMs from vmware_gateway: {}", e);
            StatusCode::BAD_GATEWAY
        })?;
    
    if !response.status().is_success() {
        tracing::error!("vmware_gateway returned status: {}", response.status());
        return Err(StatusCode::BAD_GATEWAY);
    }
    
    let vm_list: VmwareVmList = response.json().await.map_err(|e| {
        tracing::error!("Failed to parse vmware_gateway response: {}", e);
        StatusCode::BAD_GATEWAY
    })?;
    
    Ok(Json(vm_list.vms))
}

// VM Config handlers
async fn list_vm_configs(State(state): State<AppState>) -> Json<Vec<VmConfig>> {
    let configs = state.vm_configs.read().await;
    Json(configs.clone())
}

async fn create_vm_config(
    State(state): State<AppState>,
    Json(payload): Json<CreateVmConfig>,
) -> (StatusCode, Json<VmConfig>) {
    let now = chrono::Utc::now().to_rfc3339();
    let CreateVmConfig {
        name,
        user_id,
        enabled,
    } = payload;
    let config = VmConfig {
        id: uuid::Uuid::new_v4().to_string(),
        created_at: now.clone(),
        updated_at: now,
        name,
        user_id,
        enabled,
    };

    state.vm_configs.write().await.push(config.clone());
    (StatusCode::CREATED, Json(config))
}

async fn get_vm_config(
    State(state): State<AppState>,
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
    State(state): State<AppState>,
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
    if let Some(user_id) = payload.user_id {
        config.user_id = user_id;
    }
    if let Some(enabled) = payload.enabled {
        config.enabled = enabled;
    }
    config.updated_at = chrono::Utc::now().to_rfc3339();

    Ok(Json(config.clone()))
}

async fn delete_vm_config(
    State(state): State<AppState>,
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
async fn list_task_queues(State(state): State<AppState>) -> Json<Vec<TaskQueue>> {
    let queues = state.task_queues.read().await;
    Json(queues.clone())
}

async fn create_task_queue(
    State(state): State<AppState>,
    Json(payload): Json<CreateTaskQueue>,
) -> (StatusCode, Json<TaskQueue>) {
    let now = chrono::Utc::now().to_rfc3339();
    let queue = TaskQueue {
        id: uuid::Uuid::new_v4().to_string(),
        vm_id: payload.vm_id,
        name: payload.name,
        tasks: payload.tasks,
        enabled: payload.enabled,
        created_at: now.clone(),
        updated_at: now,
    };

    let mut queues = state.task_queues.write().await;
    queues.push(queue.clone());

    (StatusCode::CREATED, Json(queue))
}

async fn get_task_queue(
    State(state): State<AppState>,
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
    State(state): State<AppState>,
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
        queue.tasks = tasks;
    }
    if let Some(vm_id) = payload.vm_id {
        queue.vm_id = vm_id;
    }
    if let Some(enabled) = payload.enabled {
        queue.enabled = enabled;
    }
    queue.updated_at = chrono::Utc::now().to_rfc3339();
    Ok(Json(queue.clone()))
}

async fn delete_task_queue(
    State(state): State<AppState>,
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
