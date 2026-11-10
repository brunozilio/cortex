//! `superfik`: talk to Superfik's agent in a terminal.
//!
//! - `superfik [prompt]` opens the chat (sending `prompt` first).
//! - `superfik -p <prompt>` answers once and prints the reply.
//! - `superfik login` / `logout` connect or forget an LLM provider.
//! - `superfik models` lists what the connected providers offer.

mod login;
mod print;
mod tui;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use superfik_sdk::{PermissionMode, Superfik};

#[derive(Debug, Parser)]
#[command(name = "superfik", version, about = "Talk to Superfik's coding agent in your project.")]
struct Cli {
    /// What to ask; opens the chat with it unless --print is given.
    prompt: Vec<String>,
    /// Answer once, print the reply and exit.
    #[arg(short, long)]
    print: bool,
    /// The model (its id or name); the last one used otherwise.
    #[arg(short, long)]
    model: Option<String>,
    /// ask, edits, plan or bypass.
    #[arg(long, value_parser = parse_mode)]
    mode: Option<PermissionMode>,
    /// The project folder (the current one by default).
    #[arg(short = 'C', long)]
    cwd: Option<PathBuf>,
    #[command(subcommand)]
    command: Option<Action>,
}

#[derive(Debug, Subcommand)]
enum Action {
    /// Connect an LLM provider: an API key, or a Claude or ChatGPT subscription.
    Login,
    /// Forget a provider's credentials.
    Logout,
    /// List the models the connected providers offer.
    Models,
}

fn parse_mode(text: &str) -> Result<PermissionMode, String> {
    superfik_core::command::mode_named(text).ok_or_else(|| format!("`{text}` is not ask, edits, plan or bypass"))
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(code) => code,
        Err(message) => {
            eprintln!("superfik: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<ExitCode, String> {
    let cwd = match cli.cwd {
        Some(dir) => dir,
        None => std::env::current_dir().map_err(|e| e.to_string())?,
    };
    let cwd = std::fs::canonicalize(&cwd).map_err(|e| format!("{}: {e}", cwd.display()))?;
    let superfik = Superfik::builder().build().map_err(|e| e.to_string())?;
    let prompt = (!cli.prompt.is_empty()).then(|| cli.prompt.join(" "));
    match cli.command {
        Some(Action::Login) => login::login(&superfik).map(|()| ExitCode::SUCCESS),
        Some(Action::Logout) => login::logout(&superfik).map(|()| ExitCode::SUCCESS),
        Some(Action::Models) => {
            for model in superfik.models().map_err(|e| e.to_string())? {
                println!("{:<40} {}", model.id, model.label);
            }
            Ok(ExitCode::SUCCESS)
        }
        None if cli.print => {
            let prompt = prompt.ok_or("--print needs a prompt")?;
            print::print(&superfik, cwd, cli.model.as_deref(), cli.mode, &prompt)
        }
        None => {
            tui::run(superfik, cwd, cli.model, cli.mode, prompt).map_err(|e| e.to_string())?;
            Ok(ExitCode::SUCCESS)
        }
    }
}
