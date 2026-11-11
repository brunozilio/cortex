use domain::billing::{ModelUsage, SessionUsage, cache_hit_rate};

pub fn report(cost: f64, usage: &SessionUsage) -> String {
    if usage.models.is_empty() {
        return "No model calls yet in this session.".into();
    }
    let priced = usage.models.iter().any(|model| model.estimated_cost_usd.is_some());
    let mut text = if priced {
        format!("This session cost ${cost:.4}.")
    } else {
        "No price is known for these models (a subscription bills by its plan).".into()
    };
    let width = usage.models.iter().map(|model| model.model.len()).max().unwrap_or(0);
    for model in &usage.models {
        text.push_str(&format!("\n  {:<width$}  {}", model.model, line(model)));
    }
    text
}

fn line(usage: &ModelUsage) -> String {
    let hit = cache_hit_rate(usage).map(|rate| format!(" ({:.0}% hit)", rate * 100.0)).unwrap_or_default();
    let cost = usage.estimated_cost_usd.map_or_else(|| "not priced".to_owned(), |cost| format!("${cost:.4}"));
    format!(
        "in {} · out {} · cache read {}{hit} · cache write {} · {cost}",
        tokens(usage.input),
        tokens(usage.output),
        tokens(usage.cache_read),
        tokens(usage.cache_write)
    )
}

fn tokens(count: u64) -> String {
    match count {
        0..1_000 => count.to_string(),
        1_000..1_000_000 => format!("{:.1}k", count as f64 / 1_000.0),
        _ => format!("{:.1}M", count as f64 / 1_000_000.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(model: &str, cost: Option<f64>) -> ModelUsage {
        ModelUsage {
            model: model.into(),
            input: 1_200,
            output: 300,
            cache_read: 10_800,
            cache_write: 0,
            estimated_cost_usd: cost,
            basis: String::new(),
            price_date: String::new(),
        }
    }

    #[test]
    fn each_model_shows_its_tokens_cache_hits_and_cost() {
        let mut usage = SessionUsage::default();
        usage.add(&call("claude-opus-5-5", Some(0.75)));
        usage.add(&call("local", None));
        let text = report(0.75, &usage);
        assert!(text.starts_with("This session cost $0.7500."), "{text}");
        assert!(
            text.contains("claude-opus-5-5  in 1.2k · out 300 · cache read 10.8k (90% hit) · cache write 0 · $0.7500")
        );
        assert!(text.contains("local            in 1.2k"), "{text}");
        assert!(text.contains("not priced"), "{text}");
    }

    #[test]
    fn an_unpriced_or_empty_session_says_so() {
        assert_eq!(report(0.0, &SessionUsage::default()), "No model calls yet in this session.");
        let mut usage = SessionUsage::default();
        usage.add(&call("kiro", None));
        assert!(report(0.0, &usage).starts_with("No price is known"));
    }
}
