//! `superfik`: talk to Superfik's agent in a terminal.
//!
//! - `superfik [prompt]` opens the chat (sending `prompt` first).
//! - `superfik -p <prompt>` answers once and prints the reply.
//! - `superfik login` / `logout` connect or forget an LLM provider.
//! - `superfik models` lists what the connected providers offer.
//! - `superfik pull <model>` / `rm <model>` download or delete a local MLX model.
//! - `superfik mcp …` lists, adds, removes and approves MCP servers.
//!
//! Every terminal talks to one background daemon per computer (Unix), so
//! local models, embeddings and memory load once.

mod config;
mod daemon;
mod doctor;
mod local_models;
mod login;
mod mcp;
mod print;
mod print_output;
mod tui;
mod update;
mod verdict;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use superfik_sdk::PermissionMode;

#[derive(Debug, Parser)]
#[command(name = "superfik", version, about = "Talk to Superfik's coding agent in your project.")]
struct Cli {
    /// What to ask; opens the chat with it unless --print is given.
    prompt: Vec<String>,
    /// Answer once, print the reply and exit.
    #[arg(short, long)]
    print: bool,
    /// Continue the last conversation in this folder.
    #[arg(short = 'c', long = "continue")]
    resume: bool,
    /// Continue an earlier conversation: its id, or pick one of this folder.
    #[arg(short = 'r', long = "resume", conflicts_with = "resume", num_args = 0..=1, default_missing_value = "")]
    pick: Option<String>,
    /// With --print: text, json (one result object) or stream-json (every
    /// event as a JSON line, then the result).
    #[arg(long = "output-format", requires = "print", value_parser = print_output::Format::parse, default_value = "text")]
    output_format: print_output::Format,
    /// With --print: text (the prompt) or stream-json (user messages as
    /// JSON lines on stdin, answered in turn).
    #[arg(long = "input-format", requires = "print", default_value = "text")]
    input_format: String,
    /// With --print: the most model turns a run may take.
    #[arg(long = "max-turns", requires = "print")]
    max_turns: Option<u32>,
    /// Rules allowed without asking, like `Bash(git diff:*) Edit` (repeatable).
    #[arg(long = "allowedTools", alias = "allowed-tools", value_name = "RULES")]
    allowed_tools: Vec<String>,
    /// Rules refused, like `Bash(rm:*)` (repeatable).
    #[arg(long = "disallowedTools", alias = "disallowed-tools", value_name = "RULES")]
    disallowed_tools: Vec<String>,
    /// Text added at the end of the system prompt.
    #[arg(long = "append-system-prompt")]
    append_system_prompt: Option<String>,
    /// With -c or -r: go on in a copy of that conversation, leaving it as
    /// it is.
    #[arg(long = "fork-session")]
    fork_session: bool,
    /// Chat at the bottom of the terminal, the conversation in its own
    /// scrollback, instead of full screen.
    #[arg(long)]
    inline: bool,
    /// The model to go on with when the chosen one is overloaded (same
    /// provider).
    #[arg(long = "fallback-model")]
    fallback_model: Option<String>,
    /// The model (its id or name); the last one used otherwise.
    #[arg(short, long)]
    model: Option<String>,
    /// ask, edits, plan or bypass.
    #[arg(long, value_parser = parse_mode)]
    mode: Option<PermissionMode>,
    /// With --print: judge the work against these criteria before it ends
    /// (exits 1 when it fails them).
    #[arg(long, requires = "print")]
    verification: Option<String>,
    /// The project folder (the current one by default).
    #[arg(short = 'C', long)]
    cwd: Option<PathBuf>,
    /// Let the agent read this folder like the project (repeatable).
    #[arg(long = "add-dir", value_name = "PATH")]
    add_dir: Vec<String>,
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
    /// Download a local MLX model from Hugging Face (Apple silicon), e.g.
    /// `mlx-community/Qwen3-8B-4bit`; a bare name means mlx-community's.
    Pull { model: String },
    /// Delete a downloaded local MLX model.
    Rm { model: String },
    /// The MCP servers a session here can use: list, add, remove, approve.
    Mcp {
        #[command(subcommand)]
        action: Option<mcp::McpAction>,
    },
    /// Replace this binary with the latest release.
    Update,
    /// Check that this installation can work in the folder.
    Doctor,
    /// Read or change the settings: the user's, or this project's with --project.
    Config {
        #[command(subcommand)]
        action: config::ConfigAction,
        /// This project's .superfik/settings.local.json instead.
        #[arg(long, global = true)]
        project: bool,
    },
    /// The background daemon every terminal shares.
    #[command(hide = true)]
    Daemon {
        #[command(subcommand)]
        action: DaemonAction,
    },
}

#[derive(Debug, Subcommand)]
enum DaemonAction {
    /// Serve this computer's terminals until the last one leaves.
    Serve,
    /// Stop the running daemon (its terminals' runs end).
    Stop,
    /// Say whether one is running.
    Status,
}

