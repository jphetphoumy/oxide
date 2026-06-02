use anyhow::{Context, Result, anyhow};
use reqwest::header::{ACCEPT, CONTENT_TYPE, HeaderMap, HeaderValue, USER_AGENT};
use tokio::sync::mpsc;

use crate::auth::{token_refresh, token_storage};
use crate::dust::stream::EventStream;
use crate::dust::types::{
    Conversation, ConversationMessage, CreateConversationRequest, CreateConversationResponse,
    Mention, MessageBody, MessageContext, PostMessageResponse, StreamEvent,
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
    Token(String),
    Complete(Option<String>),
    Error(String),
    ConversationCreated(String),
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

    pub async fn create_conversation(
        &self,
        message: &str,
        agent_id: &str,
    ) -> Result<CreateConversationResponse> {
        self.post_message_body(message, agent_id, None)
            .await
            .context("failed to create Dust conversation")
    }

    pub async fn post_message(
        &self,
        conversation_id: &str,
        message: &str,
        agent_id: &str,
    ) -> Result<PostMessageResponse> {
        self.post_message_body(message, agent_id, Some(conversation_id))
            .await
            .context("failed to post Dust message")
    }

    pub async fn stream_events(
        &self,
        conversation_id: &str,
        message_id: &str,
    ) -> Result<EventStream> {
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
            anyhow::bail!("Dust rejected the SSE stream request: HTTP {status} — {body}");
        }

        Ok(EventStream::new(response))
    }

    pub async fn send_message_flow(
        &self,
        conversation_id: Option<String>,
        content: String,
        tx: mpsc::UnboundedSender<DustEvent>,
    ) -> Result<()> {
        let response = match conversation_id {
            Some(existing) => {
                self.post_message(&existing, &content, &self.agent_id)
                    .await?
            }
            None => self.create_conversation(&content, &self.agent_id).await?,
        };

        let conversation_id = response.conversation.s_id.clone();
        let message_id = response
            .message
            .as_ref()
            .map(|message| message.s_id.as_str())
            .ok_or_else(|| anyhow!("Dust response did not include a message id"))?
            .to_string();

        let _ = tx.send(DustEvent::ConversationCreated(conversation_id.clone()));

        let agent_message_id = self
            .wait_for_agent_message(&conversation_id, &message_id)
            .await?;
        let mut stream = self
            .stream_events(&conversation_id, &agent_message_id)
            .await?;
        while let Some(event) = stream.next_event().await {
            match event? {
                StreamEvent::GenerationTokens {
                    text,
                    classification,
                } if classification == "tokens" => {
                    let _ = tx.send(DustEvent::Token(text));
                }
                StreamEvent::AgentMessageSuccess { message } => {
                    let _ = tx.send(DustEvent::Complete(message.content));
                    return Ok(());
                }
                StreamEvent::AgentError { error } => {
                    let _ = tx.send(DustEvent::Error(format!(
                        "Dust agent error: {}",
                        error.message
                    )));
                    return Ok(());
                }
                StreamEvent::UserMessageError { error } => {
                    let _ = tx.send(DustEvent::Error(format!(
                        "Dust message error: {}",
                        error.message
                    )));
                    return Ok(());
                }
                StreamEvent::AgentGenerationCancelled => {
                    let _ = tx.send(DustEvent::Complete(None));
                    return Ok(());
                }
                StreamEvent::GenerationTokens { .. }
                | StreamEvent::AgentActionSuccess { .. }
                | StreamEvent::Unknown => {}
            }
        }

        let _ = tx.send(DustEvent::Complete(None));
        Ok(())
    }

    async fn get_conversation(&self, conversation_id: &str) -> Result<Conversation> {
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
        for _ in 0..AGENT_MESSAGE_POLL_ATTEMPTS {
            let conversation = self.get_conversation(conversation_id).await?;
            if let Some(agent_message_id) = find_agent_message(&conversation, user_message_id)
                .or_else(|| latest_agent_message_id(&conversation))
            {
                return Ok(agent_message_id);
            }

            tokio::time::sleep(std::time::Duration::from_millis(
                AGENT_MESSAGE_POLL_INTERVAL_MS,
            ))
            .await;
        }

        Err(anyhow!("Failed to retrieve agent message"))
    }

    async fn post_message_body(
        &self,
        message: &str,
        agent_id: &str,
        conversation_id: Option<&str>,
    ) -> Result<CreateConversationResponse> {
        let body = CreateConversationRequest {
            title: conversation_id
                .is_none()
                .then(|| conversation_title(message)),
            visibility: DEFAULT_VISIBILITY.to_string(),
            message: MessageBody {
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
            },
        };

        let path = conversation_id.map_or_else(
            || format!("/api/v1/w/{}/assistant/conversations", self.workspace_id),
            |conversation_id| {
                format!(
                    "/api/v1/w/{}/assistant/conversations/{conversation_id}/messages",
                    self.workspace_id
                )
            },
        );

        let token = token_refresh::get_valid_token().await?;
        self.http
            .post(self.url(&path))
            .header(CONTENT_TYPE, "application/json")
            .bearer_auth(token)
            .json(&body)
            .send()
            .await
            .context("failed to send Dust message request")?
            .error_for_status()
            .context("Dust rejected the message request")?
            .json()
            .await
            .context("failed to decode Dust message response")
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
            } if parent_message_id.as_deref() == Some(user_message_id) => Some(s_id.clone()),
            _ => None,
        })
}

fn latest_agent_message_id(conversation: &Conversation) -> Option<String> {
    conversation
        .content
        .iter()
        .rev()
        .flat_map(|group| group.iter().rev())
        .find_map(|message| match message {
            ConversationMessage::AgentMessage { s_id, .. } => Some(s_id.clone()),
            ConversationMessage::Other => None,
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
    fn latest_agent_message_id_picks_last_agent_message() {
        let conversation = Conversation {
            s_id: "c1".to_string(),
            content: vec![
                vec![ConversationMessage::Other],
                vec![ConversationMessage::AgentMessage {
                    s_id: "agent2".to_string(),
                    parent_message_id: None,
                }],
            ],
        };

        assert_eq!(
            latest_agent_message_id(&conversation),
            Some("agent2".to_string())
        );
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
}
