use std::net::{Ipv4Addr, TcpListener};
use std::time::Duration;

use console::remote::Book;

use super::cloudflare::{self, Cloudflared, Running};

pub const DEFAULT_PORT: u16 = 7468;
const READY_WITHIN: Duration = Duration::from_secs(45);
const LOOK_EVERY: Duration = Duration::from_millis(250);

#[derive(Debug, Clone)]
pub struct Plan {
    pub hostname: Option<String>,
    pub port: u16,
}

#[derive(Debug)]
pub struct Up {
    pub public: String,
    pub quick: bool,
    relay: relay::Serving,
    tunnel: Running,
}

pub fn tunnel_name() -> String {
    format!("{}-relay", brand::COMMAND)
}

impl Up {
    pub fn local_port(&self) -> u16 {
        self.relay.address().port()
    }
}

pub fn start(book: &Book, cloudflared: &Cloudflared, plan: &Plan, say: &mut dyn FnMut(&str)) -> Result<Up, String> {
    let asked = plan.hostname.as_deref().map(cloudflare::hostname).transpose()?;
    let hostname = asked.or_else(|| book.hostname());
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, plan.port)).map_err(|error| {
        format!("could not listen on 127.0.0.1:{}: {error}; is `{} remote up` already running?", plan.port, brand::COMMAND)
    })?;
    let relay = relay::spawn(listener, relay::Limits::default(), relay::embedded_page())
        .map_err(|error| format!("the relay could not start: {error}"))?;
    let port = relay.address().port();
    let (tunnel, public) = match &hostname {
        Some(host) => (named(cloudflared, host, port, say)?, format!("https://{host}")),
        None => {
            say("No host name given: starting a quick tunnel, whose address changes every time.");
            let mut tunnel = cloudflared.run_quick(port)?;
            let public = tunnel.wait_for(cloudflare::quick_address, READY_WITHIN)?;
            (tunnel, public)
        }
    };
    let local = format!("http://127.0.0.1:{port}");
    let public = book.serve_here(&public, &local, hostname.as_deref()).map_err(|error| error.to_string())?;
    Ok(Up { public, quick: hostname.is_none(), relay, tunnel })
}

fn named(cloudflared: &Cloudflared, host: &str, port: u16, say: &mut dyn FnMut(&str)) -> Result<Running, String> {
    if !cloudflared.signed_in() {
        say(&format!("Sign in to Cloudflare in the browser that opens, and pick the domain {host} belongs to."));
        cloudflared.sign_in()?;
    }
    let name = tunnel_name();
    if cloudflared.tunnel_id(&name)?.is_none() {
        say(&format!("Creating the Cloudflare tunnel {name}…"));
        cloudflared.create(&name)?;
    }
    say(&format!("Pointing {host} at the tunnel…"));
    cloudflared.route(&name, host)?;
    let mut tunnel = cloudflared.run_named(&name, port)?;
    tunnel.wait_for(cloudflare::connected, READY_WITHIN)?;
    Ok(tunnel)
}

pub fn run(book: &Book, plan: &Plan) -> Result<(), String> {
    let cloudflared = Cloudflared::find().ok_or(cloudflare::INSTALL)?;
    let mut up = start(book, &cloudflared, plan, &mut |line| println!("{line}"))?;
    println!("\nThe relay is up at {} (on this machine: 127.0.0.1:{}).", up.public, up.local_port());
    if up.quick || book.devices().is_empty() {
        println!("\n{}", super::answer(book, super::RemoteAction::Pair)?);
    }
    if up.quick {
        println!("\nA quick tunnel's address changes each time: a phone pairs again after every `{} remote up`.", brand::COMMAND);
    }
    println!("\nLeave this running; an open chat connects by itself. Ctrl-C stops the relay and the tunnel.");
    loop {
        std::thread::sleep(LOOK_EVERY);
        up.tunnel.drain();
        if up.relay.interrupted() {
            println!("Stopped.");
            return Ok(());
        }
        if !up.tunnel.alive() {
            return Err(format!("cloudflared stopped:\n{}", up.tunnel.said()));
        }
    }
}

#[cfg(all(test, unix))]
#[path = "up_tests.rs"]
mod tests;
