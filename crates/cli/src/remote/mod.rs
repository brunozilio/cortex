mod workers;

use std::path::PathBuf;

use clap::Subcommand;
use console::remote::{Book, Device, PAIRING_LASTS};

#[derive(Debug, Subcommand)]
pub enum RemoteAction {
    #[command(about = "Show the relay, this machine's id and the paired devices")]
    Status,
    #[command(
        visible_alias = "update",
        about = "Publish or update the relay as a Worker on your Cloudflare account (workers.dev) and use it; signs in to Cloudflare in the browser"
    )]
    Up {
        #[arg(long, help = "The Cloudflare account id, when the sign-in reaches more than one")]
        account: Option<String>,
    },
    #[command(about = "Forget the Cloudflare sign-in `remote up` keeps")]
    Logout,
    #[command(about = "Name the relay phones reach this machine through, as https://<host>")]
    Relay { address: String },
    #[command(about = "Invite a phone: a QR code and a link, good for one phone within ten minutes")]
    Pair,
    #[command(about = "List the paired devices")]
    Devices,
    #[command(about = "Forget a paired device; its open connections are cut within seconds")]
    Revoke { name: String },
}

pub fn folder() -> Option<PathBuf> {
    platform::data_dir(brand::NAME, brand::COMMAND).map(|data| data.join("remote"))
}

pub fn run(action: RemoteAction) -> Result<(), String> {
    let book = Book::in_folder(&folder().ok_or("this system has no data folder")?);
    match action {
        RemoteAction::Up { account } => return up(&book, account),
        RemoteAction::Logout => {
            println!("{}", sign_out()?);
            return Ok(());
        }
        _ => {}
    }
    println!("{}", answer(&book, action)?);
    Ok(())
}

fn token() -> Result<String, String> {
    if let Some(token) = std::env::var(workers::TOKEN_VARIABLE).ok().filter(|token| !token.trim().is_empty()) {
        return Ok(token.trim().to_owned());
    }
    let sign_in = app::CloudflareSignIn::system()?;
    sign_in.token(&std::sync::Arc::default(), &mut |url| {
        println!("Sign in to Cloudflare in the browser and choose the account for the relay:\n{url}\n");
        crate::login::open(url);
    })
}

fn sign_out() -> Result<String, String> {
    Ok(if app::CloudflareSignIn::system()?.sign_out()? {
        "Signed out of Cloudflare; the relay keeps running.".to_owned()
    } else {
        "Not signed in to Cloudflare.".to_owned()
    })
}

fn up(book: &Book, account: Option<String>) -> Result<(), String> {
    let api = workers::Cloudflare::new(token()?)?;
    let address = workers::deploy(&api, account, relay::embedded_page(), &mut |line| println!("{line}"))?;
    let kept = book.set_relay(&address).map_err(|error| error.to_string())?;
    println!("\nThe relay is up at {kept}. A new workers.dev address can take a minute to answer.");
    if book.devices().is_empty() {
        println!("\n{}", pair(book)?);
    }
    Ok(())
}

pub fn answer(book: &Book, action: RemoteAction) -> Result<String, String> {
    match action {
        RemoteAction::Status => status(book),
        RemoteAction::Up { .. } => {
            Err(format!("`{} remote up` runs from a terminal, where it can sign in to Cloudflare.", brand::COMMAND))
        }
        RemoteAction::Logout => sign_out(),
        RemoteAction::Relay { address } => {
            let kept = book.set_relay(&address).map_err(|error| error.to_string())?;
            Ok(format!("Phones will reach this machine through {kept}. `{} remote pair` invites one.", brand::COMMAND))
        }
        RemoteAction::Pair => pair(book),
        RemoteAction::Devices => Ok(devices(&book.devices())),
        RemoteAction::Revoke { name } => {
            let device = book.revoke(&name).map_err(|error| error.to_string())?;
            Ok(format!("{} is no longer paired; its open connections close within seconds.", device.name))
        }
    }
}

fn status(book: &Book) -> Result<String, String> {
    let machine = book.machine().map_err(|error| error.to_string())?;
    let relay = book.relay().unwrap_or_else(|| format!("none yet (`{} remote relay https://<host>`)", brand::COMMAND));
    Ok(format!("Relay:    {relay}\nMachine:  {}\n\n{}", machine.machine_id(), devices(&book.devices())))
}

