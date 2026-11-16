use super::choices::Picker;
use super::diff_view::DiffView;
use super::login::{PortBusy, SecretPrompt};
use super::search::Search;
use super::side::Side;

#[derive(Default)]
pub struct Overlays {
    pub picker: Option<Picker>,
    pub search: Option<Search>,
    pub side: Option<Side>,
    pub diffs: Option<DiffView>,
    pub secret: Option<SecretPrompt>,
    pub port_busy: Option<PortBusy>,
}
