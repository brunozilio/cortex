use std::path::Path;

use platform::keychain::Keychain;

use super::sealed::{hex, unhex};

pub const IN_KEYCHAIN: &str = "keychain";
pub const CHOICE: &str = concat!(brand::env_prefix!(), "_SECRETS");

#[derive(Clone, Copy)]
pub enum Keeper<'a> {
    File,
    FileInsteadOf(&'a dyn Keychain),
    Keychain(&'a dyn Keychain),
}

impl<'a> Keeper<'a> {
    pub fn chosen(choice: Option<&str>, keychain: &'a dyn Keychain) -> Self {
        if choice.is_some_and(|choice| choice.trim().eq_ignore_ascii_case(IN_KEYCHAIN)) {
            Self::Keychain(keychain)
        } else {
            Self::FileInsteadOf(keychain)
        }
    }
}

pub fn at(path: &Path, keeper: Keeper<'_>) -> Result<[u8; 32], String> {
    if let Some(key) = kept(path, keeper)? {
        return Ok(key);
    }
    let made = platform::store::Document::at(path).try_update_text(|text| settled(path, keeper, text));
    made.map_err(|error| unusable(&error))?
}

fn kept(path: &Path, keeper: Keeper<'_>) -> Result<Option<[u8; 32]>, String> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(unusable(&format!("{}: {error}", path.display()))),
    };
    match (keeper, text.trim()) {
        (Keeper::Keychain(keychain), IN_KEYCHAIN) => from_keychain(path, keychain).map(Some),
        (Keeper::Keychain(_), _) | (_, IN_KEYCHAIN) => Ok(None),
        (_, text) => read(path, text).map(Some),
    }
}

fn settled(path: &Path, keeper: Keeper<'_>, text: &mut String) -> Result<[u8; 32], String> {
    match (keeper, text.trim()) {
        (Keeper::Keychain(keychain), IN_KEYCHAIN) => from_keychain(path, keychain),
        (Keeper::Keychain(keychain), "") => first_in(path, keychain, text),
        (Keeper::Keychain(keychain), kept) => {
            let key = read(path, kept)?;
            if stored(path, keychain, &key) {
                IN_KEYCHAIN.clone_into(text);
            }
            Ok(key)
        }
        (Keeper::FileInsteadOf(keychain), IN_KEYCHAIN) => {
            let key = match keychain.get(&account(path)) {
                Ok(Some(kept)) => read(path, &kept)?,
                Ok(None) => return Err(not_taken_back(path, &"the keychain no longer holds it")),
                Err(error) => return Err(not_taken_back(path, &error)),
            };
            *text = hex(&key);
            Ok(key)
        }
        (_, "") => {
            let key = fresh()?;
            *text = hex(&key);
            Ok(key)
        }
        (_, kept) => read(path, kept),
    }
}

fn first_in(path: &Path, keychain: &dyn Keychain, text: &mut String) -> Result<[u8; 32], String> {
    match keychain.get(&account(path)) {
        Ok(Some(kept)) => {
            let key = read(path, &kept)?;
            IN_KEYCHAIN.clone_into(text);
            Ok(key)
        }
        Ok(None) => {
            let key = fresh()?;
            *text = if stored(path, keychain, &key) { IN_KEYCHAIN.to_owned() } else { hex(&key) };
            Ok(key)
        }
        Err(error) => {
            tracing::warn!(%error, "no keychain can be reached; the secrets key is kept in a file");
            let key = fresh()?;
            *text = hex(&key);
            Ok(key)
        }
    }
}

fn stored(path: &Path, keychain: &dyn Keychain, key: &[u8; 32]) -> bool {
    let (account, written) = (account(path), hex(key));
    let read_back = keychain.set(&account, &written).and_then(|()| keychain.get(&account));
    match read_back {
        Ok(Some(found)) if found == written => true,
        Ok(_) => {
            tracing::warn!("the keychain did not give the secrets key back; it stays in its file");
            false
        }
        Err(error) => {
            tracing::warn!(%error, "the keychain did not keep the secrets key; it stays in its file");
            false
        }
    }
}

fn from_keychain(path: &Path, keychain: &dyn Keychain) -> Result<[u8; 32], String> {
    let fallback =
        format!("to keep keys in a file instead, set {CHOICE}=file (keys saved before must then be entered again)");
    match keychain.get(&account(path)) {
        Ok(Some(kept)) => read(path, &kept),
        Ok(None) => Err(format!(
            "the secrets key is not in the system keychain, where {} says it is; saved keys cannot be opened; {fallback}",
            path.display()
        )),
        Err(error) => Err(format!(
            "the system keychain, which holds the secrets key, cannot be reached ({error}); \
             saved keys stay sealed until it can; {fallback}"
        )),
    }
}

fn account(path: &Path) -> String {
    path.display().to_string()
}

fn fresh() -> Result<[u8; 32], String> {
    let mut key = [0u8; 32];
    getrandom::fill(&mut key).map_err(|error| error.to_string())?;
    Ok(key)
}

fn read(path: &Path, text: &str) -> Result<[u8; 32], String> {
    unhex(text.trim())
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or_else(|| format!("the secrets key {} is not 64 hex digits", path.display()))
}

fn not_taken_back(path: &Path, why: &dyn std::fmt::Display) -> String {
    format!(
        "the secrets key is in the system keychain, as {} says, and could not be taken back ({why}); \
         saved keys stay sealed and nothing was replaced; allow the keychain and start again, \
         or set {CHOICE}={IN_KEYCHAIN}; to start over without the saved keys, remove that file and secrets.json",
        path.display()
    )
}

fn unusable(error: &dyn std::fmt::Display) -> String {
    format!("the secrets key cannot be used: {error}")
}

#[cfg(test)]
#[path = "key_tests.rs"]
mod key_tests;
