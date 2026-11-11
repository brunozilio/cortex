//! `{command} login` and `logout`: pick a provider on the terminal; an API
//! key is pasted, a subscription signs in in the browser.

use std::io::{BufRead, Write};

use sdk::{ModelProvider, Sdk};

/// The providers in the order the menu lists them.
pub const MENU: [ModelProvider; 6] = [
    ModelProvider::Anthropic,
    ModelProvider::ClaudeSubscription,
    ModelProvider::OpenAi,
    ModelProvider::ChatGptSubscription,
    ModelProvider::CopilotSubscription,
    ModelProvider::KiroSubscription,
];

pub fn how(provider: ModelProvider) -> &'static str {
    match provider {
        ModelProvider::CopilotSubscription => "sign in on GitHub with a code",
        p if p.subscription() => "sign in with the browser",
        _ => "paste an API key",
    }
}

/// What to tell the user while the browser is open.
pub fn visit(url: &str, code: Option<&str>) -> String {
    match code {
        Some(code) => format!("Enter the code {code} at {url} (opening the browser)."),
        None => format!("Opening the browser to sign in. If it does not open, visit:\n  {url}"),
    }
}

/// The menu entry `answer` picks among `count` (`1`-based).
pub fn picked(answer: &str, count: usize) -> Option<usize> {
    answer.trim().parse::<usize>().ok().filter(|n| (1..=count).contains(n)).map(|n| n - 1)
}

pub fn login(sdk: &Sdk) -> Result<(), String> {
    eprintln!("Connect a provider:");
    for (i, provider) in MENU.iter().enumerate() {
        eprintln!("  {}. {} ({})", i + 1, provider.name(), how(*provider));
    }
    let provider =
        MENU[picked(&ask(&format!("Choose 1-{}: ", MENU.len()))?, MENU.len()).ok_or("not one of the choices")?];
    let listed = if provider.subscription() {
        sdk.sign_in(provider, |url, code| {
            eprintln!("{}", visit(url, code));
            open(url);
        })
        .map_err(|e| e.to_string())?
    } else {
        let key = secret(&format!("{} API key (hidden): ", provider.name()))?;
        sdk.connect_provider(provider, &key).map_err(|e| e.to_string())?
    };
    let account = listed.iter().find(|p| p.provider == provider).and_then(|p| p.account.as_ref());
    match account {
        Some(account) => eprintln!("Connected {} ({}).", provider.name(), account.label),
        None => return Err(format!("{} did not connect", provider.name())),
    }
    Ok(())
}

pub fn logout(sdk: &Sdk) -> Result<(), String> {
    let connected: Vec<ModelProvider> = sdk
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
    sdk.disconnect_provider(connected[index]).map_err(|e| e.to_string())?;
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

/// Reads a line without echoing it (an API key).
fn secret(question: &str) -> Result<String, String> {
    use ratatui::crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers, read};
    use ratatui::crossterm::terminal::{disable_raw_mode, enable_raw_mode};
    eprint!("{question}");
    let _ = std::io::stderr().flush();
    enable_raw_mode().map_err(|e| e.to_string())?;
    let mut text = String::new();
    let result = loop {
        match read() {
            Ok(Event::Key(key)) if key.kind != KeyEventKind::Release => match key.code {
                KeyCode::Enter => break Ok(()),
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break Err("cancelled".into()),
                KeyCode::Char(c) => {
                    text.push(c);
                    eprint!("•");
                }
                KeyCode::Backspace if text.pop().is_some() => eprint!("\u{8} \u{8}"),
                KeyCode::Esc => break Err("cancelled".into()),
                _ => {}
            },
            Ok(Event::Paste(pasted)) => {
                eprint!("{}", "•".repeat(pasted.trim().chars().count()));
                text.push_str(pasted.trim());
            }
            Ok(_) => {}
            Err(error) => break Err(error.to_string()),
        }
        let _ = std::io::stderr().flush();
    };
    let _ = disable_raw_mode();
    eprintln!();
    result.map(|()| text.trim().to_owned())
}

/// Opens `url` in the browser. The browser never writes on the terminal
/// (Chrome's warnings would land on the chat's screen); the address is
/// shown to visit by hand when it does not open.
pub fn open(url: &str) {
    if let Ok(mut child) = browser(url).spawn() {
        std::thread::spawn(move || child.wait());
    }
}

fn browser(url: &str) -> std::process::Command {
    let program = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(windows) {
        "explorer"
    } else {
        "xdg-open"
    };
    launcher(program, url)
}

fn launcher(program: &str, url: &str) -> std::process::Command {
    let mut command = std::process::Command::new(program);
    command
        .arg(url)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    command
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
        assert!(MENU.iter().filter(|p| p.subscription()).count() == 4);
    }

    #[cfg(unix)]
    #[test]
    fn the_browser_never_writes_on_the_terminal() {
        use std::os::unix::fs::PermissionsExt;
        // A stand-in browser that complains, as Chrome does.
        let dir = tempfile::tempdir().expect("dir");
        let program = dir.path().join("browser");
        std::fs::write(&program, "#!/bin/sh\necho 'Failed sending zygote boot message' >&2\necho \"$1\"\n")
            .expect("write");
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        let ran = launcher(program.to_str().expect("path"), "https://example.com").output().expect("run");
        assert!(ran.status.success());
        assert!(ran.stdout.is_empty() && ran.stderr.is_empty(), "nothing reaches the terminal");
    }

    #[test]
    fn a_device_sign_in_shows_the_code_to_type() {
        let told = visit("https://github.com/login/device", Some("AB12-CD34"));
        assert!(told.contains("AB12-CD34") && told.contains("https://github.com/login/device"));
        assert!(visit("https://x", None).contains("https://x"));
    }
}
