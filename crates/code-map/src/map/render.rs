use super::rank::{CALLER_FILES, SYMBOLS_PER_FILE};
use super::{Pointer, SymbolHit};

pub const MAX_FILES: usize = 12;

pub fn render(pointers: &[Pointer]) -> String {
    let lines: Vec<String> = pointers.iter().take(MAX_FILES).map(line).collect();
    if lines.is_empty() {
        return String::new();
    }
    format!(
        "<code-map>\nFiles and symbols that match this prompt (from a tree-sitter index; open them rather than searching):\n{}\n</code-map>",
        lines.join("\n")
    )
}

fn line(pointer: &Pointer) -> String {
    let symbols: Vec<String> = pointer.symbols.iter().take(SYMBOLS_PER_FILE).map(symbol).collect();
    if symbols.is_empty() {
        return format!("- {}", pointer.path);
    }
    format!("- {}: {}", pointer.path, symbols.join(", "))
}

fn symbol(hit: &SymbolHit) -> String {
    let callers = match hit.callers {
        0 => String::new(),
        count => {
            let places = if count == 1 { "place" } else { "places" };
            let mut files: Vec<&str> = hit.caller_files.iter().take(CALLER_FILES).map(String::as_str).collect();
            if hit.caller_files.len() > CALLER_FILES {
                files.push("...");
            }
            format!(", called from {count} {places}: {}", files.join(", "))
        }
    };
    format!("{} {} (L{}{callers})", hit.kind.label(), hit.name, hit.line)
}
