use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};

use crate::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentAssignment {
    id: String,
    pub agent_name: String,
    pub vm_name: String,
    pub enabled: bool,
    created_at: String,
    updated_at: String,
}

#[derive(Deserialize)]
pub struct PostAgentAssignment {
    agent_name: String,
    vm_name: String,
    enabled: bool,
}
#[derive(Deserialize)]
pub struct PatchAgentAssignment {
    agent_name: Option<String>,
    vm_name: Option<String>,
    enabled: Option<bool>,
}

// VM Config handlers
pub async fn list_agent_assignments(State(state): State<AppState>) -> Json<Vec<AgentAssignment>> {
    let configs = state.mutable_state.agent_assignments.read().await;
    Json(configs.clone())
}

pub async fn create_agent_assignment(
    State(state): State<AppState>,
    Json(PostAgentAssignment {
        agent_name,
        vm_name,
        enabled,
    }): Json<PostAgentAssignment>,
) -> (StatusCode, Json<AgentAssignment>) {
    let now = chrono::Utc::now().to_rfc3339();
    let config = AgentAssignment {
        id: uuid::Uuid::new_v4().to_string(),
        created_at: now.clone(),
        updated_at: now,
        vm_name,
        agent_name,
        enabled,
    };
    state
        .mutable_state
        .agent_assignments
        .write()
        .await
        .push(config.clone());
    (StatusCode::CREATED, Json(config))
}

pub async fn get_agent_assignment(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<AgentAssignment>, StatusCode> {
    let configs = state.mutable_state.agent_assignments.read().await;
    configs
        .iter()
        .find(|c| c.id == id)
        .map(|c| Json(c.clone()))
        .ok_or(StatusCode::NOT_FOUND)
}

pub async fn update_agent_assignment(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(payload): Json<PatchAgentAssignment>,
) -> Result<Json<AgentAssignment>, StatusCode> {
    let mut configs = state.mutable_state.agent_assignments.write().await;
    let config = configs
        .iter_mut()
        .find(|c| c.id == id)
        .ok_or(StatusCode::NOT_FOUND)?;

    if let Some(v) = payload.agent_name {
        config.agent_name = v;
    }
    if let Some(v) = payload.vm_name {
        config.vm_name = v;
    }
    if let Some(v) = payload.enabled {
        config.enabled = v;
    }
    config.updated_at = chrono::Utc::now().to_rfc3339();

    Ok(Json(config.clone()))
}

pub async fn delete_agent_assignment(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    let mut configs = state.mutable_state.agent_assignments.write().await;
    let pos = configs
        .iter()
        .position(|c| c.id == id)
        .ok_or(StatusCode::NOT_FOUND)?;
    configs.remove(pos);
    Ok(StatusCode::NO_CONTENT)
}
