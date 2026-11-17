pub const LEGACY_OWNER_TAG: &str = "LocalOwner";
pub const OWNER_DIR: &str = "local";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_names_data_was_first_kept_under_never_change() {
        assert_eq!(serde_json::to_string(LEGACY_OWNER_TAG).ok().as_deref(), Some("\"LocalOwner\""));
        assert_eq!(OWNER_DIR, "local");
    }
}
