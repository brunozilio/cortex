use domain::ModelProvider;
use domain::provider::ProviderInfo;
use llm_providers::{Provider, ProviderError, declared};

include!(concat!(env!("OUT_DIR"), "/providers.rs"));

pub fn all() -> Vec<&'static dyn Provider> {
    let mut all = ALL.to_vec();
    all.extend(declared::registered());
    all.sort_by_key(|provider| provider.rank());
    all
}

pub fn find(provider: ModelProvider) -> Option<&'static dyn Provider> {
    ALL.iter()
        .copied()
        .find(|known| known.id() == provider.id())
        .or_else(|| declared::find(provider))
}

pub fn get(provider: ModelProvider) -> Result<&'static dyn Provider, ProviderError> {
    find(provider).ok_or_else(|| ProviderError::Invalid(format!("no provider is called {provider}")))
}

pub fn id(provider: &dyn Provider) -> ModelProvider {
    ModelProvider::new(provider.id())
}

pub fn infos() -> Vec<ProviderInfo> {
    all()
        .into_iter()
        .map(|provider| ProviderInfo {
            provider: id(provider),
            name: provider.name().to_owned(),
            access: provider.access(),
        })
        .collect()
}

pub fn name(provider: ModelProvider) -> String {
    find(provider).map_or_else(|| provider.id().to_owned(), |known| known.name().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_built_in_provider_has_its_own_id_rank_and_secret_slot() {
        let mut all = ALL.to_vec();
        all.sort_by_key(|provider| provider.rank());
        let mut ids: Vec<_> = all.iter().map(|p| p.id()).collect();
        let mut slots: Vec<_> = all.iter().map(|p| p.slot()).collect();
        ids.dedup();
        slots.sort_unstable();
        slots.dedup();
        assert_eq!((ids.len(), slots.len()), (all.len(), all.len()));
        assert!(all.windows(2).all(|pair| pair[0].rank() < pair[1].rank()));
    }

    #[test]
    fn every_provider_speaks_a_wire_the_engine_has() {
        for provider in ALL {
            let http: std::sync::Arc<dyn crate::model::http::Http> =
                std::sync::Arc::new(crate::model::http::ScriptedHttp::new([]));
            assert!(crate::model::wires::open(provider.wire(None), http, None).is_some(), "{}", provider.id());
        }
    }

    #[test]
    fn an_unknown_provider_is_named_by_its_id_and_cannot_be_used() {
        let unknown = ModelProvider::new("nobody");
        assert_eq!(name(unknown), "nobody");
        assert!(get(unknown).is_err());
    }
}
