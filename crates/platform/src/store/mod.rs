use std::convert::Infallible;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::file_lock::{Exclusive, Kept, WAIT};

mod error;
#[cfg(test)]
mod tests;

pub use error::Error;

#[derive(Debug, Clone)]
pub struct Document {
    path: PathBuf,
    private: bool,
    unparsable_is_set_aside: bool,
    lock: Kept,
    compact: bool,
    final_newline: bool,
    wait: Duration,
}

impl Document {
    pub fn at(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            private: true,
            unparsable_is_set_aside: true,
            lock: Kept::Beside,
            compact: false,
            final_newline: false,
            wait: WAIT,
        }
    }

    pub fn hand_edited(path: impl Into<PathBuf>) -> Self {
        Self {
            private: false,
            unparsable_is_set_aside: false,
            lock: Kept::OnlyWhileHeld,
            final_newline: true,
            ..Self::at(path)
        }
    }

    pub fn compact(mut self) -> Self {
        self.compact = true;
        self
    }

    pub fn leaving_no_lock_file(mut self) -> Self {
        self.lock = Kept::OnlyWhileHeld;
        self
    }

    pub fn authored(mut self) -> Self {
        self.unparsable_is_set_aside = false;
        self
    }

    pub fn waiting(mut self, wait: Duration) -> Self {
        self.wait = wait;
        self
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn read<T: DeserializeOwned>(&self) -> Result<Option<T>, Error> {
        let target = self.target();
        let bytes = self.bytes(&target)?;
        if let Ok(value) = parsed(bytes.as_deref()) {
            return Ok(value);
        }
        if !self.unparsable_is_set_aside {
            return self.decoded(&target, bytes.as_deref());
        }
        let _held = self.held()?;
        let bytes = self.bytes(&target)?;
        self.decoded(&target, bytes.as_deref())
    }

    pub fn read_or_default<T: Default + DeserializeOwned>(&self) -> T {
        match self.read() {
            Ok(value) => value.unwrap_or_default(),
            Err(error) => {
                tracing::warn!(%error, "what was stored could not be read; going on from nothing");
                T::default()
            }
        }
    }

    pub fn update<T, R>(&self, change: impl FnOnce(&mut T) -> R) -> Result<R, Error>
    where
        T: Default + Serialize + DeserializeOwned,
    {
        match self.try_update(|value| Ok::<R, Infallible>(change(value)))? {
            Ok(answer) => Ok(answer),
            Err(never) => match never {},
        }
    }

    pub fn try_update<T, R, E>(&self, change: impl FnOnce(&mut T) -> Result<R, E>) -> Result<Result<R, E>, Error>
    where
        T: Default + Serialize + DeserializeOwned,
    {
        let _held = self.held()?;
        let target = self.target();
        let bytes = self.bytes(&target)?;
        let mut value = match self.decoded::<T>(&target, bytes.as_deref()) {
            Ok(value) => value.unwrap_or_default(),
            Err(error @ Error::Corrupt { .. }) => {
                tracing::warn!(%error, "what was stored did not parse; going on from nothing");
                T::default()
            }
            Err(error) => return Err(error),
        };
        let before = self.encoded(&value)?;
        let answer = match change(&mut value) {
            Ok(answer) => answer,
            Err(refused) => return Ok(Err(refused)),
        };
        let after = self.encoded(&value)?;
        if after != before {
            self.replace(&target, &after)?;
        }
        Ok(Ok(answer))
    }

    pub fn write<T: Serialize>(&self, value: &T) -> Result<(), Error> {
        self.put(&self.encoded(value)?)
    }

    pub fn read_text(&self) -> Result<Option<String>, Error> {
        let target = self.target();
        self.bytes(&target)?.map(|bytes| self.text(bytes)).transpose()
    }

    pub fn update_text<R>(&self, change: impl FnOnce(&mut String) -> R) -> Result<R, Error> {
        match self.try_update_text(|text| Ok::<R, Infallible>(change(text)))? {
            Ok(answer) => Ok(answer),
            Err(never) => match never {},
        }
    }

    pub fn try_update_text<R, E>(
        &self,
        change: impl FnOnce(&mut String) -> Result<R, E>,
    ) -> Result<Result<R, E>, Error> {
        let _held = self.held()?;
        let target = self.target();
        let before = self.bytes(&target)?.map(|bytes| self.text(bytes)).transpose()?;
        let mut text = before.clone().unwrap_or_default();
        let answer = match change(&mut text) {
            Ok(answer) => answer,
            Err(refused) => return Ok(Err(refused)),
        };
        if before.as_deref() != Some(text.as_str()) && !(before.is_none() && text.is_empty()) {
            self.replace(&target, text.as_bytes())?;
        }
        Ok(Ok(answer))
    }

    pub fn write_text(&self, text: &str) -> Result<(), Error> {
        self.put(text.as_bytes())
    }

    fn put(&self, bytes: &[u8]) -> Result<(), Error> {
        let _held = self.held()?;
        let target = self.target();
        if self.bytes(&target).ok().flatten().as_deref() == Some(bytes) {
            return Ok(());
        }
        self.replace(&target, bytes)
    }

    fn held(&self) -> Result<Exclusive, Error> {
        Exclusive::within(&self.path, self.wait, self.lock)
            .map_err(|source| self.io(source))?
            .ok_or_else(|| Error::Busy { path: self.path.clone(), waited: self.wait })
    }

    fn target(&self) -> PathBuf {
        let linked = std::fs::symlink_metadata(&self.path).is_ok_and(|found| found.file_type().is_symlink());
        linked.then(|| std::fs::canonicalize(&self.path).ok()).flatten().unwrap_or_else(|| self.path.clone())
    }

    fn bytes(&self, target: &Path) -> Result<Option<Vec<u8>>, Error> {
        match std::fs::read(target) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(source) => Err(self.io(source)),
        }
    }

    fn text(&self, bytes: Vec<u8>) -> Result<String, Error> {
        String::from_utf8(bytes).map_err(|error| self.io(io::Error::new(io::ErrorKind::InvalidData, error)))
    }

    fn decoded<T: DeserializeOwned>(&self, target: &Path, bytes: Option<&[u8]>) -> Result<Option<T>, Error> {
        let error = match parsed(bytes) {
            Ok(value) => return Ok(value),
            Err(error) => error,
        };
        let (path, reason) = (self.path.clone(), error.to_string());
        if !self.unparsable_is_set_aside {
            return Err(Error::Invalid { path, reason });
        }
        match set_aside(target) {
            Ok(kept_at) => Err(Error::Corrupt { path, kept_at, reason }),
            Err(source) => Err(Error::Io { path, source }),
        }
    }

    fn encoded<T: Serialize>(&self, value: &T) -> Result<Vec<u8>, Error> {
        let encoded = if self.compact { serde_json::to_vec(value) } else { serde_json::to_vec_pretty(value) };
        let mut bytes = encoded.map_err(|source| Error::Serialize { path: self.path.clone(), source })?;
        if self.final_newline {
            bytes.push(b'\n');
        }
        Ok(bytes)
    }

    fn replace(&self, target: &Path, bytes: &[u8]) -> Result<(), Error> {
        let replaced = if self.private {
            crate::private_file::replace(target, bytes)
        } else {
            crate::private_file::replace_keeping_mode(target, bytes)
        };
        replaced.map_err(|source| self.io(source))
    }

    fn io(&self, source: io::Error) -> Error {
        Error::Io { path: self.path.clone(), source }
    }
}

fn parsed<T: DeserializeOwned>(bytes: Option<&[u8]>) -> Result<Option<T>, serde_json::Error> {
    match bytes.filter(|bytes| !bytes.iter().all(u8::is_ascii_whitespace)) {
        Some(bytes) => serde_json::from_slice(bytes).map(Some),
        None => Ok(None),
    }
}

fn set_aside(target: &Path) -> io::Result<PathBuf> {
    let name = target.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
    let epoch = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |since| since.as_secs());
    let first = target.with_file_name(format!("{name}.corrupt-{epoch}"));
    let aside = std::iter::once(first)
        .chain((1..).map(|again| target.with_file_name(format!("{name}.corrupt-{epoch}-{again}"))))
        .find(|candidate| !candidate.exists())
        .ok_or_else(|| io::Error::other("no free name to keep the unreadable file under"))?;
    std::fs::rename(target, &aside)?;
    Ok(aside)
}
