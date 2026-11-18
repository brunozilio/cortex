mod config;
mod doctor;
mod eval;
mod logging;
mod login;
mod mcp;
mod plugin;
mod print;
mod print_output;
mod remote;
mod teammate;
mod tui;

use std::path::PathBuf;
use std::process::ExitCode;

use app::{Client, PermissionMode};
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = brand::COMMAND, version, about = concat!("Talk to ", brand::name!(), "'s coding agent in your project."))]
struct Cli {
    #[arg(help = "What to ask; opens the chat with it unless --print is given")]
    prompt: Vec<String>,
    #[arg(help = "Answer once, print the reply and exit")]
    #[arg(short, long)]
    print: bool,
    #[arg(help = "Continue the last conversation in this folder")]
    #[arg(short = 'c', long = "continue")]
    resume: bool,
    #[arg(help = "Continue an earlier conversation: its id, or pick one of this folder")]
    #[arg(short = 'r', long = "resume", conflicts_with = "resume", num_args = 0..=1, default_missing_value = "")]
    pick: Option<String>,
    #[arg(
        help = "With --print: text, json (one result object) or stream-json (every event as a JSON line, then the result)"
    )]
    #[arg(long = "output-format", requires = "print", value_parser = print_output::Format::parse, default_value = "text")]
    output_format: print_output::Format,
    #[arg(
        help = "With --print: text (the prompt) or stream-json (user messages as JSON lines on stdin, answered in turn)"
    )]
    #[arg(long = "input-format", requires = "print", default_value = "text")]
    input_format: String,
    #[arg(help = "With --print: the most model turns a run may take")]
    #[arg(long = "max-turns", requires = "print")]
    max_turns: Option<u32>,
    #[arg(help = "Rules allowed without asking, like `Bash(git diff:*) Edit` (repeatable)")]
    #[arg(long = "allowedTools", alias = "allowed-tools", value_name = "RULES")]
    allowed_tools: Vec<String>,
    #[arg(help = "Rules refused, like `Bash(rm:*)` (repeatable)")]
    #[arg(long = "disallowedTools", alias = "disallowed-tools", value_name = "RULES")]
    disallowed_tools: Vec<String>,
    #[arg(help = "Text added at the end of the system prompt")]
    #[arg(long = "append-system-prompt")]
    append_system_prompt: Option<String>,
    #[arg(help = "With -c or -r: go on in a copy of that conversation, leaving it as it is")]
    #[arg(long = "fork-session")]
    fork_session: bool,
    #[arg(help = "Chat at the bottom of the terminal, the conversation in its own scrollback, instead of full screen")]
    #[arg(long)]
    inline: bool,
    #[arg(
        help = "MCP servers to keep up as channels: what they send (a chat's messages, a webhook) reaches the agent as an event. Comma-separated."
    )]
    #[arg(long, value_delimiter = ',')]
    channels: Vec<String>,
    #[arg(help = "The model to go on with when the chosen one is overloaded (same provider)")]
    #[arg(long = "fallback-model")]
    fallback_model: Option<String>,
    #[arg(help = "The model (its id or name); the last one used otherwise")]
    #[arg(short, long)]
    model: Option<String>,
    #[arg(help = "ask, edits, plan or bypass")]
    #[arg(long, value_parser = parse_mode)]
    mode: Option<PermissionMode>,
    #[arg(help = "With --print: judge the work against these criteria before it ends (exits 1 when it fails them)")]
    #[arg(long, requires = "print")]
    verification: Option<String>,
    #[arg(help = "With --print: trust the project folder for this run only, so its hooks, scripts, MCP servers and \
                  local settings load; nothing is stored")]
    #[arg(long, requires = "print")]
    trust: bool,
    #[arg(help = "The project folder (the current one by default)")]
    #[arg(short = 'C', long)]
    cwd: Option<PathBuf>,
    #[arg(help = "Let the agent read this folder like the project (repeatable)")]
    #[arg(long = "add-dir", value_name = "PATH")]
    add_dir: Vec<String>,
    #[command(subcommand)]
    command: Option<Action>,
}

