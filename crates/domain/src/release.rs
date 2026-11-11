#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version(pub u64, pub u64, pub u64);

impl Version {
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        let mut parts = text.strip_prefix('v').unwrap_or(text).split('.').map(str::parse::<u64>);
        let version = Self(parts.next()?.ok()?, parts.next()?.ok()?, parts.next()?.ok()?);
        parts.next().is_none().then_some(version)
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.0, self.1, self.2)
    }
}

pub fn newer(current: &str, latest_tag: &str) -> Option<Version> {
    let latest = Version::parse(latest_tag)?;
    (Version::parse(current).is_none_or(|current| latest > current)).then_some(latest)
}

pub fn asset(version: Version, target: &str) -> String {
    format!("{}-{version}-{target}.tar.gz", brand::COMMAND)
}

pub fn checksum(text: &str, asset: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let (hex, name) = line.trim().split_once(char::is_whitespace)?;
        let name = name.trim_start().trim_start_matches('*');
        let valid = hex.len() == 64 && hex.chars().all(|c| c.is_ascii_hexdigit());
        (valid && name == asset).then(|| hex.to_ascii_lowercase())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_published_platform_has_its_archive() {
        let v = Version(0, 3, 1);
        let name = asset(v, "aarch64-apple-darwin").replacen(brand::COMMAND, "app", 1);
        assert_eq!(name, "app-0.3.1-aarch64-apple-darwin.tar.gz");
    }

    #[test]
    fn the_checksum_file_names_the_digest_of_its_archive() {
        let hex = "AB".repeat(32);
        let text =
            format!("{hex}  {brand_command}-0.3.1-aarch64-apple-darwin.tar.gz\n", brand_command = brand::COMMAND);
        assert_eq!(
            checksum(&text, concat!(brand::command!(), "-0.3.1-aarch64-apple-darwin.tar.gz")),
            Some("ab".repeat(32))
        );
        assert_eq!(
            checksum(&text, concat!(brand::command!(), "-0.3.1-x86_64-unknown-linux-gnu.tar.gz")),
            None,
            "another archive"
        );
        assert_eq!(
            checksum(concat!("nonsense  ", brand::command!(), ".tar.gz"), concat!(brand::command!(), ".tar.gz")),
            None
        );
    }

    #[test]
    fn a_newer_tag_is_the_update() {
        assert_eq!(newer("0.3.1", "v0.4.0"), Some(Version(0, 4, 0)));
        assert_eq!(newer("0.3.9", "v0.3.10"), Some(Version(0, 3, 10)), "numbers, not text");
    }

    #[test]
    fn the_same_or_an_older_tag_is_no_update() {
        assert_eq!(newer("0.3.1", "v0.3.1"), None);
        assert_eq!(newer("0.4.0", "v0.3.9"), None);
        assert_eq!(newer("0.3.1", "nightly"), None);
    }
}
