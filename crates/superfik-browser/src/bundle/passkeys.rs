//! The keychain group Chromium keeps Touch ID passkeys in.
//!
//! Chromium builds it at compile time from its team and bundle id; the CEF
//! distribution has no team, so it asks for `.org.chromium.Chromium.webauthn`,
//! a group no signature can hold (a provisioning profile only grants groups
//! under its team, `F3BSQ3FZTF.*`) and Touch ID stays off. The framework
//! copied into a bundle has that constant replaced by [`ACCESS_GROUP`], the
//! same length, which the app's entitlements then grant.

use std::fs;
use std::io;
use std::path::Path;

const STOCK: &[u8] = b".org.chromium.Chromium.webauthn";
pub const ACCESS_GROUP: &str = "F3BSQ3FZTF.superfik.webauthn.v1";
const KEYS_SUFFIX: &[u8] = b"-uvk";

const _: () = assert!(ACCESS_GROUP.len() == STOCK.len());

pub fn groups() -> [String; 2] {
    [ACCESS_GROUP.to_owned(), format!("{ACCESS_GROUP}{}", String::from_utf8_lossy(KEYS_SUFFIX))]
}

pub fn patch(binary: &mut [u8]) -> usize {
    let mut replaced = 0;
    let mut at = 0;
    while let Some(found) = find(&binary[at..], STOCK) {
        let start = at + found;
        let end = start + STOCK.len();
        let starts_string = start == 0 || binary[start - 1] == 0;
        let rest = &binary[end..];
        let ends_string =
            rest.first() == Some(&0) || (rest.starts_with(KEYS_SUFFIX) && rest.get(KEYS_SUFFIX.len()) == Some(&0));
        if starts_string && ends_string {
            binary[start..end].copy_from_slice(ACCESS_GROUP.as_bytes());
            replaced += 1;
        }
        at = end;
    }
    replaced
}

pub fn patch_framework(framework: &Path) -> io::Result<()> {
    let binary = framework.join("Chromium Embedded Framework");
    if !binary.exists() {
        return Ok(());
    }
    let mut bytes = fs::read(&binary)?;
    if patch(&mut bytes) > 0 {
        fs::write(&binary, bytes)?;
    }
    Ok(())
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stock_group_and_its_key_group_take_our_team() {
        let mut binary = b"\0x\0.org.chromium.Chromium.webauthn\0\x82\0.org.chromium.Chromium.webauthn-uvk\0".to_vec();
        assert_eq!(patch(&mut binary), 2);
        assert_eq!(
            binary,
            b"\0x\0F3BSQ3FZTF.superfik.webauthn.v1\0\x82\0F3BSQ3FZTF.superfik.webauthn.v1-uvk\0".to_vec()
        );
    }

    #[test]
    fn other_chromium_groups_and_longer_strings_are_left_alone() {
        let original = b"\0.org.chromium.Chromium.webauthn.extra\0a.org.chromium.Chromium.webauthn\0\
                         .org.chromium.Chromium.devicetrust\0"
            .to_vec();
        let mut binary = original.clone();
        assert_eq!(patch(&mut binary), 0);
        assert_eq!(binary, original);
    }

    #[test]
    fn the_entitlements_name_both_groups() {
        assert_eq!(groups(), ["F3BSQ3FZTF.superfik.webauthn.v1", "F3BSQ3FZTF.superfik.webauthn.v1-uvk"]);
    }
}
