pub mod device_flow;
pub mod jwt;
pub mod token_refresh;
pub mod token_storage;
pub mod workspace_selection;

use anyhow::Result;

pub fn logout() -> Result<()> {
    token_storage::clear_all()?;
    println!("Logged out. Credentials cleared.");
    Ok(())
}

pub async fn status() -> Result<()> {
    match token_storage::get_access_token()? {
        None => println!("Not logged in. Run `oxide login`."),
        Some(token) => {
            let effective_token = token_refresh::get_valid_token().await.unwrap_or(token);
            let expired = token_refresh::is_token_expired(&effective_token);
            let region = token_storage::get_region()?.unwrap_or_else(|| "unknown".to_string());
            let workspace =
                token_storage::get_workspace_id()?.unwrap_or_else(|| "unknown".to_string());
            println!("Logged in");
            println!("  Region:    {region}");
            println!("  Workspace: {workspace}");
            println!("  Token:     {}", if expired { "expired" } else { "valid" });
        }
    }
    Ok(())
}
