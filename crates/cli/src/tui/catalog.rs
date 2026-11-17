use app::Model;
use domain::provider::ProviderSummary;

#[derive(Default)]
pub struct Catalog {
    pub models: Vec<Model>,
    pub skills: Vec<(String, String)>,
    pub connected: Vec<ProviderSummary>,
    pub menu: Vec<ProviderSummary>,
    pub accounts: Vec<super::accounts::Listed>,
    pub files: Option<Vec<String>>,
    pub conversations: Vec<super::conversations::Conversation>,
    pub loading: bool,
}
