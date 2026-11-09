//! `superfik-server`: a team's Superfik server, self-hosted (ADR 0002,
//! 0017). `superfik-server --help` lists its commands (`cli`).
//!
//! Environment: `SUPERFIK_LISTEN` (default `0.0.0.0:7420`; on a Mac
//! `127.0.0.1:7420`), `SUPERFIK_DATA` (default `/var/lib/superfik`, on a
//! Mac under Application Support: the users, settings, sessions and
//! memory), `SUPERFIK_WEB` the folder of the web app's bundle (served at
//! `/`, beside `/ws`). Put it behind a TLS reverse proxy
//! (`superfik-server install --domain` sets up Caddy).

use std::sync::Arc;

use superfik_backend::{LocalServices, SealedSecretStore};
use superfik_server::cli::{self, Command};
use superfik_server::config::{self, Config};
use superfik_server::{browser_pages, hosting, install, serve, system_clock};

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("superfik-server: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let helper = args.iter().any(|arg| arg.starts_with("--type="));
    let command = if helper { Command::Serve } else { cli::parse(&args)? };
    let config = Config::on(|name| std::env::var(name).ok(), config::Host::this())?;
    match command {
        Command::Help | Command::Version => {
            println!("{}", cli::run(&command, &config)?);
            Ok(())
        }
        Command::Install { domain, user } => install(&config, domain, user),
        command => {
            cli::as_data_owner(install::effective_uid(), data_owner(&config), &config.data)?;
            match command {
                Command::Serve => serve_organization(&config),
                command => {
                    println!("{}", cli::run(&command, &config)?);
                    Ok(())
                }
            }
        }
    }
}

fn data_owner(config: &Config) -> Option<u32> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        std::fs::metadata(&config.data).ok().map(|meta| meta.uid())
    }
    #[cfg(not(unix))]
    {
        let _ = config;
        None
    }
}

fn serve_organization(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).init();
    let pages = browser_pages::wanted(std::env::var("SUPERFIK_PAGES").ok().as_deref());
    if pages && let Some(code) = browser_pages::prepare() {
        std::process::exit(code);
    }
    match config::native_worker(std::env::var("SUPERFIK_NATIVE_WORKER").ok())? {
        Some(worker) => tracing::info!(worker = %worker.display(), "members' agent tools run through the worker"),
        None => tracing::warn!(
            "SUPERFIK_NATIVE_WORKER is not set: members' agent runs are refused until it is \
             (`superfik-server install` and the Docker image set it)"
        ),
    }
    let organization = Arc::new(cli::organization(config, system_clock())?);
    if !organization.directory.has_owner()? {
        tracing::warn!("this server has no users yet: run `superfik-server init` to add its owner");
    }
    let org = organization.directory.org();

    let mut services = LocalServices::detect();
    let pages = pages.then(|| browser_pages::start(&config.data)).map(|(service, page_loop)| {
        services.pages = service;
        page_loop
    });
    let routes =
        hosting::open(&config.data, organization, services, secret_key(config)?).map_err(|error| error.to_string())?;
    let web = std::env::var_os("SUPERFIK_WEB").map(std::path::PathBuf::from);
    let routes = superfik_server::web_app::with_web_app(routes, web.as_deref());

    let listen = config.listen;
    let serving = move || -> Result<(), String> {
        let runtime = tokio::runtime::Runtime::new().map_err(|error| error.to_string())?;
        runtime.block_on(async {
            let listener = tokio::net::TcpListener::bind(listen).await.map_err(|error| error.to_string())?;
            tracing::info!(%listen, %org, "serving");
            serve(listener, routes).await.map_err(|error| error.to_string())
        })
    };
    match pages {
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        Some(page_loop) => {
            let server = std::thread::Builder::new().name("superfik-serve".to_owned()).spawn(serving)?;
            page_loop.run(|| server.is_finished());
            server.join().map_err(|_| "the server stopped unexpectedly")??;
        }
        _ => serving()?,
    }
    Ok(())
}

fn secret_key(config: &Config) -> Result<[u8; 32], Box<dyn std::error::Error>> {
    Ok(match std::env::var("SUPERFIK_SECRET_KEY") {
        Ok(hex) => config::secret_key(&hex).ok_or("SUPERFIK_SECRET_KEY is not 64 hex digits")?,
        Err(_) => SealedSecretStore::key_at(&config.data.join("secrets.key"))?,
    })
}

fn install(config: &Config, domain: Option<String>, user: String) -> Result<(), Box<dyn std::error::Error>> {
    if !cfg!(target_os = "linux") {
        return Err("`install` sets up a systemd service: run it on the Linux server, as root".into());
    }
    if install::effective_uid() != Some(0) {
        return Err("`install` sets up users, sudo and systemd: run it as root (sudo)".into());
    }
    let plan = install::Plan::new(config, domain, user)?;
    let running = std::env::current_exe()?.canonicalize()?;
    if running != plan.binary() {
        return Err(format!(
            "this is {}; install runs the server from {}: use scripts/install-server.sh",
            running.display(),
            plan.binary().display()
        )
        .into());
    }
    println!("{}", install::install(&plan, config, &mut install::System)?);
    Ok(())
}
