use domain::small_model::{Offered, SmallChoice, choose};
use domain::{ModelOption, ModelProvider};

fn bare(model: &str) -> &str {
    model.rsplit(':').next().unwrap_or(model)
}

pub fn pick(provider: ModelProvider, session: &str, configured: Option<&str>, catalog: &[ModelOption]) -> SmallChoice {
    let found = crate::providers::find(provider);
    let vendor = found.and_then(|provider| llm_providers::vendor(provider.vendor()));
    let configured = configured.map(|name| vendor.and_then(|vendor| vendor.alias(name)).unwrap_or(name));
    let declared = found.and_then(|provider| provider.small_model());
    let offered: Vec<Offered> = catalog
        .iter()
        .filter(|model| model.provider == provider)
        .map(|model| Offered { id: &model.id, input_rate: input_rate(found.map(|p| p.vendor()), &model.id) })
        .collect();
    choose(session, configured, declared, &offered)
}

fn input_rate(provider_vendor: Option<&str>, id: &str) -> Option<u64> {
    let model = bare(id);
    let dashed = model.replace('.', "-");
    [model, dashed.as_str()]
        .into_iter()
        .find_map(|name| {
            provider_vendor
                .and_then(|vendor| llm_providers::rates(vendor, name, 0))
                .or_else(|| llm_providers::owner(name).and_then(|owner| llm_providers::rates(owner.id(), name, 0)))
        })
        .map(|rates| rates.input)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn offered(provider: ModelProvider, id: &str) -> ModelOption {
        ModelOption::new(id, id).served_by(provider)
    }

    #[test]
    fn each_provider_brings_its_own_small_model() {
        let claude = ModelProvider::new("claude-subscription");
        assert_eq!(pick(claude, "claude-opus-5-5", None, &[]).model.as_deref(), Some("claude-haiku-5-5"));
        let chatgpt = ModelProvider::new("openai-subscription");
        assert_eq!(pick(chatgpt, "gpt-6.1-sol", None, &[]).model.as_deref(), Some("gpt-6-luna"));
    }

    #[test]
    fn a_subscription_without_the_declared_model_gets_its_cheapest_priced_one() {
        let chatgpt = ModelProvider::new("openai-subscription");
        let catalog = [offered(chatgpt, "gpt-6.1-sol"), offered(chatgpt, "gpt-6.1-terra")];
        assert_eq!(pick(chatgpt, "gpt-6.1-sol", None, &catalog).model.as_deref(), Some("gpt-6.1-terra"));
    }

    #[test]
    fn kiro_declares_haiku_under_its_own_name() {
        let kiro = ModelProvider::new("kiro-subscription");
        assert_eq!(pick(kiro, "kiro-subscription:auto", None, &[]).model.as_deref(), Some("claude-haiku-4.5"));
    }

    #[test]
    fn copilot_takes_the_cheapest_model_it_lists_whoever_makes_it() {
        let copilot = ModelProvider::new("copilot-subscription");
        let catalog = [
            offered(copilot, "copilot-subscription:claude-sonnet-4.5"),
            offered(copilot, "copilot-subscription:claude-haiku-4.5"),
            offered(copilot, "copilot-subscription:gpt-6.1-sol"),
            offered(copilot, "copilot-subscription:unpriced-model"),
        ];
        let chosen = pick(copilot, "copilot-subscription:claude-sonnet-4.5", None, &catalog);
        assert_eq!(chosen.model.as_deref(), Some("copilot-subscription:claude-haiku-4.5"));
    }

    #[test]
    fn an_alias_is_resolved_before_it_is_checked() {
        let claude = ModelProvider::new("claude-subscription");
        let catalog = [offered(claude, "claude-haiku-5-5"), offered(claude, "claude-opus-5-5")];
        let chosen = pick(claude, "claude-opus-5-5", Some("haiku"), &catalog);
        assert_eq!((chosen.model.as_deref(), chosen.ignored), (Some("claude-haiku-5-5"), None));
        let chatgpt = ModelProvider::new("openai-subscription");
        let elsewhere = pick(chatgpt, "gpt-6.1-sol", Some("haiku"), &[offered(chatgpt, "gpt-6-luna")]);
        assert!(elsewhere.ignored.is_some());
        assert_eq!(elsewhere.model.as_deref(), Some("gpt-6-luna"));
    }
}
