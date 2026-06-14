use std::time::Duration;

use tokio::process::Command;

use super::types::ToolResult;

const BASH_TIMEOUT: Duration = Duration::from_mins(1);

pub struct BashTool;

impl BashTool {
    pub async fn execute(command: &str) -> ToolResult {
        let result = tokio::time::timeout(
            BASH_TIMEOUT,
            Command::new("bash").arg("-c").arg(command).output(),
        )
        .await;

        match result {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                let content = format!("{stdout}{stderr}").trim().to_string();
                let is_error = !output.status.success();

                ToolResult {
                    tool_use_id: String::new(),
                    content,
                    is_error,
                }
            }
            Ok(Err(_)) => ToolResult {
                tool_use_id: String::new(),
                content: "Failed to execute bash command".to_string(),
                is_error: true,
            },
            Err(_) => ToolResult {
                tool_use_id: String::new(),
                content: "Command timed out after 60 seconds".to_string(),
                is_error: true,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn executes_echo_command() {
        let result = BashTool::execute("echo hello").await;
        assert!(!result.is_error);
        assert_eq!(result.content, "hello");
    }

    #[tokio::test]
    async fn handles_non_zero_exit() {
        let result = BashTool::execute("exit 1").await;
        assert!(result.is_error);
    }

    #[tokio::test]
    async fn handles_stderr() {
        let result = BashTool::execute("echo error >&2").await;
        assert!(!result.is_error);
        assert_eq!(result.content, "error");
    }

    #[tokio::test]
    async fn handles_empty_output() {
        let result = BashTool::execute("true").await;
        assert!(!result.is_error);
        assert_eq!(result.content, "");
    }
}
