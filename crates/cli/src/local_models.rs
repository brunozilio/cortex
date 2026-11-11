//! `{command} pull` / `{command} rm`: local MLX models, like ollama's.
//! Presentation only: the progress line and the messages; the download
//! and the folder are `sdk::LocalModels`'.

use std::io::{IsTerminal, Write};
use std::time::{Duration, Instant};

use sdk::{LocalModels, PullProgress, Pulled, Removed};

/// How often the progress line is redrawn.
const REDRAW: Duration = Duration::from_millis(100);

pub fn pull(model: &str) -> Result<(), String> {
    let models = LocalModels::system().map_err(|e| e.to_string())?;
    let terminal = std::io::stderr().is_terminal();
    let mut drawn: Option<(usize, Instant)> = None;
    let pulled = models.pull(model, |progress| {
        let new_file = drawn.is_none_or(|(index, _)| index != progress.index);
        let due = drawn.is_some_and(|(_, at)| at.elapsed() >= REDRAW);
        if terminal && (new_file || due) {
            eprint!("\r\x1b[K{}", progress_line(progress));
            let _ = std::io::stderr().flush();
        } else if !terminal && new_file {
            eprintln!("{}", file_line(progress));
        } else {
            return;
        }
        drawn = Some((progress.index, Instant::now()));
    });
    if terminal && drawn.is_some() {
        eprint!("\r\x1b[K");
    }
    println!("{}", pulled_message(&pulled.map_err(|e| e.to_string())?));
    Ok(())
}

pub fn remove(model: &str) -> Result<(), String> {
    let models = LocalModels::system().map_err(|e| e.to_string())?;
    println!("{}", removed_message(&models.remove(model).map_err(|e| e.to_string())?));
    Ok(())
}

/// The line redrawn while pulling, e.g.
/// `pulling mlx-community/Qwen3-8B-4bit  1.2 GB / 4.6 GB  26%  (5/9 files)`.
fn progress_line(progress: &PullProgress) -> String {
    let files = format!("({}/{} files)", progress.index + 1, progress.files);
    match progress.total.filter(|total| *total > 0) {
        Some(total) => format!(
            "pulling {}  {} / {}  {}%  {files}",
            progress.id,
            bytes(progress.done),
            bytes(total),
            progress.done.saturating_mul(100) / total
        ),
        None => format!("pulling {}  {}  {files}", progress.id, bytes(progress.done)),
    }
}

/// One line per file when stderr is not a terminal.
fn file_line(progress: &PullProgress) -> String {
    format!("pulling {} ({}/{})", progress.file, progress.index + 1, progress.files)
}

fn pulled_message(pulled: &Pulled) -> String {
    let what = if pulled.fresh { "pulled" } else { "already downloaded:" };
    format!(
        "{what} {} ({})\nuse it with: {brand_command} -m {}",
        pulled.id,
        bytes(pulled.bytes),
        pulled.id,
        brand_command = brand::COMMAND
    )
}

fn removed_message(removed: &Removed) -> String {
    format!("removed {} (freed {})", removed.id, bytes(removed.bytes))
}

/// Sizes as ollama and Finder show them: decimal units, one decimal.
fn bytes(count: u64) -> String {
    const UNITS: [&str; 4] = ["KB", "MB", "GB", "TB"];
    if count < 1000 {
        return format!("{count} B");
    }
    let mut size = count as f64 / 1000.0;
    let mut unit = 0;
    while size >= 1000.0 && unit + 1 < UNITS.len() {
        size /= 1000.0;
        unit += 1;
    }
    format!("{size:.1} {}", UNITS[unit])
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn progress(done: u64, total: Option<u64>) -> PullProgress {
        PullProgress {
            id: "mlx-community/Qwen3-8B-4bit".into(),
            file: "model-00001-of-00002.safetensors".into(),
            index: 4,
            files: 9,
            done,
            total,
        }
    }

    #[test]
    fn sizes_read_like_ollamas() {
        assert_eq!(bytes(0), "0 B");
        assert_eq!(bytes(999), "999 B");
        assert_eq!(bytes(1_500), "1.5 KB");
        assert_eq!(bytes(4_600_000_000), "4.6 GB");
        assert_eq!(bytes(2_000_000_000_000_000), "2000.0 TB");
    }

    #[test]
    fn the_progress_line_shows_bytes_percent_and_files() {
        assert_eq!(
            progress_line(&progress(1_200_000_000, Some(4_600_000_000))),
            "pulling mlx-community/Qwen3-8B-4bit  1.2 GB / 4.6 GB  26%  (5/9 files)"
        );
        assert_eq!(
            progress_line(&progress(1_200_000_000, None)),
            "pulling mlx-community/Qwen3-8B-4bit  1.2 GB  (5/9 files)"
        );
        assert_eq!(file_line(&progress(0, None)), "pulling model-00001-of-00002.safetensors (5/9)");
    }

    #[test]
    fn the_messages_say_what_happened_and_how_big_it_is() {
        let pulled = Pulled {
            id: "mlx-community/Qwen3-8B-4bit".into(),
            folder: PathBuf::from("/m"),
            bytes: 4_600_000_000,
            fresh: true,
        };
        assert_eq!(
            pulled_message(&pulled),
            concat!(
                "pulled mlx-community/Qwen3-8B-4bit (4.6 GB)\nuse it with: ",
                brand::command!(),
                " -m mlx-community/Qwen3-8B-4bit"
            )
        );
        assert!(pulled_message(&Pulled { fresh: false, ..pulled }).starts_with("already downloaded: "));
        let removed =
            Removed { id: "mlx-community/Qwen3-8B-4bit".into(), folder: PathBuf::from("/m"), bytes: 4_600_000_000 };
        assert_eq!(removed_message(&removed), "removed mlx-community/Qwen3-8B-4bit (freed 4.6 GB)");
    }
}
