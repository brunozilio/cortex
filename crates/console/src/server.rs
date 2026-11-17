use std::net::{Ipv4Addr, SocketAddr, TcpListener};
use std::path::PathBuf;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use tokio::sync::oneshot;

use crate::{Assets, Entries, Launch, Live, Settings};

const SHUTDOWN_GRACE: Duration = Duration::from_secs(1);

#[derive(Debug, Clone)]
pub struct Options {
    pub assets: Assets,
    pub port: u16,
    pub folder: PathBuf,
}

fn settings(options: Options, token: &str, port: u16, entries: Arc<Entries>) -> Settings {
    Settings {
        token: token.to_owned(),
        port,
        assets: options.assets,
        terminal: Launch::this_program(options.folder),
        entries,
    }
}

#[derive(Debug, thiserror::Error)]
pub enum StartError {
    #[error("could not listen on 127.0.0.1: {0}")]
    Listen(std::io::Error),
    #[error("could not make the console's secret: {0}")]
    Secret(String),
    #[error("could not start the console's thread: {0}")]
    Thread(std::io::Error),
}

#[derive(Debug)]
pub struct Running {
    address: SocketAddr,
    token: String,
    entries: Arc<Entries>,
    live: Live,
    stop: Option<oneshot::Sender<()>>,
    thread: Option<JoinHandle<()>>,
}

impl Running {
    pub fn address(&self) -> SocketAddr {
        self.address
    }

    pub fn token(&self) -> &str {
        &self.token
    }

    pub fn live(&self) -> Live {
        self.live.clone()
    }

    pub fn url(&self) -> String {
        self.url_at("/")
    }

    pub fn url_at(&self, path: &str) -> String {
        let path = path.split('#').next().unwrap_or_default();
        match self.entries.mint() {
            Some(code) => format!("http://{}{path}#enter={code}", self.address),
            None => format!("http://{}{path}", self.address),
        }
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

pub fn start(options: Options) -> Result<Running, StartError> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, options.port)).map_err(StartError::Listen)?;
    listener.set_nonblocking(true).map_err(StartError::Listen)?;
    let address = listener.local_addr().map_err(StartError::Listen)?;
    let token = crate::access::random_hex().ok_or_else(|| StartError::Secret("no randomness".into()))?;
    let live = Live::default();
    let entries = Arc::new(Entries::default());
    let app = crate::router(settings(options, &token, address.port(), Arc::clone(&entries)), live.clone());
    let (stop, stopped) = oneshot::channel::<()>();
    let thread = std::thread::Builder::new()
        .name("console".into())
        .spawn(move || serve(listener, app, stopped))
        .map_err(StartError::Thread)?;
    Ok(Running { address, token, entries, live, stop: Some(stop), thread: Some(thread) })
}

fn serve(listener: TcpListener, app: axum::Router, stopped: oneshot::Receiver<()>) {
    let Ok(runtime) = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build() else {
        return;
    };
    runtime.block_on(async move {
        let Ok(listener) = tokio::net::TcpListener::from_std(listener) else { return };
        let (closing, mut closed) = tokio::sync::watch::channel(false);
        let server = axum::serve(listener, app).with_graceful_shutdown(async move {
            let _ = closed.wait_for(|closing| *closing).await;
        });
        let deadline = async move {
            let _ = stopped.await;
            let _ = closing.send(true);
            tokio::time::sleep(SHUTDOWN_GRACE).await;
        };
        tokio::select! {
            _ = server => {}
            _ = deadline => {}
        }
    });
    runtime.shutdown_timeout(SHUTDOWN_GRACE);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_page_s_terminal_starts_in_the_folder_the_console_was_started_for() {
        let project = PathBuf::from("/work/project");
        let options = Options { assets: Assets::Missing, port: 0, folder: project.clone() };
        let made = settings(options, "secret", 4321, Arc::default());
        assert_eq!(made.terminal.cwd, project);
        assert_eq!((made.token.as_str(), made.port), ("secret", 4321));
    }
}
