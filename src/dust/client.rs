use anyhow::{Context, Result, anyhow};
use reqwest::header::{ACCEPT, CONTENT_TYPE, HeaderMap, HeaderValue, USER_AGENT};
use serde::de::DeserializeOwned;
use tokio::sync::mpsc;
use tracing::{debug, error, trace};

use crate::auth::{token_refresh, token_storage};
use crate::dust::stream::EventStream;
use crate::dust::types::{
    AgentInfo, Conversation, ConversationMessage, ConversationSummary, CreateConversationRequest,
    CreateConversationResponse, ListAgentsResponse, ListConversationsResponse, Mention,
    MessageBody, MessageContext, PostMessageResponse, StreamEvent,
};

pub const DUST_CLI_USER_AGENT: &str = "Dust CLI";
pub const DUST_CLI_VERSION: &str = "0.4.5";
const DUST_CLI_VERSION_HEADER: &str = "X-Dust-CLI-Version";
pub const DEFAULT_AGENT_ID: &str = "dust";
const DEFAULT_BASE_URL: &str = "https://dust.tt";
const DEFAULT_VISIBILITY: &str = "unlisted";
const DEFAULT_ORIGIN: &str = "cli";
const AGENT_MESSAGE_POLL_ATTEMPTS: usize = 100;
const AGENT_MESSAGE_POLL_INTERVAL_MS: u64 = 300;

#[derive(Debug, Clone)]
pub struct DustClient {
    http: reqwest::Client,
    base_url: String,
    workspace_id: String,
    agent_id: String,
    user_context: UserContext,
}

#[derive(Debug, Clone)]
pub struct UserContext {
    pub timezone: String,
    pub username: String,
    pub email: Option<String>,
    pub full_name: Option<String>,
}

#[derive(Debug, Clone)]
pub enum DustEvent {
    Token(String, Option<String>),            // text, conversation_id
    Complete(Option<String>, Option<String>), // content, conversation_id
    Error(String),
    ConversationCreated(String),
    UserMessageCreated(String), // user_message_id for stream resumption
    ConversationsListed(Vec<ConversationSummary>),
    ConversationLoaded {
        conversation_id: String,
        title: Option<String>,
        messages: Vec<(String, String)>, // (role, content) where role is "user" | "agent" | "system"
    },
    ToolUse(crate::mcp::ToolCall),
}

#[derive(Debug, serde::Deserialize)]
struct ConversationWrapper {
    conversation: Conversation,
}

impl UserContext {
    pub fn from_env() -> Self {
        let username = std::env::var("OXIDE_USERNAME")
            .ok()
            .or_else(|| std::env::var("USER").ok())
            .or_else(|| std::env::var("USERNAME").ok())
            .unwrap_or_else(|| "oxide".to_string());

        Self {
            timezone: std::env::var("TZ").unwrap_or_else(|_| "UTC".to_string()),
            username,
            email: std::env::var("OXIDE_EMAIL").ok(),
            full_name: std::env::var("OXIDE_FULL_NAME").ok(),
        }
    }
}

impl DustClient {
    pub fn from_env() -> Result<Self> {
        let workspace_id = token_storage::get_workspace_id()?
            .ok_or_else(|| anyhow!("No workspace selected. Run `oxide login` first."))?;
        let region = token_storage::get_region()?.unwrap_or_else(|| "us-central1".to_string());
        let config = crate::config::Config::load()?;
        let agent_id = resolve_agent_id(config.agent_id(), std::env::var("OXIDE_AGENT_ID").ok());

        Self::new(
            base_url_for_region(&region),
            workspace_id,
            agent_id,
            UserContext::from_env(),
        )
    }

    pub fn new(
        base_url: String,
        workspace_id: String,
        agent_id: String,
        user_context: UserContext,
    ) -> Result<Self> {
        let http = build_http_client()?;

        Ok(Self {
            http,
            base_url,
            workspace_id,
            agent_id,
            user_context,
        })
    }

    #[cfg(test)]
    pub fn agent_id(&self) -> &str {
        &self.agent_id
    }

