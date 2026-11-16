use domain::provider::ProviderSummary;
use sdk::Model;

#[derive(Default)]
pub struct Catalog {
    pub models: Vec<Model>,
    pub skills: Vec<(String, String)>,
    pub connected: Vec<ProviderSummary>,
    pub menu: Vec<ProviderSummary>,
    pub accounts: Vec<super::accounts::Listed>,
    pub files: Option<Vec<String>>,
    pub loading: bool,
}
