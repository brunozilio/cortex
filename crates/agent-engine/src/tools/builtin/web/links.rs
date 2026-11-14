use url::Url;

const LONGEST: usize = 200;

#[derive(Debug, Default)]
pub struct Links {
    page: Option<Url>,
    shown: Vec<String>,
}

impl Links {
    pub fn new(page: &str) -> Self {
        Self { page: Url::parse(page).ok(), shown: Vec::new() }
    }

    pub fn number(&mut self, href: &str) -> Option<usize> {
        let shown = self.shown_form(href)?;
        let at = match self.shown.iter().position(|known| *known == shown) {
            Some(at) => at,
            None => {
                self.shown.push(shown);
                self.shown.len() - 1
            }
        };
        Some(at + 1)
    }

    pub fn list(&self) -> String {
        if self.shown.is_empty() {
            return String::new();
        }
        let origin = self.page.as_ref().map(|page| page.origin().ascii_serialization());
        let header = match origin {
            Some(origin) if self.shown.iter().any(|shown| shown.starts_with('/')) => {
                format!("Links (paths are on {origin}):")
            }
            _ => "Links:".to_owned(),
        };
        let rows: Vec<String> =
            self.shown.iter().enumerate().map(|(at, shown)| format!("[{}] {shown}", at + 1)).collect();
        format!("{header}\n{}", rows.join("\n"))
    }

    pub fn absolute(&self, href: &str) -> Option<String> {
        self.resolved(href).map(String::from)
    }

    fn resolved(&self, href: &str) -> Option<Url> {
        let href = href.trim();
        let lower = href.to_ascii_lowercase();
        if href.is_empty() || href.starts_with('#') {
            return None;
        }
        if ["javascript:", "mailto:", "tel:", "data:"].iter().any(|scheme| lower.starts_with(scheme)) {
            return None;
        }
        let target = match &self.page {
            Some(page) => page.join(href).ok()?,
            None => Url::parse(href).ok()?,
        };
        matches!(target.scheme(), "http" | "https").then_some(target)
    }

    fn shown_form(&self, href: &str) -> Option<String> {
        let mut target = self.resolved(href)?;
        target.set_fragment(None);
        let kept: Vec<(String, String)> = target
            .query_pairs()
            .filter(|(key, _)| !key.starts_with("utm_"))
            .map(|(k, v)| (k.into(), v.into()))
            .collect();
        if kept.is_empty() {
            target.set_query(None);
        } else {
            target.query_pairs_mut().clear().extend_pairs(kept);
        }
        let same_site = self.page.as_ref().is_some_and(|page| page.origin() == target.origin());
        let shown = if same_site { target[url::Position::BeforePath..].to_owned() } else { target.to_string() };
        (shown.len() <= LONGEST).then_some(shown)
    }
}