#[derive(Debug, Subcommand)]
enum Action {
    #[command(about = "Connect an LLM provider with an API key or a subscription; it lists the providers to pick from")]
    Login,
    #[command(about = "Forget a provider's credentials")]
    Logout,
    #[command(about = "List the models the connected providers offer")]
    Models,
    #[command(about = "The MCP servers a session here can use: list, get, add, remove, approve, deny, login, logout")]
    Mcp {
        #[command(subcommand)]
        action: Option<mcp::McpAction>,
    },
    #[command(about = concat!("The plugins in ~/", brand::dir!(), "/plugins: list, install, update, remove"))]
    Plugin {
        #[command(subcommand)]
        action: Option<plugin::PluginAction>,
    },
    #[command(
        about = "Remote control from a phone, through your relay: status, up, logout, relay, pair, devices, revoke"
    )]
    Remote {
        #[command(subcommand)]
        action: Option<remote::RemoteAction>,
    },
    #[command(about = "Check that this installation can work in the folder")]
    Doctor,
    #[command(
        about = concat!(
            "Run the evals in ",
            brand::dir!(),
            "/evals (each <name>/case.json, with an optional fixture/) and score them"
        )
    )]
    Eval {
        #[arg(help = "Only these cases (all by default)")]
        names: Vec<String>,
        #[arg(long, help = "Print the scores as JSON")]
        json: bool,
    },
    #[command(hide = true, about = "Run as a teammate of a lead's team (started by the lead)")]
    Teammate {
        #[arg(
            long,
            help = "The file the lead wrote with this teammate's team, name, folder, model, mode and instructions"
        )]
        spec: Option<PathBuf>,
    },
    #[command(about = "Read or change the settings: the user's, or this project's with --project")]
    Config {
        #[command(subcommand)]
        action: config::ConfigAction,
        #[arg(help = concat!("This project's ", brand::dir!(), "/settings.local.json instead"))]
        #[arg(long, global = true)]
        project: bool,
    },
}

fn rule_list(text: &str) -> Vec<String> {
    domain::permission_rules::split_list(text)
}

fn parse_mode(text: &str) -> Result<PermissionMode, String> {
    domain::command::mode_named(text).ok_or_else(|| format!("`{text}` is not ask, edits, plan or bypass"))
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    logging::install();
    match run(cli) {
        Ok(code) => code,
        Err(message) => {
            eprintln!("{brand_command}: {message}", brand_command = brand::COMMAND);
            ExitCode::FAILURE
        }
    }
}

struct Launch {
    client: Client,
    cwd: PathBuf,
    prompt: Option<String>,
    add_dirs: Vec<PathBuf>,
    headless: app::Headless,
}

fn run(mut cli: Cli) -> Result<ExitCode, String> {
    let spec = teammate::spec_of(&mut cli)?;
    let cwd = match cli.cwd.take() {
        Some(dir) => dir,
        None => std::env::current_dir().map_err(|e| e.to_string())?,
    };
    let cwd = std::fs::canonicalize(&cwd).map_err(|e| format!("{}: {e}", cwd.display()))?;
    if let Some(Action::Config { action, project }) = &cli.command {
        println!("{}", config::run(&config::file(&cwd, *project)?, action.clone())?);
        return Ok(ExitCode::SUCCESS);
    }
    let client = Client::builder().cwd(&cwd).build().map_err(|e| e.to_string())?;
    let launch = launch(&cli, client, cwd)?;
    match cli.command.take() {
        Some(Action::Teammate { .. }) => be_a_teammate(cli, launch, spec),
        Some(action) => subcommand(action, &launch.client, &launch.cwd, cli.model),
        None if cli.print => print_the_answers(cli, launch),
        None if !on_a_terminal() => {
            eprintln!("{brand_command}: no terminal: use -p for print mode", brand_command = brand::COMMAND);
            Ok(ExitCode::from(2))
        }
        None => chat(cli, launch),
    }
}

fn on_a_terminal() -> bool {
    use std::io::IsTerminal;
    std::io::stdin().is_terminal() && std::io::stdout().is_terminal()
}

fn launch(cli: &Cli, client: Client, cwd: PathBuf) -> Result<Launch, String> {
    let prompt = (!cli.prompt.is_empty()).then(|| cli.prompt.join(" "));
    let add_dirs = cli.add_dir.iter().map(|typed| tui::add_dir::folder(&cwd, typed)).collect::<Result<Vec<_>, _>>()?;
    let headless = app::Headless {
        max_turns: cli.max_turns,
        allowed_tools: cli.allowed_tools.iter().flat_map(|rules| rule_list(rules)).collect(),
        disallowed_tools: cli.disallowed_tools.iter().flat_map(|rules| rule_list(rules)).collect(),
        append_system: cli.append_system_prompt.clone(),
        fallback_model: cli.fallback_model.clone(),
    };
    Ok(Launch { client, cwd, prompt, add_dirs, headless })
}

fn subcommand(
    action: Action,
    client: &Client,
    cwd: &std::path::Path,
    model: Option<String>,
) -> Result<ExitCode, String> {
    match action {
        Action::Login => login::login(client).map(|()| ExitCode::SUCCESS),
        Action::Logout => login::logout(client).map(|()| ExitCode::SUCCESS),
        Action::Doctor => {
            let checks = doctor::checks(client, cwd);
            println!("{}", doctor::render(&checks));
            Ok(if checks.iter().all(|c| c.ok) { ExitCode::SUCCESS } else { ExitCode::FAILURE })
        }
        Action::Eval { names, json } => {
            let trust = client.trust_state(cwd).is_ok_and(|report| report.trust.is_trusted());
            evals(
                cwd,
                &names,
                json,
                eval::Process { program: std::env::current_exe().map_err(|e| e.to_string())?, model, trust },
            )
        }
        Action::Models => {
            for model in client.models().map_err(|e| e.to_string())? {
                println!("{:<40} {}", model.id, model.label);
            }
            Ok(ExitCode::SUCCESS)
        }
        Action::Mcp { action } => {
            mcp::run(client, cwd, action.unwrap_or(mcp::McpAction::List)).map(|()| ExitCode::SUCCESS)
        }
        Action::Plugin { action } => {
            plugin::run(client, action.unwrap_or(plugin::PluginAction::List)).map(|()| ExitCode::SUCCESS)
        }
        Action::Remote { action } => {
            remote::run(action.unwrap_or(remote::RemoteAction::Status)).map(|()| ExitCode::SUCCESS)
        }
        Action::Config { .. } | Action::Teammate { .. } => Ok(ExitCode::SUCCESS),
    }
}

