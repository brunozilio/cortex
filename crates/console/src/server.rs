use std::net::{Ipv4Addr, SocketAddr, TcpListener};
use std::path::PathBuf;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use tokio::sync::oneshot;

use crate::{Backend, Settings};

const SHUTDOWN_GRACE: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Default)]
pub struct Options {
    pub assets: Option<PathBuf>,
    pub port: u16,
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

    pub fn url(&self) -> String {
        format!("http://{}/?token={}", self.address, self.token)
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

pub fn start(backend: Arc<dyn Backend>, options: Options) -> Result<Running, StartError> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, options.port)).map_err(StartError::Listen)?;
    listener.set_nonblocking(true).map_err(StartError::Listen)?;
    let address = listener.local_addr().map_err(StartError::Listen)?;
    let token = secret()?;
    let app = crate::router(backend, Settings { token: token.clone(), assets: options.assets });
    let (stop, stopped) = oneshot::channel::<()>();
    let thread = std::thread::Builder::new()
        .name("console".into())
        .spawn(move || serve(listener, app, stopped))
        .map_err(StartError::Thread)?;
    Ok(Running { address, token, stop: Some(stop), thread: Some(thread) })
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

fn secret() -> Result<String, StartError> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|error| StartError::Secret(error.to_string()))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}
