//! The person proving they are at this computer before the app opens:
//! Touch ID through the system's own sheet (which falls back to the login
//! password), behind the `Unlock` seam so the app's lock is tested with
//! stand-ins. Computers with no way to ask answer `Unavailable`.

#[cfg(target_os = "macos")]
mod touch_id;
#[cfg(target_os = "windows")]
mod windows_hello;

#[cfg(target_os = "macos")]
pub use touch_id::TouchId;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum UnlockError {
    #[error("unlocking was cancelled")]
    Cancelled,
    #[error("this computer has no way to unlock")]
    Unavailable,
}

pub trait Unlock: Send + Sync {
    fn unlock(&self, reason: &str) -> Result<(), UnlockError>;

    fn set_parent_window(&self, _window: isize) {}
}

#[derive(Debug, Default)]
pub struct Always;

impl Unlock for Always {
    fn unlock(&self, _: &str) -> Result<(), UnlockError> {
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct Nothing;

impl Unlock for Nothing {
    fn unlock(&self, _: &str) -> Result<(), UnlockError> {
        Err(UnlockError::Unavailable)
    }
}

pub fn system() -> Box<dyn Unlock> {
    #[cfg(target_os = "macos")]
    return Box::new(TouchId);
    #[cfg(target_os = "windows")]
    return Box::new(windows_hello::WindowsHello::default());
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    Box::new(Nothing)
}