fn evals(cwd: &std::path::Path, names: &[String], json: bool, asker: eval::Process) -> Result<ExitCode, String> {
    let outcomes: Vec<_> = eval::cases(cwd, names)?
        .iter()
        .map(|(name, dir)| {
            eprintln!("Running {name}…");
            eval::run_case(&asker, name, dir)
        })
        .collect();
    if json {
        println!("{}", serde_json::to_string_pretty(&outcomes).map_err(|e| e.to_string())?);
    } else {
        println!("{}", domain::evals::table(&outcomes));
    }
    Ok(if outcomes.iter().all(domain::evals::Outcome::ok) { ExitCode::SUCCESS } else { ExitCode::FAILURE })
}

fn print_the_answers(cli: Cli, launch: Launch) -> Result<ExitCode, String> {
    let Launch { client, cwd, prompt, add_dirs, headless } = launch;
    let prompts: Box<dyn Iterator<Item = Result<String, String>>> = match cli.input_format.as_str() {
        "stream-json" => Box::new(print_output::prompts(std::io::stdin().lines())),
        "text" => Box::new(std::iter::once(Ok(prompt.ok_or("--print needs a prompt")?))),
        other => return Err(format!("unknown input format {other}: text or stream-json")),
    };
    let resume = print::resumed(cli.pick.as_deref(), cli.resume, || {
        Ok(tui::last_session(&client.settings().map_err(|e| e.to_string())?, &cwd))
    })?;
    let wanted = print::Wanted {
        model: cli.model.as_deref(),
        mode: cli.mode,
        verification: cli.verification,
        add_dirs,
        headless,
        format: cli.output_format,
        resume,
        fork: cli.fork_session,
        trust: cli.trust,
        asked: None,
    };
    print::print_all(&client, cwd, wanted, prompts)
}

fn be_a_teammate(cli: Cli, launch: Launch, spec: Option<domain::team::TeammateSpec>) -> Result<ExitCode, String> {
    let home = std::env::home_dir().ok_or("a teammate needs a home folder")?;
    let me = match spec {
        Some(spec) => app::team::Me { team: app::team::Team::named(&home, &spec.team), name: spec.name },
        None => app::team::Me::from_env(&home).ok_or("a teammate is started by its lead, with its team and name")?,
    };
    let Launch { client, cwd, add_dirs, headless, .. } = launch;
    let asker = teammate::Mailbox::new(me.clone());
    let forward = |ask: &app::AgentAsk| asker.forward(ask);
    let wanted = print::Wanted {
        model: cli.model.as_deref(),
        mode: cli.mode,
        verification: None,
        add_dirs,
        headless,
        format: cli.output_format,
        resume: None,
        fork: false,
        trust: false,
        asked: Some(&forward),
    };
    let (team, name) = (me.team.clone(), me.name.clone());
    let ended = print::print_all(&client, cwd, wanted, teammate::Mailbox::new(me));
    team.departed(&name);
    ended
}

fn chat(cli: Cli, launch: Launch) -> Result<ExitCode, String> {
    let Launch { client, cwd, prompt, add_dirs, headless } = launch;
    let last = if cli.resume { last_conversation(&client, &cwd)? } else { None };
    let (resume, pick) = match cli.pick {
        Some(id) if !id.is_empty() => (Some(app::SessionId(id)), false),
        Some(_) => (last, true),
        None => (last, false),
    };
    let start = tui::Start {
        resume,
        add_dirs,
        pick,
        headless,
        fork: cli.fork_session,
        inline: cli.inline,
        channels: cli.channels,
    };
    tui::run(client, cwd, cli.model, cli.mode, prompt, None, start).map_err(|e| e.to_string())?;
    Ok(ExitCode::SUCCESS)
}

fn last_conversation(client: &Client, cwd: &std::path::Path) -> Result<Option<app::SessionId>, String> {
    let settings = client.settings().map_err(|e| e.to_string())?;
    let last = tui::last_session(&settings, cwd);
    if last.is_none() {
        eprintln!(
            "{brand_command}: no earlier conversation in this folder; starting a new one",
            brand_command = brand::COMMAND
        );
    }
    Ok(last)
}
