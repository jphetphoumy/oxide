use anyhow::{Result, anyhow};
use std::time::Duration;
use tokio::sync::mpsc;

use crate::dust::client::{DustClient, DustEvent};

/// Maximum nesting depth for local `oxide_agent` calls.
pub const MAX_SUBAGENT_DEPTH: u32 = 3;

/// Timeout for subagent execution in seconds.
const SUBAGENT_TIMEOUT_SECS: u64 = 120;

/// Run a one-shot conversation with a Dust agent and collect the full response.
///
/// - Creates a fresh conversation (no reuse of the caller's conversation).
/// - Buffers tokens; does not forward streaming events to the TUI.
/// - Returns the complete agent response text on success.
/// - `depth` is the current nesting level (0 = top-level call from user session).
///   If `depth` >= `MAX_SUBAGENT_DEPTH`, returns an error immediately.
pub async fn run_subagent(
    client: &DustClient,
    prompt: String,
    agent_id: Option<String>,
    depth: u32,
) -> Result<String> {
    if depth >= MAX_SUBAGENT_DEPTH {
        return Err(anyhow!(
            "oxide_agent max depth ({MAX_SUBAGENT_DEPTH}) exceeded — cannot spawn further subagents",
        ));
    }

    // Clone the client and optionally override the agent ID.
    let mut subagent_client = client.clone();
    if let Some(id) = agent_id {
        subagent_client.set_agent(id);
    }

    // Use an unbounded channel — the subagent runner is the only sender.
    let (tx, mut rx) = mpsc::unbounded_channel::<DustEvent>();

    // Run the full message flow in the current task (already inside a tokio::spawn).
    // Pass empty active_skills — subagent conversations never inherit parent skills.
    let flow = subagent_client
        .send_message_flow_with_skills(None, prompt, tx, &[])
        .await;

    // Drain all events from the channel (send_message_flow is done by now).
    let mut response_text = String::new();
    while let Ok(event) = rx.try_recv() {
        match event {
            DustEvent::Complete(Some(content), _) => {
                response_text = content;
            }
            DustEvent::Error(e) => {
                return Err(anyhow!("subagent error: {e}"));
            }
            DustEvent::Token(token, _) => {
                // Fallback: accumulate tokens if Complete doesn't include full content.
                response_text.push_str(&token);
            }
            _ => {}
        }
    }

    // Propagate any transport-level error from the flow itself.
    flow?;

    if response_text.is_empty() {
        return Err(anyhow!("subagent returned empty response"));
    }

    Ok(response_text)
}

/// Helper to run a subagent with a timeout wrapper.
pub async fn run_subagent_with_timeout(
    client: &DustClient,
    prompt: String,
    agent_id: Option<String>,
    depth: u32,
) -> Result<String> {
    tokio::time::timeout(
        Duration::from_secs(SUBAGENT_TIMEOUT_SECS),
        run_subagent(client, prompt, agent_id, depth),
    )
    .await
    .map_err(|_| anyhow!("subagent timed out after {SUBAGENT_TIMEOUT_SECS}s"))?
}
