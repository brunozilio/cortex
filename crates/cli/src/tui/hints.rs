use std::path::{MAIN_SEPARATOR, Path};
use std::time::Duration;

pub const REWIND_WINDOW: Duration = Duration::from_secs(1);
const CLEAR_SHOWN: Duration = Duration::from_secs(2);

pub fn home_relative(cwd: &Path, home: Option<&Path>) -> String {
    let home = home.filter(|home| !home.as_os_str().is_empty());
    match home.and_then(|home| cwd.strip_prefix(home).ok()) {
        Some(rest) if rest.as_os_str().is_empty() => "~".to_owned(),
        Some(rest) => format!("~{MAIN_SEPARATOR}{}", rest.display()),
        None => cwd.display().to_string(),
    }
}

pub fn second_esc(prompt_is_empty: bool, armed_for: Duration) -> Option<&'static str> {
    match prompt_is_empty {
        true if armed_for < REWIND_WINDOW => Some("Press Esc again to go back to an earlier prompt"),
        false if armed_for < CLEAR_SHOWN => Some("Press Esc again to clear the prompt"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_folders_inside_the_home_folder_are_shortened() {
        let home = Path::new("/Users/ana");
        assert_eq!(home_relative(Path::new("/Users/ana/code/app"), Some(home)), "~/code/app");
        assert_eq!(home_relative(home, Some(home)), "~");
        assert_eq!(home_relative(Path::new("/Users/ana2/x"), Some(home)), "/Users/ana2/x", "a sibling, not inside");
        assert_eq!(home_relative(Path::new("/srv/app"), Some(home)), "/srv/app");
        assert_eq!(home_relative(Path::new("/srv/app"), None), "/srv/app");
        assert_eq!(home_relative(Path::new("/srv/app"), Some(Path::new(""))), "/srv/app");
    }

    #[test]
    fn the_hint_says_what_the_second_esc_does() {
        let just_now = Duration::from_millis(100);
        assert_eq!(second_esc(true, just_now), Some("Press Esc again to go back to an earlier prompt"));
        assert_eq!(second_esc(false, just_now), Some("Press Esc again to clear the prompt"));
        assert_eq!(second_esc(true, Duration::from_millis(1500)), None, "too late to rewind: no promise");
        assert_eq!(second_esc(false, Duration::from_millis(1500)), Some("Press Esc again to clear the prompt"));
        assert_eq!(second_esc(false, Duration::from_secs(3)), None);
    }
}
