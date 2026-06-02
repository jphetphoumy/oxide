use std::io::{self, Write};

use anyhow::{Context, Result, anyhow};
use reqwest::Client;
use serde::Deserialize;

use crate::auth::{token_refresh, token_storage};
use crate::dust::client::base_url_for_region;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceChoice {
    pub s_id: String,
    pub name: String,
    pub role: String,
}

#[derive(Debug, Deserialize)]
struct MeResponse {
    user: UserInfo,
}

#[derive(Debug, Deserialize)]
struct UserInfo {
    workspaces: Vec<Workspace>,
}

#[derive(Debug, Clone, Deserialize)]
struct Workspace {
    #[serde(rename = "sId")]
    s_id: String,
    name: String,
    role: String,
}

pub async fn ensure_workspace_selected_with_client(http: &Client) -> Result<String> {
    if let Some(workspace_id) = token_storage::get_workspace_id()?
        && !is_placeholder_workspace_id(&workspace_id)
    {
        return Ok(workspace_id);
    }

    select_workspace_for_login(http).await
}

pub async fn select_workspace_for_login(http: &Client) -> Result<String> {
    let region = token_storage::get_region()?.unwrap_or_else(|| "us-central1".to_string());
    let workspaces = fetch_workspaces(http, &region).await?;
    let selected = prompt_workspace_selection(&workspaces)?;
    token_storage::save_workspace_id(&selected.s_id)?;
    println!("Selected workspace: {} ({})", selected.name, selected.s_id);
    Ok(selected.s_id)
}

pub async fn fetch_workspaces(http: &Client, region: &str) -> Result<Vec<WorkspaceChoice>> {
    let token = token_refresh::get_valid_token().await?;

    let response = http
        .get(format!("{}/api/v1/me", base_url_for_region(region)))
        .bearer_auth(token)
        .send()
        .await
        .context("failed to request Dust profile")?
        .error_for_status()
        .context("Dust rejected the profile request")?;

    let body = response
        .text()
        .await
        .context("failed to read Dust profile response")?;
    parse_me_response(&body)
}

pub fn parse_me_response(body: &str) -> Result<Vec<WorkspaceChoice>> {
    let me: MeResponse =
        serde_json::from_str(body).context("failed to decode Dust profile response")?;

    Ok(me
        .user
        .workspaces
        .into_iter()
        .map(|workspace| WorkspaceChoice {
            s_id: workspace.s_id,
            name: workspace.name,
            role: workspace.role,
        })
        .collect())
}

pub fn prompt_workspace_selection(workspaces: &[WorkspaceChoice]) -> Result<WorkspaceChoice> {
    if workspaces.is_empty() {
        return Err(anyhow!("Dust account has no accessible workspaces"));
    }

    if workspaces.len() == 1 {
        return Ok(workspaces[0].clone());
    }

    println!("Choose a workspace:");
    for (index, workspace) in workspaces.iter().enumerate() {
        println!("  {}) {} ({})", index + 1, workspace.name, workspace.role);
    }

    loop {
        print!("Workspace [1-{}, default=1]: ", workspaces.len());
        io::stdout()
            .flush()
            .context("failed to flush workspace prompt")?;

        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .context("failed to read workspace selection")?;

        if let Some(index) = parse_selection(&input, workspaces.len()) {
            return Ok(workspaces[index].clone());
        }

        println!("Invalid selection, try again.");
    }
}

fn parse_selection(input: &str, len: usize) -> Option<usize> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Some(0);
    }

    let index = trimmed.parse::<usize>().ok()?;
    if index == 0 || index > len {
        None
    } else {
        Some(index - 1)
    }
}

fn is_placeholder_workspace_id(workspace_id: &str) -> bool {
    workspace_id == "me" || workspace_id.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_spaces_array() {
        let body = r#"{"user":{"workspaces":[{"sId":"ws_123","name":"Main","role":"admin"}]}}"#;
        let workspaces = parse_me_response(body).expect("parse");
        assert_eq!(
            workspaces,
            vec![WorkspaceChoice {
                s_id: "ws_123".to_string(),
                name: "Main".to_string(),
                role: "admin".to_string(),
            }]
        );
    }

    #[test]
    fn parses_root_array_and_display_name() {
        let body = r#"{"user":{"workspaces":[{"sId":"ws_456","name":"Team","role":"member"}]}}"#;
        let workspaces = parse_me_response(body).expect("parse");
        assert_eq!(
            workspaces,
            vec![WorkspaceChoice {
                s_id: "ws_456".to_string(),
                name: "Team".to_string(),
                role: "member".to_string(),
            }]
        );
    }

    #[test]
    fn selection_defaults_to_first_entry_on_empty_input() {
        let workspaces = vec![
            WorkspaceChoice {
                s_id: "ws_1".to_string(),
                name: "One".to_string(),
                role: "admin".to_string(),
            },
            WorkspaceChoice {
                s_id: "ws_2".to_string(),
                name: "Two".to_string(),
                role: "member".to_string(),
            },
        ];
        assert_eq!(parse_selection("", workspaces.len()), Some(0));
    }

    #[test]
    fn selection_rejects_out_of_range_entries() {
        assert_eq!(parse_selection("3", 2), None);
        assert_eq!(parse_selection("0", 2), None);
    }
}
