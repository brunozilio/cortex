#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Offered<'a> {
    pub id: &'a str,
    pub input_rate: Option<u64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SmallChoice {
    pub model: Option<String>,
    pub ignored: Option<String>,
}

fn bare(model: &str) -> &str {
    model.rsplit(':').next().unwrap_or(model)
}

pub fn choose(session: &str, configured: Option<&str>, declared: Option<&str>, catalog: &[Offered]) -> SmallChoice {
    let offers = |model: &str| catalog.is_empty() || catalog.iter().any(|offered| bare(offered.id) == bare(model));
    let mut choice = SmallChoice::default();
    if let Some(configured) = configured {
        if offers(configured) {
            choice.model = Some(configured.to_owned());
            return choice;
        }
        choice.ignored =
            Some(format!("smallModel {configured} is not offered by this session's provider, so it is not used"));
    }
    if let Some(declared) = declared.filter(|declared| offers(declared)) {
        choice.model = Some(declared.to_owned());
        return choice;
    }
    let session_rate = catalog.iter().find(|offered| bare(offered.id) == bare(session)).and_then(|o| o.input_rate);
    choice.model = catalog
        .iter()
        .filter(|offered| bare(offered.id) != bare(session))
        .filter_map(|offered| Some((offered.input_rate?, offered.id)))
        .filter(|(rate, _)| session_rate.is_none_or(|session| *rate < session))
        .min_by_key(|(rate, id)| (*rate, *id))
        .map(|(_, id)| id.to_owned());
    choice
}

#[cfg(test)]
mod tests {
    use super::*;

    fn offered(id: &str, input_rate: Option<u64>) -> Offered<'_> {
        Offered { id, input_rate }
    }

    #[test]
    fn the_user_s_choice_wins_when_the_provider_offers_it() {
        let catalog = [offered("gpt-6-luna", Some(1)), offered("gpt-6.1-sol", Some(20))];
        let chosen = choose("gpt-6.1-sol", Some("gpt-6-luna"), Some("other"), &catalog);
        assert_eq!(chosen, SmallChoice { model: Some("gpt-6-luna".into()), ignored: None });
    }

    #[test]
    fn a_choice_the_provider_does_not_offer_is_reported_and_replaced() {
        let catalog = [offered("gpt-6-luna", Some(1)), offered("gpt-6.1-sol", Some(20))];
        let chosen = choose("gpt-6.1-sol", Some("claude-haiku-5-5"), Some("gpt-6-luna"), &catalog);
        assert_eq!(chosen.model.as_deref(), Some("gpt-6-luna"));
        assert!(chosen.ignored.is_some_and(|why| why.contains("claude-haiku-5-5")));
    }

    #[test]
    fn without_a_declared_model_the_cheapest_offered_one_is_taken() {
        let catalog =
            [offered("big", Some(50)), offered("mid", Some(10)), offered("tiny", Some(2)), offered("x", None)];
        assert_eq!(choose("big", None, Some("missing"), &catalog).model.as_deref(), Some("tiny"));
        assert_eq!(choose("tiny", None, None, &catalog).model, None, "nothing is cheaper than the session's");
    }

    #[test]
    fn an_unknown_catalog_trusts_the_declared_model() {
        assert_eq!(choose("m", None, Some("small"), &[]).model.as_deref(), Some("small"));
        assert_eq!(choose("m", None, None, &[]).model, None);
    }

    #[test]
    fn provider_prefixes_do_not_hide_a_match() {
        let catalog = [offered("openai-subscription:gpt-6-luna", Some(1))];
        let chosen = choose("openai-subscription:gpt-6.1-sol", Some("gpt-6-luna"), None, &catalog);
        assert_eq!(chosen.model.as_deref(), Some("gpt-6-luna"));
    }
}
