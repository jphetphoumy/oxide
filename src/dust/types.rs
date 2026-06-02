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

pub type PostMessageResponse = CreateConversationResponse;

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
#[serde(tag = "type")]
pub enum ConversationMessage {
    #[serde(rename = "agent_message")]
    AgentMessage {
        #[serde(rename = "sId")]
        s_id: String,
        #[serde(rename = "parentMessageId")]
        parent_message_id: Option<String>,
    },
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
}