fn pair(book: &Book) -> Result<String, String> {
    let relay = book
        .relay()
        .ok_or_else(|| format!("Name the relay first: `{} remote relay https://<host>`.", brand::COMMAND))?;
    let link = book.invite().map_err(|error| error.to_string())?.url(&relay);
    let minutes = PAIRING_LASTS.as_secs() / 60;
    Ok(format!(
        "{}\n{link}\n\nScan the code or open the link on the phone. A page on an iPhone's home screen keeps its own pairing: open it from there and paste this link.\n\
         It works for one phone within {minutes} minutes, while a chat with remote control is open on this machine.",
        qr(&link)?
    ))
}

pub fn qr(text: &str) -> Result<String, String> {
    let code = qrcode::QrCode::new(text.as_bytes()).map_err(|error| error.to_string())?;
    Ok(code
        .render::<qrcode::render::unicode::Dense1x2>()
        .dark_color(qrcode::render::unicode::Dense1x2::Light)
        .light_color(qrcode::render::unicode::Dense1x2::Dark)
        .quiet_zone(true)
        .build())
}

fn devices(devices: &[Device]) -> String {
    if devices.is_empty() {
        return format!("No device is paired. `{} remote pair` invites one.", brand::COMMAND);
    }
    let rows: Vec<String> =
        devices.iter().map(|device| format!("{:<32} paired {}", device.name, when(device.paired_at))).collect();
    format!("Paired devices:\n{}", rows.join("\n"))
}

fn when(seconds: u64) -> String {
    i64::try_from(seconds)
        .ok()
        .and_then(|seconds| jiff::Timestamp::from_second(seconds).ok())
        .map_or_else(|| "at an unknown time".to_owned(), |at| at.strftime("%Y-%m-%d %H:%M UTC").to_string())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "a failed step fails the test")]
mod tests {
    use super::*;

    fn book() -> (tempfile::TempDir, Book) {
        let folder = tempfile::tempdir().unwrap();
        let book = Book::in_folder(folder.path());
        (folder, book)
    }

    #[test]
    fn update_is_another_name_for_up() {
        #[derive(clap::Parser)]
        struct Remote {
            #[command(subcommand)]
            action: RemoteAction,
        }
        let parsed = <Remote as clap::Parser>::try_parse_from(["remote", "update", "--account", "abc"]).unwrap();
        assert!(matches!(parsed.action, RemoteAction::Up { account: Some(account) } if account == "abc"));
    }

    #[test]
    fn pairing_needs_a_relay_first() {
        let (_folder, book) = book();
        assert!(answer(&book, RemoteAction::Pair).unwrap_err().contains("remote relay https://"));
    }

    #[test]
    fn pairing_prints_a_code_and_a_link_whose_secret_rides_in_the_fragment() {
        let (_folder, book) = book();
        answer(&book, RemoteAction::Relay { address: "https://relay.example/".into() }).unwrap();
        let told = answer(&book, RemoteAction::Pair).unwrap();
        let link = told.lines().find(|line| line.starts_with("https://")).unwrap();
        assert!(link.starts_with("https://relay.example/#pair="), "{link}");
        assert!(told.contains('█') || told.contains('▀') || told.contains('▄'));
        assert!(book.anyone_to_serve());
    }

    #[test]
    fn a_relay_without_https_is_refused_with_the_reason() {
        let (_folder, book) = book();
        let refused = answer(&book, RemoteAction::Relay { address: "http://relay.example".into() }).unwrap_err();
        assert!(refused.contains("https://"), "{refused}");
    }

    #[test]
    fn status_names_the_machine_and_says_when_nothing_is_paired() {
        let (_folder, book) = book();
        let told = answer(&book, RemoteAction::Status).unwrap();
        assert!(told.contains(&book.machine().unwrap().machine_id()));
        assert!(told.contains("No device is paired"));
    }

    #[test]
    fn revoking_an_unknown_device_names_it() {
        let (_folder, book) = book();
        assert!(answer(&book, RemoteAction::Revoke { name: "ghost".into() }).unwrap_err().contains("ghost"));
    }
}
