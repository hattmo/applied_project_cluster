use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, Json, IntoResponse},
    routing::{delete, get, post, put},
    Router,
};
use tower_http::services::ServeDir;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Clone)]
struct AppState {
    version: String,
    vm_configs: Arc<RwLock<Vec<VmConfig>>>,
    task_queues: Arc<RwLock<Vec<TaskQueue>>>,
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

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "npc_ui_backend=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let state = Arc::new(AppState {
        version: env!("CARGO_PKG_VERSION").to_string(),
        vm_configs: Arc::new(RwLock::new(Vec::new())),
        task_queues: Arc::new(RwLock::new(Vec::new())),
    });

    let app = Router::new()
        // API routes
        .route("/health", get(health_handler))
        .route("/api/v1/status", get(status_handler))
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
}

async fn health_handler() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

async fn status_handler(State(state): State<Arc<AppState>>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        version: state.version.clone(),
    })
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
