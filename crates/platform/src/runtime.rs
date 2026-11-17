use std::cell::Cell;
use std::future::Future;
use std::num::NonZeroUsize;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};

use tokio::runtime::{Builder, Handle, Runtime, RuntimeFlavor};

pub const WORKERS_AT_MOST: usize = 4;
pub const BLOCKING_AT_MOST: usize = 256;
pub const WORKER_NAME: &str = "engine-worker";
pub const BLOCKING_NAME: &str = "engine-blocking";

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Unavailable {
    #[error("the async runtime could not start: {0}")]
    NotStarted(String),
    #[error("a blocking wait panicked")]
    Panicked,
}

static SHARED: OnceLock<Result<Runtime, String>> = OnceLock::new();

thread_local! {
    static WAITING: Cell<bool> = const { Cell::new(false) };
}

struct Waiting;

impl Waiting {
    fn enter() -> Self {
        WAITING.set(true);
        Self
    }
}

impl Drop for Waiting {
    fn drop(&mut self) {
        WAITING.set(false);
    }
}

pub fn workers() -> usize {
    std::thread::available_parallelism().map_or(2, NonZeroUsize::get).clamp(2, WORKERS_AT_MOST)
}

fn build() -> Result<Runtime, String> {
    let named = AtomicUsize::new(0);
    Builder::new_multi_thread()
        .worker_threads(workers())
        .max_blocking_threads(BLOCKING_AT_MOST)
        .thread_name_fn(move || {
            let name = if named.fetch_add(1, Ordering::Relaxed) < workers() { WORKER_NAME } else { BLOCKING_NAME };
            name.to_owned()
        })
        .enable_all()
        .build()
        .map_err(|error| error.to_string())
}

pub fn shared() -> Result<&'static Runtime, Unavailable> {
    SHARED.get_or_init(build).as_ref().map_err(|error| Unavailable::NotStarted(error.clone()))
}

pub fn handle() -> Result<Handle, Unavailable> {
    match Handle::try_current() {
        Ok(handle) => Ok(handle),
        Err(_) => Ok(shared()?.handle().clone()),
    }
}

fn aside<F>(future: F) -> Result<F::Output, Unavailable>
where
    F: Future + Send,
    F::Output: Send,
{
    std::thread::scope(|scope| scope.spawn(|| block_on(future)).join()).map_err(|_| Unavailable::Panicked)?
}

pub fn block_on<F>(future: F) -> Result<F::Output, Unavailable>
where
    F: Future + Send,
    F::Output: Send,
{
    if WAITING.get() {
        return aside(future);
    }
    match Handle::try_current() {
        Ok(handle) if handle.runtime_flavor() == RuntimeFlavor::CurrentThread => aside(future),
        Ok(handle) => Ok(tokio::task::block_in_place(|| {
            let _waiting = Waiting::enter();
            handle.block_on(future)
        })),
        Err(_) => {
            let runtime = shared()?;
            let _waiting = Waiting::enter();
            Ok(runtime.block_on(future))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn the_shared_runtime_is_built_once_with_a_bounded_number_of_named_workers() {
        let first = shared().expect("runtime") as *const Runtime;
        let second = shared().expect("runtime") as *const Runtime;
        assert_eq!(first, second);
        assert!((2..=WORKERS_AT_MOST).contains(&workers()));
        let name = shared()
            .expect("runtime")
            .block_on(async { tokio::spawn(async { std::thread::current().name().map(str::to_owned) }).await })
            .expect("task");
        assert_eq!(name.as_deref(), Some(WORKER_NAME));
    }

    #[test]
    fn a_blocking_wait_works_from_a_plain_thread_a_worker_and_a_blocking_thread() {
        assert_eq!(block_on(async { 1 }), Ok(1));
        let runtime = shared().expect("runtime");
        let on_worker = runtime.block_on(async { tokio::spawn(async { block_on(async { 2 }) }).await });
        assert_eq!(on_worker.expect("task"), Ok(2));
        let on_blocking = runtime.block_on(async { tokio::task::spawn_blocking(|| block_on(async { 3 })).await });
        assert_eq!(on_blocking.expect("task"), Ok(3));
    }

    #[test]
    fn a_blocking_wait_inside_another_one_is_served_aside_instead_of_panicking() {
        let nested = block_on(async { block_on(async { 4 }) });
        assert_eq!(nested, Ok(Ok(4)));
        let single = Builder::new_current_thread().enable_all().build().expect("runtime");
        let slept = single.block_on(async { block_on(async { tokio::time::sleep(Duration::from_millis(1)).await }) });
        assert_eq!(slept, Ok(()));
    }
}
