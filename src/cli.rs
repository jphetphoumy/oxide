use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "oxide",
    about = "Talk to Dust agents from the terminal",
    version,
    after_help = "Run `oxide` with no subcommand to start the chat TUI."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Authenticate with Dust via OAuth
    Login,
    /// Clear stored credentials
    Logout,
    /// Show current auth status
    Status,
}
