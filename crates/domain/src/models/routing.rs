#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Route<'a> {
    pub provider: Option<&'a str>,
    pub model: &'a str,
}

pub fn route<'a>(wanted: &'a str, providers: &[&str]) -> Route<'a> {
    let wanted = wanted.trim();
    match wanted.split_once(':') {
        Some((provider, model)) if !model.is_empty() && providers.contains(&provider) => {
            Route { provider: Some(provider), model }
        }
        _ => Route { provider: None, model: wanted },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KNOWN: [&str; 3] = ["zenith", "acme", "orbit"];

    #[test]
    fn a_known_provider_prefix_names_the_provider() {
        assert_eq!(route("acme:gpt-6.1-sol", &KNOWN), Route { provider: Some("acme"), model: "gpt-6.1-sol" });
        assert_eq!(
            route(" zenith:claude-haiku-5-5 ", &KNOWN),
            Route { provider: Some("zenith"), model: "claude-haiku-5-5" }
        );
    }

    #[test]
    fn anything_else_is_a_model_of_the_current_provider() {
        assert_eq!(route("gpt-6.1-sol", &KNOWN), Route { provider: None, model: "gpt-6.1-sol" });
        let catalog_id = "orbit-sub:claude-haiku-4.5";
        assert_eq!(route(catalog_id, &KNOWN), Route { provider: None, model: catalog_id });
        assert_eq!(route("acme:", &KNOWN), Route { provider: None, model: "acme:" });
    }
}
