//! Reads the cookies of every Chromium-family browser found for this user,
//! as Settings › Browser's Import does, and prints each cookie's host and
//! name (never its value).
//!
//! `cargo run -p superfik-browser --example logins_probe`

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let home = std::env::home_dir().ok_or("no home folder")?;
    let sources = superfik_browser::import::sources(&home);
    if sources.is_empty() {
        println!("no Chromium-family browser found under {}", home.display());
    }
    for source in &sources {
        match superfik_browser::import::read_all(source) {
            Ok(cookies) => {
                println!("{}: {} cookies ({})", source.kind.name(), cookies.len(), source.dir.display());
                for cookie in cookies.iter().take(20) {
                    println!("  {} {} ({} bytes)", cookie.domain, cookie.name, cookie.value.len());
                }
            }
            Err(error) => println!("{}: {error}", source.kind.name()),
        }
    }
    Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn main() {
    eprintln!("logins_probe runs on macOS and Linux");
}
