use axum::{Json, extract::State, http::StatusCode};

use serde::{Deserialize, Serialize};

use crate::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct VmList {
    vm_names: Vec<String>,
    count: usize,
    pattern: String,
}
// VMware VM handlers
pub async fn list_vms(State(state): State<AppState>) -> Result<Json<Vec<String>>, StatusCode> {
    let url = format!(
        "http://{}/api/vms",
        state.static_state.vmware_gateway_hostname
    );

    let response = state.http_client.get(&url).send().await.map_err(|e| {
        tracing::error!("Failed to fetch VMs from vmware_gateway: {}", e);
        StatusCode::BAD_GATEWAY
    })?;

    if !response.status().is_success() {
        tracing::error!("vmware_gateway returned status: {}", response.status());
        return Err(StatusCode::BAD_GATEWAY);
    }

    let vm_list: VmList = response.json().await.map_err(|e| {
        tracing::error!("Failed to parse vmware_gateway response: {}", e);
        StatusCode::BAD_GATEWAY
    })?;

    Ok(Json(vm_list.vm_names))
}