fn rule_list(text: &str) -> Vec<String> {
    superfik_core::permission_rules::split_list(text)
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
    #[cfg(unix)]
    if let Some(Action::Daemon { action }) = &cli.command {
        match action {
            DaemonAction::Serve => daemon::serve()?,
            DaemonAction::Stop => daemon::stop()?,
            DaemonAction::Status => println!("{}", daemon::status()),
        }
        return Ok(ExitCode::SUCCESS);
    }
    // Local models are this computer's files: no daemon needed.
    match &cli.command {
        Some(Action::Pull { model }) => return local_models::pull(model).map(|()| ExitCode::SUCCESS),
        Some(Action::Rm { model }) => return local_models::remove(model).map(|()| ExitCode::SUCCESS),
        Some(Action::Update) => return update::run().map(|()| ExitCode::SUCCESS),
        Some(Action::Config { action, project }) => {
            println!("{}", config::run(&config::file(&cwd, *project)?, action.clone())?);
            return Ok(ExitCode::SUCCESS);
        }
        _ => {}
    }
    let (superfik, stale) = daemon::connect()?;
    let prompt = (!cli.prompt.is_empty()).then(|| cli.prompt.join(" "));
    let add_dirs = cli.add_dir.iter().map(|typed| tui::dirs::folder(&cwd, typed)).collect::<Result<Vec<_>, _>>()?;
    let headless = superfik_sdk::Headless {
        max_turns: cli.max_turns,
        allowed_tools: cli.allowed_tools.iter().flat_map(|rules| rule_list(rules)).collect(),
        disallowed_tools: cli.disallowed_tools.iter().flat_map(|rules| rule_list(rules)).collect(),
        append_system: cli.append_system_prompt.clone(),
        fallback_model: cli.fallback_model.clone(),
    };
    match cli.command {
        Some(Action::Login) => login::login(&superfik).map(|()| ExitCode::SUCCESS),
        Some(Action::Logout) => login::logout(&superfik).map(|()| ExitCode::SUCCESS),
        Some(Action::Doctor) => {
            let checks = doctor::checks(&superfik, &cwd);
            println!("{}", doctor::render(&checks));
            Ok(if checks.iter().all(|c| c.ok) { ExitCode::SUCCESS } else { ExitCode::FAILURE })
        }
        Some(Action::Models) => {
            for model in superfik.models().map_err(|e| e.to_string())? {
                println!("{:<40} {}", model.id, model.label);
            }
            Ok(ExitCode::SUCCESS)
        }
        Some(Action::Mcp { action }) => {
            mcp::run(&superfik, &cwd, action.unwrap_or(mcp::McpAction::List)).map(|()| ExitCode::SUCCESS)
        }
        Some(Action::Daemon { .. }) => Err("the daemon needs macOS or Linux".into()),
        Some(Action::Pull { .. } | Action::Rm { .. } | Action::Update | Action::Config { .. }) => Ok(ExitCode::SUCCESS),
        None if cli.print => {
            if let Some(stale) = &stale {
                eprintln!("superfik: {stale}");
            }
            let prompts = match cli.input_format.as_str() {
                "stream-json" => print_output::prompts(std::io::stdin().lines().map_while(Result::ok))?,
                "text" => vec![prompt.ok_or("--print needs a prompt")?],
                other => return Err(format!("unknown input format {other}: text or stream-json")),
            };
            let resume = match cli.pick.as_deref() {
                Some(id) if !id.is_empty() => Some(superfik_sdk::SessionId(id.to_owned())),
                _ if cli.resume => tui::last_session(&superfik.settings().map_err(|e| e.to_string())?, &cwd),
                _ => None,
            };
            let wanted = print::Wanted {
                model: cli.model.as_deref(),
                mode: cli.mode,
                verification: cli.verification,
                add_dirs,
                headless,
                format: cli.output_format,
                resume,
                fork: cli.fork_session,
            };
            print::print_all(&superfik, cwd, wanted, &prompts)
        }
        None => {
            let resume = if cli.resume {
                let settings = superfik.settings().map_err(|e| e.to_string())?;
                let last = tui::last_session(&settings, &cwd);
                if last.is_none() {
                    eprintln!("superfik: no earlier conversation in this folder; starting a new one");
                }
                last
            } else {
                None
            };
            let (resume, pick) = match cli.pick {
                Some(id) if !id.is_empty() => (Some(superfik_sdk::SessionId(id)), false),
                Some(_) => (resume, true),
                None => (resume, false),
            };
            let start = tui::Start { resume, add_dirs, pick, headless, fork: cli.fork_session, inline: cli.inline };
            tui::run(superfik, cwd, cli.model, cli.mode, prompt, stale, start).map_err(|e| e.to_string())?;
            Ok(ExitCode::SUCCESS)
        }
    }
}
