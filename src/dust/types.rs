use serde::{Deserialize, Serialize};

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
    #[serde(other)]
    Unknown,
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
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListAgentsResponse {
    pub agent_configurations: Vec<AgentInfo>,
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
}
