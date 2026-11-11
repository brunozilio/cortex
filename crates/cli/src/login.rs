use std::io::{BufRead, Write};

use domain::Access;
use domain::provider::ProviderSummary;
use sdk::Sdk;

pub fn how(access: Access) -> &'static str {
    match access {
        Access::Code => "sign in with a code",
        Access::Browser => "sign in with the browser",
        Access::Key => "paste an API key",
    }
}

pub fn visit(url: &str, code: Option<&str>) -> String {
    match code {
        Some(code) => format!("Enter the code {code} at {url} (opening the browser)."),
        None => format!("Opening the browser to sign in. If it does not open, visit:\n  {url}"),
    }
}

pub fn picked(answer: &str, count: usize) -> Option<usize> {
    answer.trim().parse::<usize>().ok().filter(|n| (1..=count).contains(n)).map(|n| n - 1)
}

pub fn login(sdk: &Sdk) -> Result<(), String> {
    let menu = sdk.providers().map_err(|e| e.to_string())?;
    eprintln!("Connect a provider:");
    for (i, summary) in menu.iter().enumerate() {
        eprintln!("  {}. {} ({})", i + 1, summary.name, how(summary.access));
    }
    let choice = picked(&ask(&format!("Choose 1-{}: ", menu.len()))?, menu.len()).ok_or("not one of the choices")?;
    let ProviderSummary { provider, name, access, .. } = menu[choice].clone();
    let listed = if access.subscription() {
        sdk.sign_in(provider, |url, code| {
            eprintln!("{}", visit(url, code));
            open(url);
        })
        .map_err(|e| e.to_string())?
    } else {
        let key = secret(&format!("{name} API key (hidden): "))?;
        sdk.connect_provider(provider, &key).map_err(|e| e.to_string())?
    };
    let account = listed.iter().find(|p| p.provider == provider).and_then(|p| p.account.as_ref());
    match account {
        Some(account) => eprintln!("Connected {name} ({}).", account.label),
        None => return Err(format!("{name} did not connect")),
    }
    Ok(())
}

pub fn logout(sdk: &Sdk) -> Result<(), String> {
    let connected: Vec<ProviderSummary> =
        sdk.providers().map_err(|e| e.to_string())?.into_iter().filter(|p| p.account.is_some()).collect();
    if connected.is_empty() {
        eprintln!("No provider is connected.");
        return Ok(());
    }
    for (i, summary) in connected.iter().enumerate() {
        eprintln!("  {}. {}", i + 1, summary.name);
    }
    let index = picked(&ask("Forget which? ")?, connected.len()).ok_or("not one of the choices")?;
    sdk.disconnect_provider(connected[index].provider).map_err(|e| e.to_string())?;
    eprintln!("Forgot {}.", connected[index].name);
    Ok(())
}

fn ask(question: &str) -> Result<String, String> {
    eprint!("{question}");
    let _ = std::io::stderr().flush();
    let mut line = String::new();
    std::io::stdin().lock().read_line(&mut line).map_err(|e| e.to_string())?;
    Ok(line.trim().to_owned())
}

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

pub fn open(url: &str) {
    if let Ok(mut child) = browser(url).spawn() {
        std::thread::spawn(move || child.wait());
    }
}

fn browser(url: &str) -> std::process::Command {
    quiet(platform::open_url(url))
}

fn quiet(mut command: std::process::Command) -> std::process::Command {
    command.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    command
}

#[cfg(test)]
fn launcher(program: &str, url: &str) -> std::process::Command {
    let mut command = std::process::Command::new(program);
    command.arg(url);
    quiet(command)
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
        assert_eq!(how(Access::Key), "paste an API key");
    }

    #[cfg(unix)]
    #[test]
    fn the_browser_never_writes_on_the_terminal() {
        use std::os::unix::fs::PermissionsExt;
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
