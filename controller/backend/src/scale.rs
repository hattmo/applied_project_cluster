use axum::{Json, extract::State, http::StatusCode};
use k8s_openapi::api::apps::v1::StatefulSet;
use kube::{
    Api,
    api::{Patch, PatchParams},
};
use matrix_sdk::{ServerName, ruma::UserId};
use serde::{Deserialize, Serialize};

use crate::{AppState, get_replica_count};

#[derive(Serialize, Deserialize)]
pub struct AgentScale {
    replicas: i32,
}

pub async fn get_scale_agents(State(state): State<AppState>) -> Json<AgentScale> {
    Json(AgentScale {
        replicas: *state.mutable_state.replicas.read().await,
    })
}

pub async fn update_scale_agents(
    State(state): State<AppState>,
    Json(payload): Json<AgentScale>,
) -> Result<Json<AgentScale>, StatusCode> {
    let replicas = payload.replicas;
    if replicas < 1 || replicas > 5 {
        return Err(StatusCode::BAD_REQUEST);
    }

    let mut stored_replicas = state.mutable_state.replicas.write().await;
    let api: Api<StatefulSet> =
        Api::namespaced(state.kube_client.clone(), state.static_state.namespace);
    let current_replicas = get_replica_count(&api)
        .await
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    // Handle Matrix room membership changes
    let server_name = ServerName::parse(state.static_state.matrix_hostname).map_err(|e| {
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
    Ok(Json(AgentScale {
        replicas: new_replicas,
    }))
}
