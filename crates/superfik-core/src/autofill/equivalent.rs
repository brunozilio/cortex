//! Domains that sign in with the same account, as Bitwarden's
//! "equivalent domains": a login saved for one is offered on the others
//! under the default (Domain) rule. Our own short list of well-known
//! groups, by registrable domain. Pure.

const GROUPS: &[&[&str]] = &[
    &["google.com", "youtube.com", "gmail.com"],
    &["apple.com", "icloud.com"],
    &[
        "microsoft.com",
        "microsoftonline.com",
        "live.com",
        "outlook.com",
        "hotmail.com",
        "office.com",
        "bing.com",
        "msn.com",
        "xbox.com",
        "skype.com",
    ],
    &[
        "amazon.com",
        "amazon.com.br",
        "amazon.co.uk",
        "amazon.de",
        "amazon.fr",
        "amazon.it",
        "amazon.es",
        "amazon.ca",
        "amazon.com.mx",
        "amazon.co.jp",
        "amazon.in",
        "amazon.com.au",
        "amazon.nl",
    ],
    &["ebay.com", "ebay.co.uk", "ebay.de", "ebay.fr", "ebay.it", "ebay.es", "ebay.ca", "ebay.com.au"],
    &["mercadolivre.com.br", "mercadolibre.com", "mercadopago.com.br", "mercadopago.com"],
    &["yahoo.com", "yahoo.co.uk", "yahoo.com.br"],
    &["atlassian.com", "atlassian.net", "bitbucket.org", "trello.com"],
    &["facebook.com", "messenger.com"],
    &["twitter.com", "x.com"],
    &["paypal.com", "paypal.me"],
    &["steampowered.com", "steamcommunity.com"],
    &["playstation.com", "sonyentertainmentnetwork.com"],
    &["dropbox.com", "getdropbox.com"],
    &["proton.me", "protonmail.com"],
];

pub fn equivalent(a: &str, b: &str) -> bool {
    a == b || GROUPS.iter().any(|group| group.contains(&a) && group.contains(&b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_group_signs_in_together_and_nothing_else_does() {
        assert!(equivalent("google.com", "youtube.com"));
        assert!(equivalent("amazon.com", "amazon.com.br"));
        assert!(equivalent("github.com", "github.com"));
        assert!(!equivalent("google.com", "apple.com"));
        assert!(!equivalent("google.com", "evil.com"));
    }

    #[test]
    fn every_group_is_registrable_domains_in_lower_case() {
        for domain in GROUPS.iter().flat_map(|group| group.iter()) {
            assert_eq!(psl::domain_str(domain), Some(*domain), "{domain} is not a registrable domain");
        }
    }
}
