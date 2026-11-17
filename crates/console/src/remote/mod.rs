mod book;
mod channel;
mod forward;
mod link;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::thread::JoinHandle;

use tokio::sync::{oneshot, watch};

pub use book::{Book, BookError, Device, PAIRING_LASTS, now};
pub use link::{Pace, RelayAddressError, machine_socket, relay_address};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    Waiting,
    Connecting,
    Connected,
    Taken,
    Failed(String),
}

#[derive(Debug, Clone)]
pub struct Options {
    pub relay: String,
    pub folder: PathBuf,
    pub console: SocketAddr,
    pub token: String,
    pub pace: Pace,
}

impl Options {
    pub fn for_console_link(link: &str, relay: String, folder: PathBuf) -> Option<Self> {
        let rest = link.strip_prefix("http://")?;
        let (address, rest) = rest.split_once('/')?;
        let console = address.parse().ok()?;
        let query = rest.split_once('?').map_or("", |(_, query)| query);
        let token = query.split('&').find_map(|pair| pair.strip_prefix("token="))?;
        let token = token.split('#').next().filter(|token| !token.is_empty())?.to_owned();
        Some(Self { relay, folder, console, token, pace: Pace::default() })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum StartError {
    #[error(transparent)]
    Address(#[from] RelayAddressError),
    #[error(transparent)]
    Book(#[from] BookError),
    #[error("could not start the remote control's thread: {0}")]
    Thread(std::io::Error),
}

#[derive(Debug)]
pub struct Remote {
    status: watch::Receiver<Status>,
    stop: Option<oneshot::Sender<()>>,
    thread: Option<JoinHandle<()>>,
}

impl Remote {
    pub fn status(&self) -> Status {
        self.status.borrow().clone()
    }

    pub fn watch(&self) -> watch::Receiver<Status> {
        self.status.clone()
    }
}

impl Drop for Remote {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

pub fn start(options: Options) -> Result<Remote, StartError> {
    let book = Book::in_folder(&options.folder);
    let machine = book.machine()?;
    let socket = machine_socket(&options.relay, &machine.machine_id())?;
    let place = channel::Place { book, machine, local: forward::Local::new(options.console, &options.token) };
    let (status, watching) = watch::channel(Status::Connecting);
    let (stop, stopped) = oneshot::channel();
    let pace = options.pace;
    let thread = std::thread::Builder::new()
        .name("remote".into())
        .spawn(move || {
            let Ok(runtime) = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build() else {
                return;
            };
            runtime.block_on(link::keep_up(place, socket, pace, status, stopped));
            runtime.shutdown_timeout(std::time::Duration::from_secs(1));
        })
        .map_err(StartError::Thread)?;
    Ok(Remote { status: watching, stop: Some(stop), thread: Some(thread) })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "a failed step fails the test")]
mod tests {
    use super::*;

    #[test]
    fn the_console_link_gives_the_address_and_token_the_tunnel_forwards_with() {
        let options = Options::for_console_link("http://127.0.0.1:4321/?token=abc123", "https://r".into(), "/f".into()).unwrap();
        assert_eq!(options.console, "127.0.0.1:4321".parse().unwrap());
        assert_eq!(options.token, "abc123");
        let deeper = Options::for_console_link("http://127.0.0.1:9/docs?x=1&token=t", "https://r".into(), "/f".into()).unwrap();
        assert_eq!(deeper.token, "t");
    }

    #[test]
    fn a_link_without_a_loopback_address_or_a_token_gives_nothing() {
        for link in ["https://127.0.0.1:1/?token=a", "http://127.0.0.1:1/", "http://127.0.0.1:1/?token=", "http://host:1/?token=a"] {
            assert!(Options::for_console_link(link, "https://r".into(), "/f".into()).is_none(), "{link}");
        }
    }
}
