use std::net::{SocketAddr, TcpListener};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

use tokio::sync::oneshot;

use crate::Limits;

const GRACE: Duration = Duration::from_secs(1);

#[derive(Debug)]
pub struct Serving {
    address: SocketAddr,
    interrupted: Arc<AtomicBool>,
    stop: Option<oneshot::Sender<()>>,
    thread: Option<JoinHandle<()>>,
}

impl Serving {
    pub fn address(&self) -> SocketAddr {
        self.address
    }

    pub fn interrupted(&self) -> bool {
        self.interrupted.load(Ordering::SeqCst)
    }
}

impl Drop for Serving {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

pub fn spawn(listener: TcpListener, limits: Limits, bundle: &'static [page::Asset]) -> std::io::Result<Serving> {
    listener.set_nonblocking(true)?;
    let address = listener.local_addr()?;
    let interrupted = Arc::new(AtomicBool::new(false));
    let noticed = interrupted.clone();
    let (stop, stopped) = oneshot::channel::<()>();
    let runtime = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build()?;
    let thread = std::thread::Builder::new().name("relay".into()).spawn(move || {
        runtime.block_on(async move {
            let Ok(listener) = tokio::net::TcpListener::from_std(listener) else { return };
            let server = axum::serve(listener, crate::router(limits, bundle));
            tokio::select! {
                _ = server => {}
                _ = stopped => {}
                _ = tokio::signal::ctrl_c() => noticed.store(true, Ordering::SeqCst),
            }
        });
        runtime.shutdown_timeout(GRACE);
    })?;
    Ok(Serving { address, interrupted, stop: Some(stop), thread: Some(thread) })
}
