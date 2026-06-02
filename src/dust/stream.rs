use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::Value;

use crate::dust::types::StreamEvent;

#[derive(Debug, Deserialize)]
struct DustSseEnvelope {
    #[serde(default, rename = "eventId")]
    _event_id: Option<String>,
    data: Value,
}

pub struct EventStream {
    response: reqwest::Response,
    buffer: String,
}

impl EventStream {
    #[allow(clippy::missing_const_for_fn)]
    pub fn new(response: reqwest::Response) -> Self {
        Self {
            response,
            buffer: String::new(),
        }
    }

    pub async fn next_event(&mut self) -> Option<Result<StreamEvent>> {
        loop {
            if let Some(event) = pop_event(&mut self.buffer) {
                return Some(event);
            }

            match self.response.chunk().await {
                Ok(Some(chunk)) => self.buffer.push_str(&String::from_utf8_lossy(&chunk)),
                Ok(None) => return None,
                Err(error) => {
                    return Some(Err(error).context("failed to read Dust SSE stream"));
                }
            }
        }
    }
}

fn pop_event(buffer: &mut String) -> Option<Result<StreamEvent>> {
    loop {
        let separator = buffer.find("\n\n")?;
        let event_text = buffer[..separator].to_string();
        buffer.drain(..separator + 2);

        let data = event_text
            .lines()
            .filter_map(|line| line.strip_prefix("data: "))
            .collect::<Vec<_>>()
            .join("\n");

        if data.trim().is_empty() {
            continue;
        }

        return Some(parse_stream_event(&data));
    }
}

fn parse_stream_event(data: &str) -> Result<StreamEvent> {
    if let Ok(envelope) = serde_json::from_str::<DustSseEnvelope>(data) {
        return serde_json::from_value(envelope.data)
            .context("failed to parse Dust SSE payload inside envelope");
    }

    serde_json::from_str::<StreamEvent>(data)
        .with_context(|| format!("failed to parse Dust SSE payload: {data}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dust::types::{AgentMessage, StreamError};

    #[test]
    fn parses_generation_tokens_event() {
        let mut buffer = String::from(
            "event: message\ndata: {\"type\":\"generation_tokens\",\"text\":\"Hello\",\"classification\":\"tokens\"}\n\n",
        );

        let event = pop_event(&mut buffer).expect("event").expect("valid event");

        assert_eq!(
            event,
            StreamEvent::GenerationTokens {
                text: "Hello".to_string(),
                classification: "tokens".to_string(),
            }
        );
        assert!(buffer.is_empty());
    }

    #[test]
    fn parses_multiline_data_block() {
        let mut buffer = String::from(
            "data: {\"type\":\"agent_message_success\",\"message\":{\"content\":\"Hello\"}}\ndata: \n\n",
        );

        let event = pop_event(&mut buffer).expect("event").expect("valid event");

        assert_eq!(
            event,
            StreamEvent::AgentMessageSuccess {
                message: AgentMessage {
                    content: Some("Hello".to_string()),
                },
            }
        );
    }

    #[test]
    fn parses_error_event() {
        let mut buffer = String::from(
            "data: {\"type\":\"agent_error\",\"error\":{\"code\":\"bad_request\",\"message\":\"boom\"}}\n\n",
        );

        let event = pop_event(&mut buffer).expect("event").expect("valid event");

        assert_eq!(
            event,
            StreamEvent::AgentError {
                error: StreamError {
                    code: Some("bad_request".to_string()),
                    message: "boom".to_string(),
                },
            }
        );
    }

    #[test]
    fn parses_dust_envelope_payload() {
        let mut buffer = String::from(
            "data: {\"eventId\":\"1780389674109-0\",\"data\":{\"type\":\"generation_tokens\",\"created\":1780389674108,\"configurationId\":\"dust\",\"messageId\":\"pmOzQf9QwW\",\"text\":\"Bonjour\",\"classification\":\"tokens\",\"traceId\":\"llm_trace_ee79ca10-5052-4aa5-b8fe-f8332fba730e\",\"step\":0}}\n\n",
        );

        let event = pop_event(&mut buffer).expect("event").expect("valid event");

        assert_eq!(
            event,
            StreamEvent::GenerationTokens {
                text: "Bonjour".to_string(),
                classification: "tokens".to_string(),
            }
        );
    }

    #[test]
    fn skips_empty_events() {
        let mut buffer = String::from("\n\ndata: {\"type\":\"agent_generation_cancelled\"}\n\n");

        let event = pop_event(&mut buffer).expect("event").expect("valid event");

        assert_eq!(event, StreamEvent::AgentGenerationCancelled);
    }
}
