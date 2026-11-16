use std::collections::HashMap;

const SHOWN_ONCE_FROM: usize = 3;
const PLACES: usize = 20;

pub(super) fn grouped(text: &str) -> String {
    let blocks: Vec<&str> = text.split("\n\n").collect();
    let kinds: Vec<Option<String>> = blocks.iter().map(|block| kind(block)).collect();
    let mut places: HashMap<&str, Vec<&str>> = HashMap::new();
    for (block, kind) in blocks.iter().zip(&kinds) {
        if let (Some(kind), Some(place)) = (kind, place(block)) {
            places.entry(kind).or_default().push(place);
        }
    }
    let mut shown: Vec<&str> = Vec::new();
    let mut out: Vec<String> = Vec::new();
    for (block, kind) in blocks.iter().zip(&kinds) {
        let others = kind.as_deref().and_then(|kind| places.get(kind).map(|places| (kind, places)));
        match others {
            Some((kind, places)) if places.len() >= SHOWN_ONCE_FROM => {
                if !shown.contains(&kind) {
                    shown.push(kind);
                    out.push(format!("{block}\n{}", elsewhere(&places[1..])));
                }
            }
            _ => out.push((*block).to_owned()),
        }
    }
    out.join("\n\n")
}

fn kind(block: &str) -> Option<String> {
    let heading = block.lines().next().filter(|line| line.starts_with("warning: ") || line.starts_with("warning["))?;
    place(block)?;
    Some(heading.split('`').enumerate().map(|(at, part)| if at % 2 == 1 { "…" } else { part }).collect())
}

fn place(block: &str) -> Option<&str> {
    block.lines().find_map(|line| line.trim_start().strip_prefix("--> "))
}

fn elsewhere(places: &[&str]) -> String {
    let listed = places.iter().take(PLACES).copied().collect::<Vec<_>>().join(", ");
    let rest = places.len().saturating_sub(PLACES);
    let more = if rest > 0 { format!(" and {rest} more") } else { String::new() };
    format!("… [{} more of this warning: {listed}{more}]", places.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unused(name: &str, file: &str) -> String {
        format!("warning: unused variable: `{name}`\n --> {file}\n  |\n1 | let {name} = 1;\n  |     ^")
    }

    #[test]
    fn a_warning_repeated_three_times_is_shown_once_with_the_other_places() {
        let output = [
            unused("a", "src/a.rs:1:9"),
            unused("b", "src/b.rs:2:9"),
            "error[E0308]: mismatched types\n --> src/d.rs:4:1".to_owned(),
            unused("c", "src/c.rs:3:9"),
            "warning: `x` (lib) generated 3 warnings".to_owned(),
        ]
        .join("\n\n");
        let expected = [
            format!("{}\n… [2 more of this warning: src/b.rs:2:9, src/c.rs:3:9]", unused("a", "src/a.rs:1:9")),
            "error[E0308]: mismatched types\n --> src/d.rs:4:1".to_owned(),
            "warning: `x` (lib) generated 3 warnings".to_owned(),
        ]
        .join("\n\n");
        assert_eq!(grouped(&output), expected);
    }

    #[test]
    fn two_of_a_kind_different_warnings_and_errors_stay_whole() {
        let output = [
            unused("a", "src/a.rs:1:9"),
            unused("b", "src/b.rs:2:9"),
            "warning: unreachable pattern\n --> src/c.rs:3:9".to_owned(),
            "error: boom\n --> src/d.rs:1:1".to_owned(),
            "error: boom\n --> src/e.rs:1:1".to_owned(),
            "error: boom\n --> src/f.rs:1:1".to_owned(),
        ]
        .join("\n\n");
        assert_eq!(grouped(&output), output);
    }

    #[test]
    fn many_places_are_listed_up_to_a_limit_and_the_rest_counted() {
        let output: Vec<String> = (0..30).map(|at| unused("a", &format!("src/f{at}.rs:1:9"))).collect();
        let text = grouped(&output.join("\n\n"));
        assert!(text.contains("… [29 more of this warning: src/f1.rs:1:9, "), "{text}");
        assert!(text.ends_with("src/f20.rs:1:9 and 9 more]"), "{text}");
    }
}
