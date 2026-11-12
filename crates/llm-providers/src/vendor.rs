use domain::ModelOption;
use domain::billing::TokenRates;
use serde_json::Value;

use super::ProviderError;

pub trait Vendor: Sync {
    fn id(&self) -> &'static str;
    fn owns(&self, model: &str) -> bool;
    fn rates(&self, model: &str, prompt_tokens: u64) -> Option<TokenRates>;

    fn alias(&self, _name: &str) -> Option<&'static str> {
        None
    }

    fn small(&self) -> Option<&'static str> {
        None
    }

    fn reasons(&self, _model: &str) -> bool {
        false
    }

    fn effort(
        &self,
        _body: &mut Value,
        _selected: Option<&ModelOption>,
        _effort: &str,
        _always: bool,
    ) -> Result<(), ProviderError> {
        Ok(())
    }
}

pub fn vendor(id: &str) -> Option<&'static dyn Vendor> {
    crate::vendors::ALL.iter().copied().find(|vendor| vendor.id() == id)
}

pub fn owner(model: &str) -> Option<&'static dyn Vendor> {
    let model = bare(model);
    crate::vendors::ALL.iter().copied().find(|vendor| vendor.owns(model))
}

pub fn rates(vendor_id: &str, model: &str, prompt_tokens: u64) -> Option<TokenRates> {
    let model = bare(model);
    vendor(vendor_id).filter(|vendor| vendor.owns(model)).and_then(|vendor| vendor.rates(model, prompt_tokens))
}

pub fn effort(
    vendor_id: &str,
    body: &mut Value,
    selected: Option<&ModelOption>,
    effort: &str,
    always: bool,
) -> Result<(), ProviderError> {
    vendor(vendor_id).map_or(Ok(()), |vendor| vendor.effort(body, selected, effort, always))
}

pub type Price = (&'static [&'static str], TokenRates);

pub const fn per(input: u64, output: u64, cache_read: u64, cache_write: u64) -> TokenRates {
    TokenRates { input, output, cache_read, cache_write }
}

pub fn rated(prices: &[Price], model: &str) -> Option<TokenRates> {
    prices.iter().find(|(names, _)| names.contains(&model)).map(|(_, rates)| *rates)
}

fn bare(model: &str) -> &str {
    model.rsplit(':').next().unwrap_or(model)
}
