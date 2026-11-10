//! `superfik login` and `logout`: pick a provider on the terminal; an API
//! key is pasted, a subscription signs in in the browser.

use std::io::{BufRead, Write};

use superfik_sdk::{ModelProvider, Superfik};

/// The providers in the order the menu lists them.
pub const MENU: [ModelProvider; 4] = [
    ModelProvider::Anthropic,
    ModelProvider::ClaudeSubscription,
    ModelProvider::OpenAi,
    ModelProvider::ChatGptSubscription,
];

pub fn how(provider: ModelProvider) -> &'static str {
    if provider.subscription() { "sign in with the browser" } else { "paste an API key" }
}

/// The menu entry `answer` picks among `count` (`1`-based).
pub fn picked(answer: &str, count: usize) -> Option<usize> {
    answer.trim().parse::<usize>().ok().filter(|n| (1..=count).contains(n)).map(|n| n - 1)
}

pub fn login(superfik: &Superfik) -> Result<(), String> {
    eprintln!("Connect a provider:");
    for (i, provider) in MENU.iter().enumerate() {
        eprintln!("  {}. {} ({})", i + 1, provider.name(), how(*provider));
    }
    let provider = MENU[picked(&ask("Choose 1-4: ")?, MENU.len()).ok_or("not one of the choices")?];
    let listed = if provider.subscription() {
        superfik
            .sign_in(provider, |url| {
                eprintln!("Opening the browser to sign in. If it does not open, visit:\n  {url}");
                open(url);
            })
            .map_err(|e| e.to_string())?
    } else {
        let key = ask(&format!("{} API key: ", provider.name()))?;
        superfik.connect_provider(provider, &key).map_err(|e| e.to_string())?
    };
    let account = listed.iter().find(|p| p.provider == provider).and_then(|p| p.account.as_ref());
    match account {
        Some(account) => eprintln!("Connected {} ({}).", provider.name(), account.label),
        None => return Err(format!("{} did not connect", provider.name())),
    }
    Ok(())
}

pub fn logout(superfik: &Superfik) -> Result<(), String> {
    let connected: Vec<ModelProvider> = superfik
        .providers()
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter(|p| p.account.is_some())
        .map(|p| p.provider)
        .collect();
    if connected.is_empty() {
        eprintln!("No provider is connected.");
        return Ok(());
    }
    for (i, provider) in connected.iter().enumerate() {
        eprintln!("  {}. {}", i + 1, provider.name());
    }
    let index = picked(&ask("Forget which? ")?, connected.len()).ok_or("not one of the choices")?;
    superfik.disconnect_provider(connected[index]).map_err(|e| e.to_string())?;
    eprintln!("Forgot {}.", connected[index].name());
    Ok(())
}

fn ask(question: &str) -> Result<String, String> {
    eprint!("{question}");
    let _ = std::io::stderr().flush();
    let mut line = String::new();
    std::io::stdin().lock().read_line(&mut line).map_err(|e| e.to_string())?;
    Ok(line.trim().to_owned())
}

fn open(url: &str) {
    let program = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(windows) {
        "explorer"
    } else {
        "xdg-open"
    };
    let _ = std::process::Command::new(program).arg(url).spawn();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_choice_is_a_number_in_range() {
        assert_eq!(picked(" 2 ", 4), Some(1));
        assert_eq!(picked("0", 4), None);
        assert_eq!(picked("5", 4), None);
        assert_eq!(picked("two", 4), None);
        assert!(MENU.iter().filter(|p| p.subscription()).count() == 2);
    }
}
