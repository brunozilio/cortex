use std::io::Read as _;

use domain::dependencies::{Found, Package, concern, day_of, requested};

use crate::model::http::Http;

pub fn concerns(http: &dyn Http, command: &str, today: i64) -> Vec<String> {
    requested(command).iter().filter_map(|package| concern(package, &looked_up(http, package), today)).collect()
}

pub fn today() -> i64 {
    let seconds = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |since| since.as_secs());
    i64::try_from(seconds / 86_400).unwrap_or(0)
}

fn looked_up(http: &dyn Http, package: &Package) -> Found {
    let headers =
        [("Accept".to_owned(), "application/json".to_owned()), ("User-Agent".to_owned(), "coding-agent".to_owned())];
    let Ok(mut answer) = http.get(&package.ecosystem.lookup_url(&package.name), &headers) else {
        return Found::Unknown;
    };
    match answer.status {
        404 => Found::Missing,
        200 => {
            let mut text = String::new();
            if answer.body.read_to_string(&mut text).is_err() {
                return Found::Unknown;
            }
            let parsed: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
            Found::Published { first_day: package.ecosystem.first_published(&parsed).and_then(day_of) }
        }
        _ => Found::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::http::ScriptedHttp;

    #[test]
    fn each_installed_package_is_looked_up_and_only_concerns_are_kept() {
        let today = day_of("2026-11-13").expect("day");
        let http = ScriptedHttp::new([
            (200, r#"{"time":{"created":"2010-12-29T00:00:00Z"}}"#.to_owned()),
            (404, "{}".to_owned()),
            (200, r#"{"time":{"created":"2026-11-10T00:00:00Z"}}"#.to_owned()),
            (500, String::new()),
        ]);

        let found = concerns(&http, "npm install express made-up-pkg fresh-pkg && npm i flaky", today);

        assert_eq!(
            found,
            [
                "made-up-pkg is not published in its registry: the name may be made up".to_owned(),
                "fresh-pkg was first published 3 days ago".to_owned(),
            ]
        );
        let urls: Vec<String> = http.sent().into_iter().map(|sent| sent.url).collect();
        assert_eq!(urls[0], "https://registry.npmjs.org/express");
        assert_eq!(urls.len(), 4);
    }

    #[test]
    fn an_ordinary_command_looks_nothing_up() {
        let http = ScriptedHttp::new([]);
        assert!(concerns(&http, "cargo build && npm test", today()).is_empty());
        assert!(http.sent().is_empty());
    }
}
