use std::time::Duration;

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use matrix_sdk::ruma::{
    UserId,
    events::{Mentions, room::message::RoomMessageEventContent},
};
use serde::{Deserialize, Serialize};
use tokio::{sync::mpsc::UnboundedReceiver, time::timeout};

use crate::{AppState, MutableState, update_membership};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskQueue {
    id: String,
    name: String,
    vm_name: String,
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
pub struct PostTaskQueue {
    name: String,
    vm_name: String,
    tasks: Vec<Task>,
    enabled: bool,
}

#[derive(Deserialize)]
pub struct PatchTaskQueue {
    name: Option<String>,
    vm_name: Option<String>,
    tasks: Option<Vec<Task>>,
    enabled: Option<bool>,
}
pub async fn sync_matrix_room(
    mut notifier_rx: UnboundedReceiver<()>,
    AppState {
        mutable_state:
            MutableState {
                agent_assignments,
                task_queues,
                room_members,
                ..
            },
        room,
        ..
    }: AppState,
) -> anyhow::Result<()> {
    loop {
        let _ = timeout(Duration::from_secs(600), notifier_rx.recv()).await;

        tracing::info!("Starting worker tasks");
        tracing::info_span!("Updating room members");
        update_membership(&room, &room_members).await;
        tracing::info!(?task_queues, "Processing task queues");
        for queue in task_queues
            .read()
            .await
            .iter()
            .filter(|queue| queue.enabled)
        {
            tracing::info!(?queue, "Enabled task");
            let Some(agent_name) = agent_assignments.read().await.iter().find_map(|i| {
                if i.vm_name == queue.vm_name {
                    Some(i.agent_name.clone())
                } else {
                    None
                }
            }) else {
                tracing::error!("No agent assigned to tasks for vm");
                continue;
            };
            let queue_message = build_prompt(queue, &agent_name);
            tracing::info!(message = queue_message);
            let Ok(user_id) = UserId::parse(&agent_name) else {
                tracing::error!(agent_name, "Failed to parse UserID");
                continue;
            };
            let message = RoomMessageEventContent::text_plain(&queue_message)
                .add_mentions(Mentions::with_user_ids([user_id]));
            // Create a proper Matrix mention so only the targeted agent responds
            if let Err(e) = room.send(message).await {
                tracing::error!(error=?e, "Error sending message");
            };
        }
    }
}

fn build_prompt(task_queue: &TaskQueue, agent_name: &str) -> String {
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
        "@{0}, I want you to perform the following task as described in the table below on the VM ({1}). To perform these tasks utilize the vmware gateway at the url http://vmware-gateway:80. To get a screen shot use the http://vmware-gateway/api/{1}/screen. To send keyboard inputs use http://vmware-gateway/api/{1}/keyboard?keys=<keys to send>. you should already know how to use that api based on tools in your context. you can use plain text to send the keys for that string. you can also use special keys like <super> and <enter> etc. if you need those. The description is a description of the task, and if there are keystrokes try using those to accomplish your task but feel free to adjust if they dont work.  Remember which tasks you complete and when you see this message again compare what you have already done with what you still need to do and pick up where you left off.  If all the tasks are already completed start at the top of the list and do them again.  If you get stuck use judgment to try to complete the task in spirit.  The main point is to persistently send commands to the vm and try to work through issues: \n|Description|Keystrokes|delay|\n|:---:|:---:|:---:|\n{2}",
        agent_name, task_queue.vm_name, rows
    )
}

pub async fn list_task_queues(State(state): State<AppState>) -> Json<Vec<TaskQueue>> {
    let queues = state.mutable_state.task_queues.read().await;
    Json(queues.clone())
}

pub async fn create_task_queue(
    State(state): State<AppState>,
    Json(payload): Json<PostTaskQueue>,
) -> (StatusCode, Json<TaskQueue>) {
    let now = chrono::Utc::now().to_rfc3339();
    let queue = TaskQueue {
        id: uuid::Uuid::new_v4().to_string(),
        vm_name: payload.vm_name,
        name: payload.name,
        tasks: payload.tasks,
        enabled: payload.enabled,
        created_at: now.clone(),
        updated_at: now,
    };

    let mut queues = state.mutable_state.task_queues.write().await;
    queues.push(queue.clone());
    let _ = state.notifier.send(());
    (StatusCode::CREATED, Json(queue))
}

pub async fn get_task_queue(
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

pub async fn update_task_queue(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(payload): Json<PatchTaskQueue>,
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
    if let Some(vm_id) = payload.vm_name {
        queue.vm_name = vm_id;
    }
    if let Some(enabled) = payload.enabled {
        queue.enabled = enabled;
    }
    queue.updated_at = chrono::Utc::now().to_rfc3339();

    let _ = state.notifier.send(());
    Ok(Json(queue.clone()))
}

pub async fn delete_task_queue(
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
