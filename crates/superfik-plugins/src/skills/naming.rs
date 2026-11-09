//! Display names and icons for skills. Pure and deterministic.

use superfik_core::SkillIcon;

const ICONS: [SkillIcon; 4] = [SkillIcon::Cube, SkillIcon::Bolt, SkillIcon::Sparkle, SkillIcon::Book];

pub fn skill_title(name: &str) -> String {
    let words = name.split(['-', '_', ' ']).filter(|word| !word.is_empty()).collect::<Vec<_>>().join(" ");
    let mut chars = words.chars();
    let human = match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect::<String>(),
        None => "Unnamed".to_owned(),
    };
    if human.to_ascii_lowercase().ends_with(" skill") { human } else { format!("{human} skill") }
}

pub fn skill_icon(name: &str) -> SkillIcon {
    let index = (fnv1a(name.as_bytes()) % ICONS.len() as u64) as usize;
    ICONS[index]
}

fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3))
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn titles_are_humanized() {
        assert_eq!(skill_title("dither-minis"), "Dither minis skill");
        assert_eq!(skill_title("motion"), "Motion skill");
        assert_eq!(skill_title("snake_case_name"), "Snake case name skill");
        assert_eq!(skill_title("pdf-skill"), "Pdf skill");
        assert_eq!(skill_title("--"), "Unnamed skill");
    }

    #[test]
    fn icons_are_deterministic_and_varied() {
        assert_eq!(skill_icon("motion"), skill_icon("motion"));
        let names = ["motion", "dither-minis", "tdd", "pdf", "docx", "skeuomorphic-ui", "a", "b", "c"];
        let icons: HashSet<SkillIcon> = names.iter().map(|name| skill_icon(name)).collect();
        assert!(icons.len() > 1);
    }

    #[test]
    fn fnv_matches_reference_vector() {
        assert_eq!(fnv1a(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a(b"a"), 0xaf63_dc4c_8601_ec8c);
    }
}
