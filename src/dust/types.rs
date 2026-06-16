use serde::{Deserialize, Serialize};

use crate::mcp::ToolCall;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageContext {
    pub timezone: String,
    pub username: String,
    pub origin: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_name: Option<String>,
    #[serde(
        rename = "clientSideMCPServerIds",
        skip_serializing_if = "Option::is_none"
    )]
    pub client_side_mcp_server_ids: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Mention {
    #[serde(rename = "configurationId")]
    pub configuration_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MessageBody {
    pub content: String,
    pub mentions: Vec<Mention>,
    pub context: MessageContext,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CreateConversationRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub visibility: String,
    pub message: MessageBody,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ConversationRef {
    #[serde(rename = "sId")]
    pub s_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct MessageRef {
    #[serde(rename = "sId")]
    pub s_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CreateConversationResponse {
    pub conversation: ConversationRef,
    #[serde(default)]
    pub message: Option<MessageRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
// POST /messages always returns the created message.
pub struct PostMessageResponse {
    pub message: MessageRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AgentMessage {
    #[serde(default)]
    pub content: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Conversation {
    #[serde(rename = "sId")]
    pub s_id: String,
    #[serde(default)]
    pub content: Vec<Vec<ConversationMessage>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ConversationSummary {
    #[serde(rename = "sId")]
    pub s_id: String,
    #[serde(default)]
    pub title: Option<String>,
    pub created: i64,
    #[serde(default)]
    pub updated: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListConversationsResponse {
    pub conversations: Vec<ConversationSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "type")]
pub enum ConversationMessage {
    #[serde(rename = "agent_message")]
    AgentMessage {
        #[serde(rename = "sId")]
        s_id: String,
        #[serde(rename = "parentMessageId")]
        parent_message_id: Option<String>,
        #[serde(default)]
        content: Option<String>,
    },
    #[serde(rename = "user_message")]
    UserMessage { content: String },
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct StreamError {
    #[serde(default)]
    pub code: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "type")]
pub enum StreamEvent {
    #[serde(rename = "generation_tokens")]
    GenerationTokens {
        text: String,
        classification: String,
    },
    #[serde(rename = "agent_message_success")]
    AgentMessageSuccess { message: AgentMessage },
    #[serde(rename = "agent_error")]
    AgentError { error: StreamError },
    #[serde(rename = "user_message_error")]
    UserMessageError { error: StreamError },
    #[serde(rename = "agent_generation_cancelled")]
    AgentGenerationCancelled,
    #[serde(rename = "agent_action_success")]
    AgentActionSuccess {
        #[serde(default)]
        action: serde_json::Value,
    },
    #[serde(rename = "tool_approve_execution")]
    ToolApproveExecution {
        #[serde(rename = "actionId")]
        action_id: String,
        #[serde(rename = "conversationId")]
        conversation_id: String,
        #[serde(rename = "messageId")]
        message_id: String,
        #[serde(default)]
        inputs: serde_json::Value,
        #[serde(default)]
        metadata: serde_json::Value,
    },
    #[serde(other)]
    Unknown,
}

impl StreamEvent {
    pub fn extract_tool_use_from_action(action: &serde_json::Value) -> Option<ToolCall> {
        if action.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
            serde_json::from_value(action.clone()).ok()
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AgentModelInfo {
    #[serde(rename = "modelId")]
    pub model_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AgentInfo {
    #[serde(rename = "sId")]
    pub s_id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub scope: String,
    #[serde(default)]
    pub model: Option<AgentModelInfo>,
}

impl AgentInfo {
    pub fn context_size(&self) -> Option<u32> {
        self.model
            .as_ref()
            .and_then(|m| context_size_for_model(&m.model_id))
    }
}

/// Static context window sizes sourced from Dust model configurations.
pub fn context_size_for_model(model_id: &str) -> Option<u32> {
    match model_id {
        // Anthropic — 180K
        "claude-3-opus-20240229"
        | "claude-3-5-sonnet-20240620"
        | "claude-3-5-sonnet-20241022"
        | "claude-3-haiku-20240307"
        | "claude-3-5-haiku-20241022"
        | "claude-haiku-4-5-20251001" => Some(180_000),
        // Anthropic — 200K, OpenAI reasoning 200K
        "claude-4-opus-20250514"
        | "claude-4-sonnet-20250514"
        | "claude-sonnet-4-5-20250929"
        | "claude-3-7-sonnet-20250219"
        | "claude-opus-4-5-20251101"
        | "o1"
        | "o3"
        | "o3-mini"
        | "o4-mini" => Some(200_000),
        // Anthropic — 250K
        "claude-opus-4-6" | "claude-opus-4-7" | "claude-opus-4-8" | "claude-fable-5"
        | "claude-sonnet-4-6" => Some(250_000),
        // OpenAI
        "gpt-3.5-turbo" => Some(16_384),
        "gpt-4-turbo"
        | "gpt-4o"
        | "gpt-4o-2024-08-06"
        | "gpt-4o-mini"
        | "o1-mini"
        | "codestral-latest"
        | "accounts/fireworks/models/kimi-k2-instruct-0905"
        | "mistral-small-latest"
        | "mistral-medium" => Some(128_000),
        "gpt-5" | "gpt-5.1" | "gpt-5.2" | "gpt-5.4-mini" | "gpt-5.4-nano" | "gpt-5-mini"
        | "gpt-5-nano" => Some(400_000),
        "gpt-4.1-2025-04-14"
        | "gpt-4.1-mini-2025-04-14"
        | "gpt-5.4"
        | "gpt-5.5"
        | "gemini-2.5-flash"
        | "gemini-2.5-flash-lite"
        | "gemini-2.5-pro"
        | "gemini-3-pro-preview"
        | "gemini-3.1-pro-preview"
        | "gemini-3.1-flash-lite"
        | "gemini-3-flash-preview"
        | "gemini-3.5-flash"
        | "accounts/fireworks/models/deepseek-v4-pro" => Some(1_000_000),
        // Mistral
        "mistral-large-latest" | "mistral-medium-3-5" => Some(256_000),
        // Fireworks
        "accounts/fireworks/models/deepseek-v3p2" => Some(163_800),
        "accounts/fireworks/models/kimi-k2p5" => Some(262_100),
        "accounts/fireworks/models/minimax-m2p5" => Some(196_608),
        "accounts/fireworks/models/glm-5" => Some(202_752),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListAgentsResponse {
    pub agent_configurations: Vec<AgentInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextUsageResponse {
    pub context_usage: Option<u32>,
    pub context_size: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_serializes_cli_origin_and_agent_mention() {
        let request = CreateConversationRequest {
            title: None,
            visibility: "unlisted".to_string(),
            message: MessageBody {
                content: "hello".to_string(),
                mentions: vec![Mention {
                    configuration_id: "agent_123".to_string(),
                }],
                context: MessageContext {
                    timezone: "Europe/Paris".to_string(),
                    username: "oxide".to_string(),
                    origin: "cli".to_string(),
                    email: None,
                    full_name: Some("Oxide User".to_string()),
                    client_side_mcp_server_ids: None,
                },
            },
        };

        let json = serde_json::to_value(request).expect("serialize");
        assert_eq!(json["message"]["context"]["origin"], "cli");
        assert_eq!(
            json["message"]["mentions"][0]["configurationId"],
            "agent_123"
        );
        assert_eq!(json["message"]["context"]["fullName"], "Oxide User");
        assert!(json.get("title").is_none());
    }

    #[test]
    fn create_conversation_response_deserializes_message_id() {
        let json = r#"{
            "conversation": {"sId": "c_123"},
            "message": {"sId": "m_123"}
        }"#;

        let response =
            serde_json::from_str::<CreateConversationResponse>(json).expect("deserialize");
        assert_eq!(response.conversation.s_id, "c_123");
        assert_eq!(response.message.expect("message").s_id, "m_123");
    }

    #[test]
    fn post_message_response_deserializes_message_id() {
        let json = r#"{
            "message": {"sId": "m_456"}
        }"#;

        let response = serde_json::from_str::<PostMessageResponse>(json).expect("deserialize");
        assert_eq!(response.message.s_id, "m_456");
    }

    #[test]
    fn post_message_request_serializes_message_only() {
        let request = MessageBody {
            content: "follow up".to_string(),
            mentions: vec![Mention {
                configuration_id: "agent_123".to_string(),
            }],
            context: MessageContext {
                timezone: "Europe/Paris".to_string(),
                username: "oxide".to_string(),
                origin: "cli".to_string(),
                email: None,
                full_name: None,
                client_side_mcp_server_ids: None,
            },
        };

        let json = serde_json::to_value(request).expect("serialize");
        assert!(json.get("title").is_none());
        assert!(json.get("visibility").is_none());
        assert_eq!(json["content"], "follow up");
    }

    #[test]
    fn agent_info_deserializes_from_api_response() {
        let json = r#"{
            "sId": "abc123",
            "name": "dust",
            "description": "General-purpose assistant",
            "scope": "workspace"
        }"#;

        let agent = serde_json::from_str::<AgentInfo>(json).expect("deserialize");
        assert_eq!(agent.s_id, "abc123");
        assert_eq!(agent.name, "dust");
        assert_eq!(agent.description, "General-purpose assistant");
        assert_eq!(agent.scope, "workspace");
    }

    #[test]
    fn agent_info_handles_missing_description() {
        let json = r#"{
            "sId": "abc123",
            "name": "dust",
            "scope": "global"
        }"#;

        let agent = serde_json::from_str::<AgentInfo>(json).expect("deserialize");
        assert_eq!(agent.description, "");
    }

    #[test]
    fn list_agents_response_deserializes_array() {
        let json = r#"{
            "agentConfigurations": [
                {
                    "sId": "a1",
                    "name": "dust",
                    "description": "General assistant",
                    "scope": "workspace"
                },
                {
                    "sId": "a2",
                    "name": "helper",
                    "description": "Code helper",
                    "scope": "published"
                }
            ]
        }"#;

        let response = serde_json::from_str::<ListAgentsResponse>(json).expect("deserialize");
        assert_eq!(response.agent_configurations.len(), 2);
        assert_eq!(response.agent_configurations[0].name, "dust");
        assert_eq!(response.agent_configurations[1].name, "helper");
    }

    #[test]
    fn list_agents_response_handles_empty_array() {
        let json = r#"{"agentConfigurations": []}"#;
        let response = serde_json::from_str::<ListAgentsResponse>(json).expect("deserialize");
        assert!(response.agent_configurations.is_empty());
    }

    #[test]
    fn conversation_summary_deserializes_from_api_response() {
        let json = r#"{
            "sId": "conv_123",
            "title": "Project brainstorm",
            "created": 1707900000000,
            "updated": 1707950000000
        }"#;

        let summary = serde_json::from_str::<ConversationSummary>(json).expect("deserialize");
        assert_eq!(summary.s_id, "conv_123");
        assert_eq!(summary.title, Some("Project brainstorm".to_string()));
        assert_eq!(summary.created, 1707900000000);
        assert_eq!(summary.updated, Some(1707950000000));
    }

    #[test]
    fn conversation_summary_handles_missing_title() {
        let json = r#"{
            "sId": "conv_456",
            "created": 1707900000000
        }"#;

        let summary = serde_json::from_str::<ConversationSummary>(json).expect("deserialize");
        assert_eq!(summary.s_id, "conv_456");
        assert_eq!(summary.title, None);
        assert_eq!(summary.created, 1707900000000);
        assert_eq!(summary.updated, None);
    }

    #[test]
    fn list_conversations_response_deserializes_array() {
        let json = r#"{
            "conversations": [
                {
                    "sId": "c1",
                    "title": "First chat",
                    "created": 1707900000000,
                    "updated": 1707950000000
                },
                {
                    "sId": "c2",
                    "title": null,
                    "created": 1707800000000
                }
            ]
        }"#;

        let response =
            serde_json::from_str::<ListConversationsResponse>(json).expect("deserialize");
        assert_eq!(response.conversations.len(), 2);
        assert_eq!(response.conversations[0].s_id, "c1");
        assert_eq!(
            response.conversations[0].title,
            Some("First chat".to_string())
        );
        assert_eq!(response.conversations[1].s_id, "c2");
        assert_eq!(response.conversations[1].title, None);
    }

    #[test]
    fn conversation_message_user_message_deserializes() {
        let json = r#"{
            "type": "user_message",
            "content": "Hello, assistant"
        }"#;

        let msg = serde_json::from_str::<ConversationMessage>(json).expect("deserialize");
        match msg {
            ConversationMessage::UserMessage { content } => {
                assert_eq!(content, "Hello, assistant");
            }
            _ => panic!("Expected UserMessage variant"),
        }
    }

    #[test]
    fn conversation_message_agent_message_still_deserializes() {
        let json = r#"{
            "type": "agent_message",
            "sId": "msg_123",
            "parentMessageId": null
        }"#;

        let msg = serde_json::from_str::<ConversationMessage>(json).expect("deserialize");
        match msg {
            ConversationMessage::AgentMessage { s_id, .. } => {
                assert_eq!(s_id, "msg_123");
            }
            _ => panic!("Expected AgentMessage variant"),
        }
    }

    #[test]
    fn extract_tool_use_from_action_parses_tool_call() {
        let action = serde_json::json!({
            "type": "tool_use",
            "id": "tool_123",
            "name": "bash",
            "input": {
                "command": "ls -la"
            }
        });

        let tool_call = StreamEvent::extract_tool_use_from_action(&action);
        assert!(tool_call.is_some());
        let tool_call = tool_call.unwrap();
        assert_eq!(tool_call.id, "tool_123");
        assert_eq!(tool_call.name, "bash");
        assert_eq!(tool_call.input["command"], "ls -la");
    }

    #[test]
    fn extract_tool_use_from_action_ignores_non_tool_use() {
        let action = serde_json::json!({
            "type": "message",
            "id": "msg_123",
            "text": "hello"
        });

        let tool_call = StreamEvent::extract_tool_use_from_action(&action);
        assert!(tool_call.is_none());
    }

    #[test]
    fn context_usage_response_deserializes() {
        let json = r#"{
            "contextUsage": 12345,
            "contextSize": 200000
        }"#;

        let response = serde_json::from_str::<ContextUsageResponse>(json).expect("deserialize");
        assert_eq!(response.context_usage, Some(12345));
        assert_eq!(response.context_size, Some(200000));
    }

    #[test]
    fn context_usage_response_handles_null_fields() {
        let json = r#"{
            "contextUsage": null,
            "contextSize": null
        }"#;

        let response = serde_json::from_str::<ContextUsageResponse>(json).expect("deserialize");
        assert_eq!(response.context_usage, None);
        assert_eq!(response.context_size, None);
    }

    #[test]
    fn context_usage_response_ignores_model_field() {
        let json = r#"{
            "contextUsage": 5000,
            "contextSize": 100000,
            "model": {"providerId": "anthropic", "modelId": "claude-3-7-sonnet"}
        }"#;

        let response = serde_json::from_str::<ContextUsageResponse>(json).expect("deserialize");
        assert_eq!(response.context_usage, Some(5000));
        assert_eq!(response.context_size, Some(100000));
    }
}
