use axum::{
    Router,
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{delete, get, post, put},
};
use k8s_openapi::api::apps::v1::StatefulSet;
use kube::{
    Client as KubeClient,
    api::{Api, Patch, PatchParams},
};
use matrix_sdk::{
    Client, Room, RoomMemberships, ServerName,
    ruma::{
        RoomOrAliasId, UserId, api::client::room::create_room,
        events::room::message::RoomMessageEventContent,
    },
};
use reqwest::Client as HttpClient;
use serde::{Deserialize, Serialize};
use std::{fs, process::ExitCode, sync::Arc, time::Duration};
use tokio::{sync::RwLock, time::sleep};
use tokio_util::sync::CancellationToken;
use tower_http::{cors::CorsLayer, services::ServeDir, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Serialize)]
struct AgentScaleStatus {
    current_replicas: i32,
}

#[derive(Deserialize)]
struct ScaleAgentsRequest {
    replicas: i32,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct MatrixError {
    errcode: String,
    error: String,
}

#[derive(Clone, Default)]
struct MutableState {
    vm_configs: Arc<RwLock<Vec<VmConfig>>>,
    task_queues: Arc<RwLock<Vec<TaskQueue>>>,
    room_members: Arc<RwLock<Box<[MatrixUser]>>>,
    replicas: Arc<RwLock<i32>>,
}

impl MutableState {
    fn new(replicas: i32) -> Self {
        Self {
            replicas: Arc::new(RwLock::new(replicas)),
            ..Default::default()
        }
    }
}

#[allow(dead_code)]
#[derive(Clone)]
struct AppState {
    version: &'static str,
    matrix_hostname: &'static str,
    vmware_gateway_hostname: &'static str,
    username: &'static str,
    namespace: &'static str,
    mutable_state: MutableState,
    http_client: HttpClient,
    client: Client,
    room: Room,
    kube_client: KubeClient,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
struct MatrixUser {
    user_id: String,
    display_name: Option<String>,
}

#[derive(Serialize)]
struct HealthResponse {
    version: &'static str,
    vmware_gateway_hostname: &'static str,
    matrix_hostname: &'static str,
    username: &'static str,
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

async fn create_client(
    matrix_hostname: &str,
    shared_secret: &str,
    password: &str,
) -> anyhow::Result<(Client, HttpClient, &'static str)> {
    let username = "controller";

    let ca_cert_path = std::env::var("MATRIX_CA_CERT")?;
    tracing::info!("Loading CA certificate from {}", ca_cert_path);
    let ca_cert_pem = fs::read(&ca_cert_path)?;
    let ca_cert = reqwest::Certificate::from_pem(&ca_cert_pem)?;

    let client = Client::builder()
        .server_name(&ServerName::parse(matrix_hostname)?)
        .add_root_certificates(vec![ca_cert.clone()])
        .build()
        .await?;

    let http_client = HttpClient::builder()
        .timeout(Duration::from_secs(30))
        .add_root_certificate(ca_cert)
        .build()?;

    tracing::info!("CA certificate loaded, attempting login");
    let test_result = client
        .matrix_auth()
        .login_username(username, password)
        .send()
        .await;

    if test_result.is_ok() {
        tracing::info!("Existing Matrix credentials are valid");
        return Ok((client, http_client, username));
    }
    tracing::warn!("Matrix credentials are invalid, Creating account");

    create_account(
        matrix_hostname,
        shared_secret,
        password,
        username,
        true,
        &http_client,
    )
    .await?;

    client
        .matrix_auth()
        .login_username(&username, &password)
        .send()
        .await?;

    Ok((client, http_client, username))
}

async fn create_account(
    matrix_hostname: &str,
    shared_secret: &str,
    password: &str,
    username: &str,
    admin: bool,
    http_client: &HttpClient,
) -> Result<(), anyhow::Error> {
    let register_url = format!("https://{}/_synapse/admin/v1/register", matrix_hostname);
    let nonce_response = http_client
        .get(&register_url)
        .send()
        .await?
        .json::<serde_json::Value>()
        .await?;
    tracing::debug!(?nonce_response, "Register api get response");
    let nonce = nonce_response
        .get("nonce")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("No nonce in response"))?;
    tracing::debug!(?shared_secret, %username, %password, "Creating new user");
    let admin_bytes = if admin {
        b"admin".as_slice()
    } else {
        &b"notadmin".as_slice()
    };
    let bytes = [
        nonce.as_bytes(),
        b"\0",
        username.as_bytes(),
        b"\0",
        password.as_bytes(),
        b"\0",
        admin_bytes,
    ];
    let bytes: Box<[u8]> = bytes.into_iter().flatten().copied().collect();
    let signature = hmac_sha1_compact::HMAC::mac(&bytes, shared_secret.as_bytes());
    let signature = hex::encode(signature);
    let register_body = serde_json::json!({
        "nonce": nonce,
        "username": username,
        "password": password,
        "admin": admin,
        "mac": signature
    });
    let response = http_client
        .post(&register_url)
        .json(&register_body)
        .send()
        .await?;
    if !response.status().is_success() {
        let response_json: MatrixError = response.json().await?;
        tracing::info!(response = ?response_json, "Failed to create account");
        anyhow::bail!("Failed to setup client")
    }
    tracing::info!("Matrix account created successfully: {}", username);
    Ok(())
}

#[tokio::main]
async fn main() -> ExitCode {
    if let Err(e) = setup().await {
        tracing::error!(error=?e, "Fatal Error");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
async fn setup() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                "controller_backend=debug,tower_http=debug,matrix_sdk=debug".into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load environment variables

    let namespace: &'static str = std::env::var("NAMESPACE")?.leak();
    let matrix_hostname: &'static str = std::env::var("MATRIX_HOSTNAME")?.leak();
    let vmware_gateway_hostname: &'static str = std::env::var("VMWARE_GATEWAY_HOSTNAME")?.leak();

    let matrix_secret: &'static str = std::env::var("MATRIX_SECRET")?.leak();
    let matrix_password = std::env::var("MATRIX_PASSWORD")?;

    // Load or create Matrix credentials
    let (client, http_client, username) =
        create_client(matrix_hostname, matrix_secret, &matrix_password).await?;

    let Ok(owned_room_id) = RoomOrAliasId::parse(format!("#agent_room:{matrix_hostname}")) else {
        anyhow::bail!("Failed to make room alias: {matrix_hostname}");
    };
    let Ok(owned_server_name) = ServerName::parse(matrix_hostname) else {
        anyhow::bail!("Failed to make owned server name: {matrix_hostname}");
    };

    let room = match client
        .join_room_by_id_or_alias(&owned_room_id, &[owned_server_name.clone()])
        .await
    {
        Ok(room) => room,
        Err(e) => {
            tracing::error!(error=?e, "Could not join room trying to create");
            let mut room_req = create_room::v3::Request::new();
            room_req.name = Some("agent_room".into());
            room_req.room_alias_name = Some("agent_room".into());
            client.create_room(room_req).await?
        }
    };
    tracing::info!("Joined room: {}", owned_room_id);

    let kube_client = KubeClient::try_default().await?;
    let api: Api<StatefulSet> = Api::namespaced(kube_client.clone(), namespace);
    let replica_count = get_replica_count(&api)
        .await
        .map_err(|_| anyhow::anyhow!("Failed to get replica count"))?;

    let mutable_state = MutableState::new(replica_count);
    for i in 0..5 {
        let username = format!("agent_{i}");
        let _ = create_account(
            matrix_hostname,
            matrix_secret,
            format!("{matrix_password}_{i}").as_str(),
            format!("agent_{i}").as_str(),
            false,
            &http_client,
        )
        .await;
        let user_id = &UserId::parse_with_server_name(username, &owned_server_name)?;
        if i < replica_count {
            let _ = room.invite_user_by_id(user_id).await;
        } else {
            let _ = room.kick_user(user_id, Some("Scaled down"));
        }
    }
    let token = CancellationToken::new();
    tracing::info!("Starting background job");
    let background_job = tokio::spawn(sync_matrix_room(
        token.clone(),
        room.clone(),
        mutable_state.clone(),
    ));
    let state = AppState {
        version: env!("CARGO_PKG_VERSION"),
        matrix_hostname,
        vmware_gateway_hostname,
        mutable_state,
        http_client,
        client,
        room,
        username,
        kube_client,
        namespace,
    };
    tracing::info!("Seting up routes");
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
        .route("/api/v1/agents/scale", put(update_scale_agents))
        .route("/api/v1/agents/scale", get(get_scale_agents))
        .nest_service(
            "/",
            ServeDir::new("/app/frontend/static").append_index_html_on_directories(true),
        )
        .layer(TraceLayer::new_for_http())
        .route("/health", get(status_handler))
        .route("/api/v1/status", get(status_handler))
        .layer(CorsLayer::permissive())
        .with_state(state);
    tracing::info!("Setting up listener");
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;

    tracing::info!(
        "Listening on {}",
        listener
            .local_addr()
            .map(|addr| addr.to_string())
            .unwrap_or_else(|_| { "<Unknown>".to_string() })
    );

    let serve = axum::serve(listener, app).with_graceful_shutdown(token.cancelled_owned());

    tracing::info!("Server starting");
    serve.await?;
    background_job.await??;
    Ok(())
}

async fn sync_matrix_room(
    token: CancellationToken,
    room: Room,
    MutableState {
        room_members,
        task_queues,
        vm_configs,
        ..
    }: MutableState,
) -> anyhow::Result<()> {
    let _drop = token.drop_guard_ref();

    loop {
        if let None = token
            .run_until_cancelled(sleep(Duration::from_secs(60)))
            .await
        {
            break;
        };

        update_membership(&room, &room_members).await;
        for queue in task_queues
            .read()
            .await
            .iter()
            .filter(|queue| queue.enabled)
        {
            tracing::info!(?queue, "Enabled task");
            let Some(agent_name) = vm_configs.read().await.iter().find_map(|i| {
                if i.name == queue.vm_id {
                    Some(i.user_id.clone())
                } else {
                    None
                }
            }) else {
                continue;
            };
            let queue_message = build_promt(queue, &agent_name);
            // Create a proper Matrix mention so only the targeted agent responds
            let content = if let Ok(user_id) = UserId::parse(agent_name.as_str()) {
                RoomMessageEventContent::text_mention(&queue_message, &user_id)
            } else {
                RoomMessageEventContent::text_plain(&queue_message)
            };
            let _ = room.send(content).await;
        }
    }
    Ok(())
}

fn build_promt(task_queue: &TaskQueue, agent_name: &str) -> String {
    let rows: String = task_queue
        .tasks
        .iter()
        .map(
            |Task {
                 description,
                 keystrokes,
                 delay_ms,
             }| {
                let keystrokes = keystrokes
                    .as_ref()
                    .map(|i| i.clone())
                    .unwrap_or_else(|| " ".to_string());
                let delay_ms = delay_ms
                    .map(|i| i.to_string())
                    .unwrap_or_else(|| " ".to_string());
                format!("|{description}|{keystrokes}|{delay_ms}|\n")
            },
        )
        .collect();
    format!(
        "@{}, I want you to perform the following task as described in the table below on the VM ({}). To perform these tasks utilize the vmware gateway at the url http://vmware-gateway. you should already know how to use that api based on tools in your context. The description is a description of the task, and if there are keystrokes try using those to accomplish your task but feel free to adjust if they dont work.  Remember which tasks you complete and when you see this message again compare what you have already done with what you still need to do and pick up where you left off.  If all the tasks are already completed start at the top of the list and do them again.  If you get stuck use judgment to try to complete the task in spirit.  The main point is to persistently send commands to the vm and try to work through issues: \n|Description|Keystrokes|delay|\n|:---:|:---:|:---:|\n{rows}",
        agent_name, task_queue.vm_id
    )
}

async fn update_membership(room: &Room, room_members: &Arc<RwLock<Box<[MatrixUser]>>>) {
    let Ok(members) = room.members(RoomMemberships::all()).await else {
        tracing::error!("Failed to get room members");
        return;
    };
    let mut members: Box<[MatrixUser]> = members
        .iter()
        .map(|m| MatrixUser {
            user_id: m.user_id().as_str().to_owned(),
            display_name: m.display_name().map(ToOwned::to_owned),
        })
        .collect();
    members.sort();
    let mut stored_members = room_members.write().await;

    // Update shared state
    if *stored_members != members {
        tracing::info!(?members, "Updated members");
        *stored_members = members;
    }
}

async fn status_handler(State(state): State<AppState>) -> Json<HealthResponse> {
    let AppState {
        vmware_gateway_hostname,
        matrix_hostname,
        version,
        username,
        ..
    } = state;
    Json(HealthResponse {
        version,
        vmware_gateway_hostname,
        matrix_hostname,
        username,
    })
}

// Agent handlers (Matrix room members)
async fn list_agents(State(state): State<AppState>) -> Json<Box<[MatrixUser]>> {
    let members = state.mutable_state.room_members.read().await;
    Json(members.clone())
}

async fn get_scale_agents(
    State(state): State<AppState>,
) -> Result<Json<AgentScaleStatus>, StatusCode> {
    Ok(Json(AgentScaleStatus {
        current_replicas: *state.mutable_state.replicas.read().await,
    }))
}

async fn update_scale_agents(
    State(state): State<AppState>,
    Json(payload): Json<ScaleAgentsRequest>,
) -> Result<Json<AgentScaleStatus>, StatusCode> {
    let replicas = payload.replicas;
    if replicas < 1 || replicas > 5 {
        return Err(StatusCode::BAD_REQUEST);
    }

    let mut stored_replicas = state.mutable_state.replicas.write().await;
    let api: Api<StatefulSet> = Api::namespaced(state.kube_client.clone(), state.namespace);
    let current_replicas = get_replica_count(&api).await?;

    // Handle Matrix room membership changes
    let server_name = ServerName::parse(state.matrix_hostname).map_err(|e| {
        tracing::error!("Failed to parse server name: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if replicas < current_replicas {
        // Scaling down - kick excess agents
        for i in replicas..current_replicas {
            let user_id = format!("agent_{}", i);
            if let Ok(user) = UserId::parse_with_server_name(user_id.as_str(), &server_name) {
                if let Err(e) = state.room.kick_user(&user, Some("Scaled down")).await {
                    tracing::warn!("Failed to kick agent {}: {}", user_id, e);
                } else {
                    tracing::info!("Kicked agent {} from room (scale down)", user_id);
                }
            }
        }
    } else if replicas > current_replicas {
        // Scaling up - invite new agents
        for i in current_replicas..replicas {
            let user_id = format!("agent_{}", i);
            if let Ok(user) = UserId::parse_with_server_name(user_id.as_str(), &server_name) {
                if let Err(e) = state.room.invite_user_by_id(&user).await {
                    tracing::warn!("Failed to invite agent {}: {}", user_id, e);
                } else {
                    tracing::info!("Invited agent {} to room (scale up)", user_id);
                }
            }
        }
    }

    let patch = serde_json::json!({
        "spec": {
            "replicas": replicas
        }
    });

    let patch_params = PatchParams::default();
    let patch = Patch::Merge(&patch);

    let result = api
        .patch("agent", &patch_params, &patch)
        .await
        .map_err(|e| {
            tracing::error!("Failed to scale agents: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let new_replicas = result.spec.and_then(|i| i.replicas).unwrap_or(1);
    *stored_replicas = new_replicas;
    Ok(Json(AgentScaleStatus {
        current_replicas: new_replicas,
    }))
}

async fn get_replica_count(api: &Api<StatefulSet>) -> Result<i32, StatusCode> {
    let current_sts = api.get("agent").await.map_err(|e| {
        tracing::error!("Failed to get current StatefulSet: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let current_replicas = current_sts
        .spec
        .and_then(|i| i.replicas)
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(current_replicas)
}

// VMware VM handlers
async fn list_vms(State(state): State<AppState>) -> Result<Json<Vec<String>>, StatusCode> {
    let url = format!("http://{}/api/vms", state.vmware_gateway_hostname);

    let response = state.http_client.get(&url).send().await.map_err(|e| {
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
    let configs = state.mutable_state.vm_configs.read().await;
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

    state
        .mutable_state
        .vm_configs
        .write()
        .await
        .push(config.clone());
    (StatusCode::CREATED, Json(config))
}

async fn get_vm_config(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<VmConfig>, StatusCode> {
    let configs = state.mutable_state.vm_configs.read().await;
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
    let mut configs = state.mutable_state.vm_configs.write().await;
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
    let mut configs = state.mutable_state.vm_configs.write().await;
    let pos = configs
        .iter()
        .position(|c| c.id == id)
        .ok_or(StatusCode::NOT_FOUND)?;
    configs.remove(pos);
    Ok(StatusCode::NO_CONTENT)
}

async fn list_task_queues(State(state): State<AppState>) -> Json<Vec<TaskQueue>> {
    let queues = state.mutable_state.task_queues.read().await;
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

    let mut queues = state.mutable_state.task_queues.write().await;
    queues.push(queue.clone());

    (StatusCode::CREATED, Json(queue))
}

async fn get_task_queue(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<TaskQueue>, StatusCode> {
    let queues = state.mutable_state.task_queues.read().await;
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
    let mut queues = state.mutable_state.task_queues.write().await;
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
    let mut queues = state.mutable_state.task_queues.write().await;
    let pos = queues
        .iter()
        .position(|q| q.id == id)
        .ok_or(StatusCode::NOT_FOUND)?;
    queues.remove(pos);
    Ok(StatusCode::NO_CONTENT)
}