    pub fn set_agent(&mut self, agent_id: impl Into<String>) {
        self.agent_id = agent_id.into();
    }

    pub fn list_agents_url(&self) -> String {
        self.url(&format!(
            "/api/v1/w/{}/assistant/agent_configurations?view=list",
            self.workspace_id
        ))
    }

    pub async fn list_agents(&self) -> Result<Vec<AgentInfo>> {
        let token = token_refresh::get_valid_token().await?;
        let response = self
            .http
            .get(self.list_agents_url())
            .bearer_auth(token)
            .send()
            .await
            .context("failed to list Dust agents")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Dust rejected agent list request: HTTP {status} — {body}");
        }

        let body: ListAgentsResponse = response
            .json()
            .await
            .context("failed to decode Dust agent list response")?;

        Ok(body.agent_configurations)
    }

    pub async fn list_conversations(&self) -> Result<Vec<ConversationSummary>> {
        let token = token_refresh::get_valid_token().await?;
        let url = self.url(&format!(
            "/api/w/{}/assistant/conversations",
            self.workspace_id
        ));
        let response = self
            .http
            .get(&url)
            .bearer_auth(token)
            .send()
            .await
            .context("failed to list Dust conversations")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Dust rejected conversations list request: HTTP {status} — {body}");
        }

        let body: ListConversationsResponse = response
            .json()
            .await
            .context("failed to decode Dust conversations list response")?;

        let mut conversations = body.conversations;
        conversations.sort_by(|a, b| {
            b.updated
                .unwrap_or(b.created)
                .cmp(&a.updated.unwrap_or(a.created))
        });
        Ok(conversations)
    }

    pub async fn create_conversation(
        &self,
        message: &str,
        agent_id: &str,
    ) -> Result<CreateConversationResponse> {
        debug!(
            agent_id = %agent_id,
            message_len = message.len(),
            "creating Dust conversation"
        );
        self.create_conversation_body(message, agent_id)
            .await
            .context("failed to create Dust conversation")
    }

    pub async fn post_message(
        &self,
        conversation_id: &str,
        message: &str,
        agent_id: &str,
    ) -> Result<String> {
        debug!(
            conversation_id = %conversation_id,
            agent_id = %agent_id,
            message_len = message.len(),
            "posting Dust follow-up message"
        );
        let response: PostMessageResponse = self
            .post_message_body(conversation_id, message, agent_id)
            .await
            .context("failed to post Dust message")?;
        Ok(response.message.s_id)
    }

    pub async fn stream_events(
        &self,
        conversation_id: &str,
        message_id: &str,
    ) -> Result<EventStream> {
        debug!(
            conversation_id = %conversation_id,
            message_id = %message_id,
            "opening Dust SSE stream"
        );
        let token = token_refresh::get_valid_token().await?;
        let response = self
            .http
            .get(self.sse_url(&format!(
                "assistant/conversations/{conversation_id}/messages/{message_id}/events"
            )))
            .header(ACCEPT, "text/event-stream")
            .bearer_auth(token)
            .send()
            .await
            .context("failed to start Dust SSE stream")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            error!(
                conversation_id = %conversation_id,
                message_id = %message_id,
                %status,
                body = %body,
                "Dust rejected the SSE stream request"
            );
            anyhow::bail!("Dust rejected the SSE stream request: HTTP {status} — {body}");
        }

        Ok(EventStream::new(response))
    }

    pub async fn submit_tool_result(
        &self,
        conversation_id: &str,
        tool_result: &crate::mcp::ToolResult,
    ) -> Result<()> {
        let token = token_refresh::get_valid_token().await?;
        let body = serde_json::json!({
            "tool_use_id": tool_result.tool_use_id,
            "content": [
                {
                    "type": "text",
                    "text": tool_result.content
                }
            ]
        });

        let path = &format!(
            "/api/v1/w/{}/assistant/conversations/{conversation_id}/tool_results",
            self.workspace_id
        );

        debug!(
            conversation_id = %conversation_id,
            tool_use_id = %tool_result.tool_use_id,
            "submitting Dust tool result"
        );

        let response = self
            .http
            .post(self.url(path))
            .header(CONTENT_TYPE, "application/json")
            .bearer_auth(token)
            .json(&body)
            .send()
            .await
            .context("failed to submit tool result")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Dust rejected tool result submission: HTTP {status} — {body}");
        }

        Ok(())
    }

    pub async fn resume_message_stream(
        &self,
        conversation_id: &str,
        user_message_id: &str,
        tx: mpsc::UnboundedSender<DustEvent>,
    ) -> Result<()> {
        debug!(
            conversation_id = %conversation_id,
            user_message_id = %user_message_id,
            "resuming message stream after tool execution"
        );

        let agent_message_id = self
            .wait_for_agent_message(conversation_id, user_message_id)
            .await?;

        debug!(
            agent_message_id = %agent_message_id,
            "found next agent message, resuming stream"
        );

        let mut stream = self
            .stream_events(conversation_id, &agent_message_id)
            .await?;

        while let Some(event) = stream.next_event().await {
            match event? {
                StreamEvent::GenerationTokens {
                    text,
                    classification,
                } if classification == "tokens" => {
                    let _ = tx.send(DustEvent::Token(text, Some(conversation_id.to_string())));
                }
                StreamEvent::AgentMessageSuccess { message } => {
                    debug!("resumed stream: agent message completed");
                    let _ = tx.send(DustEvent::Complete(
                        message.content,
                        Some(conversation_id.to_string()),
                    ));
                    return Ok(());
                }
                StreamEvent::AgentError { error } => {
                    let _ = tx.send(DustEvent::Error(format!(
                        "Dust agent error (after tool): {}",
                        error.message
                    )));
                    return Ok(());
                }
                StreamEvent::AgentActionSuccess { action } => {
                    if let Some(tool_call) = StreamEvent::extract_tool_use_from_action(&action) {
                        let _ = tx.send(DustEvent::ToolUse(tool_call));
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    pub async fn send_message_flow(
        &self,
        conversation_id: Option<String>,
        content: String,
        tools: Vec<crate::mcp::McpTool>,
        tx: mpsc::UnboundedSender<DustEvent>,
    ) -> Result<()> {
        debug!(
            existing_conversation = conversation_id.as_deref().unwrap_or("<new>"),
            content_len = content.len(),
            tools_count = tools.len(),
            "starting Dust message flow"
        );
        for tool in &tools {
            debug!(tool_name = %tool.name, "tool available in message flow");
        }
        let (user_message_id, conversation_id) = if let Some(existing) = conversation_id {
            let user_message_id = self
                .post_message_with_tools(&existing, &content, &self.agent_id, tools.clone())
                .await?;
            (user_message_id, existing)
        } else {
            let created = self
                .create_conversation_with_tools(&content, &self.agent_id, tools)
                .await?;
            let conversation_id = created.conversation.s_id.clone();
            let user_message_id = created
                .message
                .as_ref()
                .map(|message| message.s_id.clone())
                .ok_or_else(|| anyhow!("Dust response did not include a message id"))?;
            (user_message_id, conversation_id)
        };

        debug!(
            conversation_id = %conversation_id,
            user_message_id = %user_message_id,
            "Dust conversation is ready"
        );
        let _ = tx.send(DustEvent::ConversationCreated(conversation_id.clone()));
        let _ = tx.send(DustEvent::UserMessageCreated(user_message_id.clone()));

        let agent_message_id = self
            .wait_for_agent_message(&conversation_id, &user_message_id)
            .await?;
        debug!(
            conversation_id = %conversation_id,
            agent_message_id = %agent_message_id,
            "Dust agent message is ready"
        );
        let mut stream = self
            .stream_events(&conversation_id, &agent_message_id)
            .await?;
        while let Some(event) = stream.next_event().await {
            match event? {
                StreamEvent::GenerationTokens {
                    text,
                    classification,
                } if classification == "tokens" => {
                    trace!(
                        conversation_id = %conversation_id,
                        token_len = text.len(),
                        "received Dust token chunk"
                    );
                    let _ = tx.send(DustEvent::Token(text, Some(conversation_id.clone())));
                }
                StreamEvent::AgentMessageSuccess { message } => {
                    debug!(
                        conversation_id = %conversation_id,
                        "Dust agent message completed"
                    );
                    let _ = tx.send(DustEvent::Complete(
                        message.content,
                        Some(conversation_id.clone()),
                    ));
                    return Ok(());
                }
                StreamEvent::AgentError { error } => {
                    error!(
                        conversation_id = %conversation_id,
                        code = ?error.code,
                        message = %error.message,
                        "Dust agent error"
                    );
                    let _ = tx.send(DustEvent::Error(format!(
                        "Dust agent error: {}",
                        error.message
                    )));
                    return Ok(());
                }
                StreamEvent::UserMessageError { error } => {
                    error!(
                        conversation_id = %conversation_id,
                        code = ?error.code,
                        message = %error.message,
                        "Dust user message error"
                    );
                    let _ = tx.send(DustEvent::Error(format!(
                        "Dust message error: {}",
                        error.message
                    )));
                    return Ok(());
                }
                StreamEvent::AgentGenerationCancelled => {
                    debug!(
                        conversation_id = %conversation_id,
                        "Dust agent generation cancelled"
                    );
                    let _ = tx.send(DustEvent::Complete(None, Some(conversation_id.clone())));
                    return Ok(());
                }
                StreamEvent::AgentActionSuccess { action } => {
                    if let Some(tool_call) = StreamEvent::extract_tool_use_from_action(&action) {
                        debug!(
                            tool_name = %tool_call.name,
                            tool_id = %tool_call.id,
                            "received tool_use action from agent"
                        );
                        let _ = tx.send(DustEvent::ToolUse(tool_call));
                    }
                }
                StreamEvent::GenerationTokens { .. } | StreamEvent::Unknown => {}
            }
        }

        debug!(
            conversation_id = %conversation_id,
            "Dust stream ended without a terminal event"
        );
        let _ = tx.send(DustEvent::Complete(None, Some(conversation_id.clone())));
        Ok(())
    }

    pub async fn get_conversation(&self, conversation_id: &str) -> Result<Conversation> {
        let token = token_refresh::get_valid_token().await?;
        let response = self
            .http
            .get(self.url(&format!(
                "/api/v1/w/{}/assistant/conversations/{conversation_id}",
                self.workspace_id
            )))
            .bearer_auth(token)
            .send()
            .await
            .context("failed to get Dust conversation")?
            .error_for_status()
            .context("Dust rejected the conversation request")?;

        let body = response
            .text()
            .await
            .context("failed to read Dust conversation response")?;
        if let Ok(wrapper) = serde_json::from_str::<ConversationWrapper>(&body) {
            return Ok(wrapper.conversation);
        }

        serde_json::from_str(&body).context("failed to decode Dust conversation")
    }

    async fn wait_for_agent_message(
        &self,
        conversation_id: &str,
        user_message_id: &str,
    ) -> Result<String> {
        // Dust needs to stamp the agent reply with parentMessageId before we can stream it.
        for _ in 0..AGENT_MESSAGE_POLL_ATTEMPTS {
            let conversation = self.get_conversation(conversation_id).await?;
            if let Some(agent_message_id) = find_agent_message(&conversation, user_message_id) {
                return Ok(agent_message_id);
            }

            tokio::time::sleep(std::time::Duration::from_millis(
                AGENT_MESSAGE_POLL_INTERVAL_MS,
            ))
            .await;
        }

        let total_wait_secs = std::time::Duration::from_millis(
            AGENT_MESSAGE_POLL_ATTEMPTS as u64 * AGENT_MESSAGE_POLL_INTERVAL_MS,
        )
        .as_secs_f64();
        Err(anyhow!(
            "Timed out waiting for agent message after {AGENT_MESSAGE_POLL_ATTEMPTS} attempts ({total_wait_secs:.1}s)"
        ))
    }

    async fn create_conversation_body(
        &self,
        message: &str,
        agent_id: &str,
    ) -> Result<CreateConversationResponse> {
        let body = CreateConversationRequest {
            title: Some(conversation_title(message)),
            visibility: DEFAULT_VISIBILITY.to_string(),
            message: self.message_body(message, agent_id),
        };

        self.send_message_request(
            &format!("/api/v1/w/{}/assistant/conversations", self.workspace_id),
            "create conversation",
            &body,
        )
        .await
    }

    async fn post_message_body(
        &self,
        conversation_id: &str,
        message: &str,
        agent_id: &str,
    ) -> Result<PostMessageResponse> {
        let body = self.message_body(message, agent_id);

        self.send_message_request(
            &format!(
                "/api/v1/w/{}/assistant/conversations/{conversation_id}/messages",
                self.workspace_id
            ),
            "post message",
            &body,
        )
        .await
    }

    async fn post_message_with_tools(
        &self,
        conversation_id: &str,
        message: &str,
        agent_id: &str,
        tools: Vec<crate::mcp::McpTool>,
    ) -> Result<String> {
        let body = self.message_body_with_tools(
            message,
            agent_id,
            if tools.is_empty() { None } else { Some(tools) },
        );

        let response: PostMessageResponse = self
            .send_message_request(
                &format!(
                    "/api/v1/w/{}/assistant/conversations/{conversation_id}/messages",
                    self.workspace_id
                ),
                "post message",
                &body,
            )
            .await?;
        Ok(response.message.s_id)
    }

    async fn create_conversation_with_tools(
        &self,
        message: &str,
        agent_id: &str,
        tools: Vec<crate::mcp::McpTool>,
    ) -> Result<CreateConversationResponse> {
        let body = CreateConversationRequest {
            title: Some(conversation_title(message)),
            visibility: DEFAULT_VISIBILITY.to_string(),
            message: self.message_body_with_tools(
                message,
                agent_id,
                if tools.is_empty() { None } else { Some(tools) },
            ),
        };

        self.send_message_request(
            &format!("/api/v1/w/{}/assistant/conversations", self.workspace_id),
            "create conversation",
            &body,
        )
        .await
    }

    fn message_body(&self, message: &str, agent_id: &str) -> MessageBody {
        self.message_body_with_tools(message, agent_id, None)
    }

    fn message_body_with_tools(
        &self,
        message: &str,
        agent_id: &str,
        tools: Option<Vec<crate::mcp::McpTool>>,
    ) -> MessageBody {
        if let Some(ref tool_list) = tools {
            eprintln!("[OXIDE-CLIENT] Including {} tools in message body", tool_list.len());
            for tool in tool_list {
                eprintln!("[OXIDE-CLIENT] Tool in body: {}", tool.name);
            }
            tracing::info!(
                tool_count = tool_list.len(),
                tools = ?tool_list.iter().map(|t| &t.name).collect::<Vec<_>>(),
                "including tools in message body"
            );
        } else {
            eprintln!("[OXIDE-CLIENT] No tools in message body");
            tracing::info!("no tools in message body");
        }
        MessageBody {
            content: message.to_string(),
            mentions: vec![Mention {
                configuration_id: agent_id.to_string(),
            }],
            context: MessageContext {
                timezone: self.user_context.timezone.clone(),
                username: self.user_context.username.clone(),
                origin: DEFAULT_ORIGIN.to_string(),
                email: self.user_context.email.clone(),
                full_name: self.user_context.full_name.clone(),
            },
            tools,
        }
    }

    async fn send_message_request<T, R>(&self, path: &str, action: &str, body: &T) -> Result<R>
    where
        T: serde::Serialize + Sync,
        R: DeserializeOwned,
    {
        let token = token_refresh::get_valid_token().await?;
        let body_json =
            serde_json::to_string(body).context("failed to serialize Dust request body")?;
        debug!(path = %path, action = %action, body = %body_json, "sending Dust request");
        let response = self
            .http
            .post(self.url(path))
            .header(CONTENT_TYPE, "application/json")
            .bearer_auth(token)
            .body(body_json)
            .send()
            .await;

        let response = match response {
            Ok(response) => response,
            Err(error) => {
                error!(path = %path, action = %action, error = %error, "Dust request send failed");
                return Err(error).with_context(|| format!("failed to send Dust {action} request"));
            }
        };

        let status = response.status();
        let body_text = response.text().await;

        let body_text = match body_text {
            Ok(body_text) => body_text,
            Err(error) => {
                error!(path = %path, action = %action, error = %error, "Dust response read failed");
                return Err(error)
                    .with_context(|| format!("failed to read Dust {action} response"));
            }
        };

        if !status.is_success() {
            error!(
                path = %path,
                %status,
                body = %body_text,
                "Dust request rejected"
            );
            anyhow::bail!("Dust rejected the {action} request: HTTP {status} — {body_text}");
        }

        debug!(path = %path, %status, "Dust request succeeded");
        serde_json::from_str(&body_text)
            .with_context(|| format!("failed to decode Dust {action} response"))
    }

    fn url(&self, path: &str) -> String {
        format!(
            "{}/{}",
            self.base_url.trim_end_matches('/'),
            path.trim_start_matches('/')
        )
    }

    fn sse_url(&self, path: &str) -> String {
        format!(
            "{}/api/sse/v1/w/{}/{}",
            self.base_url.trim_end_matches('/'),
            self.workspace_id,
            path.trim_start_matches('/')
        )
    }
}

fn conversation_title(message: &str) -> String {
    // Count Unicode scalar values so the prefix matches "first 30 chars"
    // rather than "first 30 bytes".
    let prefix: String = message.chars().take(30).collect();
    format!(
        "CLI Question: {}{}",
        prefix,
        if message.chars().count() > 30 {
            "..."
        } else {
            ""
        }
    )
}

pub fn base_url_for_region(region: &str) -> String {
    match region {
        "europe-west1" => "https://eu.dust.tt".to_string(),
        _ => DEFAULT_BASE_URL.to_string(),
    }
}

pub fn resolve_agent_id(config_value: Option<&str>, env_value: Option<String>) -> String {
    env_value
        .or_else(|| config_value.map(ToOwned::to_owned))
        .unwrap_or_else(|| DEFAULT_AGENT_ID.to_string())
}

pub fn build_http_client() -> Result<reqwest::Client> {
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static(DUST_CLI_USER_AGENT));
    headers.insert(
        DUST_CLI_VERSION_HEADER,
        HeaderValue::from_static(DUST_CLI_VERSION),
    );
    reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .context("failed to build Dust HTTP client")
}

fn find_agent_message(conversation: &Conversation, user_message_id: &str) -> Option<String> {
    conversation
        .content
        .iter()
        .flat_map(|group| group.iter())
        .find_map(|message| match message {
            ConversationMessage::AgentMessage {
                s_id,
                parent_message_id,
                ..
            } if parent_message_id.as_deref() == Some(user_message_id) => Some(s_id.clone()),
            _ => None,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn region_maps_to_base_url() {
        assert_eq!(base_url_for_region("us-central1"), "https://dust.tt");
        assert_eq!(base_url_for_region("europe-west1"), "https://eu.dust.tt");
    }

    #[test]
    fn sse_url_uses_sse_prefix() {
        let client = DustClient::new(
            "https://dust.tt".to_string(),
            "ws_123".to_string(),
            DEFAULT_AGENT_ID.to_string(),
            UserContext::from_env(),
        )
        .expect("build client");

        assert_eq!(
            client.sse_url("assistant/conversations/c1/messages/m1/events"),
            "https://dust.tt/api/sse/v1/w/ws_123/assistant/conversations/c1/messages/m1/events"
        );
    }

    #[test]
    fn agent_id_defaults_to_dust() {
        assert_eq!(resolve_agent_id(None, None), DEFAULT_AGENT_ID);
    }

    #[test]
    fn agent_id_uses_env_override_when_present() {
        assert_eq!(resolve_agent_id(Some("config-agent"), None), "config-agent");
        assert_eq!(
            resolve_agent_id(Some("config-agent"), Some("custom-agent".to_string())),
            "custom-agent"
        );
    }

    #[test]
    fn find_agent_message_matches_parent_message_id() {
        let conversation = Conversation {
            s_id: "c1".to_string(),
            content: vec![vec![
                ConversationMessage::AgentMessage {
                    s_id: "agent1".to_string(),
                    parent_message_id: Some("user1".to_string()),
                    content: Some("reply".to_string()),
                },
                ConversationMessage::Other,
            ]],
        };

        assert_eq!(
            find_agent_message(&conversation, "user1"),
            Some("agent1".to_string())
        );
    }

    #[test]
    fn list_agents_url_is_correct() {
        let client = DustClient::new(
            "https://dust.tt".to_string(),
            "ws_123".to_string(),
            DEFAULT_AGENT_ID.to_string(),
            UserContext::from_env(),
        )
        .expect("build client");

        assert_eq!(
            client.list_agents_url(),
            "https://dust.tt/api/v1/w/ws_123/assistant/agent_configurations?view=list"
        );
    }

    #[test]
    fn agent_id_accessor() {
        let client = DustClient::new(
            "https://dust.tt".to_string(),
            "ws_123".to_string(),
            "my-agent".to_string(),
            UserContext::from_env(),
        )
        .expect("build client");

        assert_eq!(client.agent_id(), "my-agent");
    }

    #[test]
    fn set_agent_updates_id() {
        let mut client = DustClient::new(
            "https://dust.tt".to_string(),
            "ws_123".to_string(),
            "old-agent".to_string(),
            UserContext::from_env(),
        )
        .expect("build client");

        client.set_agent("new-agent");
        assert_eq!(client.agent_id(), "new-agent");
    }

    #[test]
    fn conversation_title_prefix_matches_dust_cli() {
        assert_eq!(
            conversation_title("hello"),
            "CLI Question: hello".to_string()
        );
        assert_eq!(
            conversation_title("abcdefghijklmnopqrstuvwxyz1234567890"),
            "CLI Question: abcdefghijklmnopqrstuvwxyz1234..."
        );
        let unicode = "é".repeat(31);
        let expected = format!("CLI Question: {}...", "é".repeat(30));
        assert_eq!(conversation_title(&unicode), expected);
    }

    #[test]
    fn list_conversations_url_is_correct() {
        let client = DustClient::new(
            "https://dust.tt".to_string(),
            "ws_123".to_string(),
            DEFAULT_AGENT_ID.to_string(),
            UserContext::from_env(),
        )
        .expect("build client");

        let url = client.url(&format!(
            "/api/w/{}/assistant/conversations",
            client.workspace_id
        ));
        assert_eq!(url, "https://dust.tt/api/w/ws_123/assistant/conversations");
    }

    #[test]
    fn conversations_sorted_newest_first() {
        use crate::dust::types::ConversationSummary;

        let mut convs = vec![
            ConversationSummary {
                s_id: "c1".into(),
                title: Some("oldest".into()),
                created: 1000,
                updated: Some(1000),
            },
            ConversationSummary {
                s_id: "c2".into(),
                title: Some("newest".into()),
                created: 3000,
                updated: Some(3000),
            },
            ConversationSummary {
                s_id: "c3".into(),
                title: Some("middle".into()),
                created: 2000,
                updated: Some(2000),
            },
        ];

        convs.sort_by(|a, b| {
            b.updated
                .unwrap_or(b.created)
                .cmp(&a.updated.unwrap_or(a.created))
        });

        assert_eq!(convs[0].s_id, "c2"); // newest
        assert_eq!(convs[1].s_id, "c3"); // middle
        assert_eq!(convs[2].s_id, "c1"); // oldest
    }
}
