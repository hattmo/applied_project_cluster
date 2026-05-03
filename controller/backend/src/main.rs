use anyhow::anyhow;
use axum::{
    Router,
    extract::State,
    response::Json,
    routing::{delete, get, post, put},
};
use k8s_openapi::api::apps::v1::StatefulSet;
use kube::{Client as KubeClient, api::Api};
use matrix_sdk::{
    Client as MatrixClient, Room, RoomMemberships, ServerName,
    ruma::{RoomOrAliasId, UserId, api::client::room::create_room},
};
use reqwest::Client as HttpClient;
use serde::{Deserialize, Serialize};
use tokio::sync::{
    RwLock,
    mpsc::{self, UnboundedSender},
};
use tokio_util::sync::CancellationToken;
use tower_http::{cors::CorsLayer, services::ServeDir, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use std::{fs, process::ExitCode, sync::Arc, time::Duration};

mod assignments;
mod scale;
mod tasks;
mod vms;

use assignments::{
    AgentAssignment, create_agent_assignment, delete_agent_assignment, get_agent_assignment,
    list_agent_assignments, update_agent_assignment,
};
use scale::{get_replica_count, get_scale_agents, update_scale_agents};
use tasks::{
    TaskQueue, create_task_queue, delete_task_queue, get_task_queue, list_task_queues,
    sync_matrix_room, update_task_queue,
};
use vms::list_vms;

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct MatrixError {
    errcode: String,
    error: String,
}

#[derive(Clone, Default)]
struct MutableState {
    agent_assignments: Arc<RwLock<Vec<AgentAssignment>>>,
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

#[derive(Clone)]
struct StaticState {
    version: &'static str,
    matrix_hostname: &'static str,
    vmware_gateway_hostname: &'static str,
    username: &'static str,
    password: &'static str,
    secret: &'static str,
    namespace: &'static str,
}

#[derive(Clone)]
struct AppState {
    static_state: StaticState,
    mutable_state: MutableState,
    http_client: HttpClient,
    #[allow(dead_code)]
    matrix_client: MatrixClient,
    kube_client: KubeClient,
    room: Room,
    notifier: UnboundedSender<()>,
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
    namespace: &'static str,
}

async fn create_clients(
    &StaticState {
        matrix_hostname,
        username,
        password,
        secret,
        ..
    }: &StaticState,
) -> anyhow::Result<(MatrixClient, HttpClient)> {
    let ca_cert_path = std::env::var("MATRIX_CA_CERT")?;
    tracing::info!("Loading CA certificate from {}", ca_cert_path);
    let ca_cert_pem = fs::read(&ca_cert_path)?;
    let ca_cert = reqwest::Certificate::from_pem(&ca_cert_pem)?;

    let client = MatrixClient::builder()
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
        return Ok((client, http_client));
    }
    tracing::warn!("Matrix credentials are invalid, Creating account");

    create_account(
        matrix_hostname,
        secret,
        username,
        password,
        true,
        &http_client,
    )
    .await?;

    client
        .matrix_auth()
        .login_username(&username, &password)
        .send()
        .await?;

    Ok((client, http_client))
}

async fn create_account(
    matrix_hostname: &str,
    shared_secret: &str,
    username: &str,
    password: &str,
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
        .ok_or_else(|| anyhow!("No nonce in response"))?;
    tracing::debug!(?shared_secret, username, password, "Creating new user");
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
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                "controller_backend=debug,tower_http=debug,matrix_sdk=debug".into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    if let Err(e) = setup().await {
        tracing::error!(error=?e, "Fatal Error");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

async fn setup() -> anyhow::Result<()> {
    // Load environment variables
    let version = env!("CARGO_PKG_VERSION");

    let matrix_hostname: &'static str = std::env::var("MATRIX_HOSTNAME")?.leak();
    let vmware_gateway_hostname: &'static str = std::env::var("VMWARE_GATEWAY_HOSTNAME")?.leak();

    let matrix_username = "controller";
    let matrix_password: &'static str = std::env::var("MATRIX_PASSWORD")?.leak();
    let matrix_secret: &'static str = std::env::var("MATRIX_SECRET")?.leak();

    let namespace: &'static str = std::env::var("NAMESPACE")?.leak();

    let static_state = StaticState {
        version,
        matrix_hostname,
        vmware_gateway_hostname,
        username: matrix_username,
        password: matrix_password,
        secret: matrix_secret,
        namespace,
    };
    let (matrix_client, http_client) = create_clients(&static_state).await?;
    let kube_client = KubeClient::try_default().await?;

    let owned_room_id = RoomOrAliasId::parse(format!("#agent_room:{matrix_hostname}"))?;
    let owned_server_name = ServerName::parse(matrix_hostname)?;

    let room = create_room(&matrix_client, owned_room_id, &owned_server_name).await?;

    let api: Api<StatefulSet> = Api::namespaced(kube_client.clone(), namespace);
    let replica_count = get_replica_count(&api)
        .await
        .ok_or(anyhow!("Failed to get replica count"))?;

    let mutable_state = MutableState::new(replica_count);
    update_membership(&room, &mutable_state.room_members).await;
    for i in 0..5 {
        let username = format!("agent_{i}");
        let _ = create_account(
            matrix_hostname,
            matrix_secret,
            format!("agent_{i}").as_str(),
            format!("{matrix_password}_{i}").as_str(),
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
    let (notify_tx, notify_rx) = mpsc::unbounded_channel();
    let state = AppState {
        static_state,
        mutable_state,
        http_client,
        matrix_client,
        room,
        kube_client,
        notifier: notify_tx,
    };
    let token = CancellationToken::new();
    tracing::info!("Starting background job");

    let spawn_token = token.clone();
    let spawn_state = state.clone();
    let background_job = tokio::spawn(async move {
        spawn_token
            .run_until_cancelled(sync_matrix_room(notify_rx, spawn_state))
            .await;
    });

    tracing::info!("Seting up routes");
    let app = Router::new()
        .route("/api/v1/agents", get(list_agents))
        .route("/api/v1/vms", get(list_vms))
        .route("/api/v1/agent-assignments", get(list_agent_assignments))
        .route("/api/v1/agent-assignments", post(create_agent_assignment))
        .route("/api/v1/agent-assignments/:id", get(get_agent_assignment))
        .route(
            "/api/v1/agent-assignments/:id",
            put(update_agent_assignment),
        )
        .route(
            "/api/v1/agent-assignments/:id",
            delete(delete_agent_assignment),
        )
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
    let _ = serve.await;
    let _ = background_job.await;
    Ok(())
}

async fn create_room(
    client: &MatrixClient,
    owned_room_id: matrix_sdk::ruma::OwnedRoomOrAliasId,
    owned_server_name: &matrix_sdk::OwnedServerName,
) -> Result<Room, anyhow::Error> {
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
    Ok(room)
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
    tracing::info!(?members, "New member list");
    *room_members.write().await = members;
}

async fn status_handler(State(state): State<AppState>) -> Json<HealthResponse> {
    let AppState { static_state, .. } = state;
    let StaticState {
        version,
        matrix_hostname,
        vmware_gateway_hostname,
        username,
        namespace,
        ..
    } = static_state;
    Json(HealthResponse {
        version,
        vmware_gateway_hostname,
        matrix_hostname,
        username,
        namespace,
    })
}

// Agent handlers (Matrix room members)
async fn list_agents(State(state): State<AppState>) -> Json<Box<[MatrixUser]>> {
    let members = state.mutable_state.room_members.read().await;
    Json(members.clone())
}
