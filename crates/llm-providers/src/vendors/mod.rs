pub mod anthropic;
pub mod openai;
pub mod xai;

pub static ALL: &[&dyn crate::Vendor] = &[&anthropic::Vendor, &openai::Vendor, &xai::Vendor];

#[cfg(test)]
mod tests {
    use super::ALL;

    #[test]
    fn the_registered_vendors_are_these_in_this_order() {
        let ids: Vec<&str> = ALL.iter().map(|vendor| vendor.id()).collect();
        assert_eq!(ids, ["anthropic", "openai", "xai"]);
    }
}
