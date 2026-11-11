use std::path::Path;

use domain::SessionId;

pub fn run(sdk: &sdk::Sdk, cwd: &Path, args: Vec<String>) -> Result<String, String> {
    let last = || crate::tui::last_session(&sdk.settings().map_err(|e| e.to_string())?, cwd).ok_or_else(nothing);
    let session = match named(&args) {
        Some(id) => SessionId(id.to_owned()),
        None => last()?,
    };
    let path = super::export(sdk, &session, cwd)?;
    if !args.iter().any(|arg| arg == "--no-open") {
        crate::login::open(&path.display().to_string());
    }
    Ok(path.display().to_string())
}

fn nothing() -> String {
    format!("no conversation in this folder yet; name one: {} trace <session>", brand::COMMAND)
}

fn named(args: &[String]) -> Option<&str> {
    args.iter().map(String::as_str).find(|arg| !arg.starts_with('-'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_session_is_the_first_word_that_is_not_a_flag() {
        let args = |words: &[&str]| words.iter().map(|word| (*word).to_owned()).collect::<Vec<_>>();
        assert_eq!(named(&args(&["--no-open", "3f2a"])), Some("3f2a"));
        assert_eq!(named(&args(&["--no-open"])), None);
        assert_eq!(named(&args(&[])), None);
    }
}
