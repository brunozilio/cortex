use super::*;
use crate::model::Access;
use crate::provider::ProviderInfo;

const ACME: ModelProvider = ModelProvider::new("acme");

fn account(label: &str, id: &str) -> ProviderAccount {
    ProviderAccount { label: label.into(), since_unix: 10, id: id.into() }
}

fn labels(book: &ProviderBook, provider: ModelProvider) -> Vec<&str> {
    book.accounts(provider).iter().map(|a| a.label.as_str()).collect()
}

fn active(book: &ProviderBook, provider: ModelProvider) -> Option<&str> {
    book.get(provider).map(|a| a.label.as_str())
}

#[test]
fn the_first_account_takes_the_provider_s_own_slot_and_others_a_fresh_one() {
    let mut book = ProviderBook::default();
    assert_eq!(book.slot_for(ACME, "a@x.io", || "fresh".into()), "");
    book.connect(ACME, account("a@x.io", ""));
    assert_eq!(book.slot_for(ACME, "b@x.io", || "fresh".into()), "fresh");
    assert_eq!(book.slot_for(ACME, "A@X.io", || "fresh".into()), "", "signing in again reuses the slot");
}

#[test]
fn a_new_account_is_added_and_becomes_the_active_one() {
    let mut book = ProviderBook::default();
    book.connect(ACME, account("a@x.io", ""));
    book.connect(ACME, account("b@x.io", "b"));
    assert_eq!(labels(&book, ACME), ["a@x.io", "b@x.io"]);
    assert_eq!(active(&book, ACME), Some("b@x.io"));
    book.connect(ACME, account("a@x.io", ""));
    assert_eq!(labels(&book, ACME), ["a@x.io", "b@x.io"], "the same account is replaced, not added");
    assert_eq!(active(&book, ACME), Some("a@x.io"));
}

#[test]
fn an_account_is_chosen_by_its_label_or_its_number() {
    let mut book = ProviderBook::default();
    book.connect(ACME, account("a@x.io", ""));
    book.connect(ACME, account("b@x.io", "b"));
    assert_eq!(book.use_account(ACME, "1").map(|a| a.label.clone()), Ok("a@x.io".into()));
    assert_eq!(active(&book, ACME), Some("a@x.io"));
    assert_eq!(book.use_account(ACME, "B@x.io").map(|a| a.id.clone()), Ok("b".into()));
    assert!(book.use_account(ACME, "3").is_err());
    assert!(book.use_account(ACME, "c@x.io").is_err());
}

#[test]
fn removing_the_active_account_makes_the_first_one_left_active() {
    let mut book = ProviderBook::default();
    book.connect(ACME, account("a@x.io", ""));
    book.connect(ACME, account("b@x.io", "b"));
    assert_eq!(book.remove(ACME, "2").map(|a| a.id), Ok("b".into()));
    assert_eq!(active(&book, ACME), Some("a@x.io"));
    assert_eq!(book.disconnect(ACME).len(), 1);
    assert_eq!(book.get(ACME), None);
}

#[test]
fn a_book_saved_with_one_account_per_provider_still_loads() {
    let old = r#"{"accounts":{"acme":{"label":"…ab12","since_unix":5}}}"#;
    let book: ProviderBook = serde_json::from_str(old).expect("old book");
    assert_eq!(active(&book, ACME), Some("…ab12"));
    assert_eq!(book.get(ACME).map(|a| a.id.as_str()), Some(""));
}

#[test]
fn a_book_round_trips() {
    let mut book = ProviderBook::default();
    book.connect(ACME, account("a", ""));
    book.connect(ACME, account("b", "b"));
    book.use_account(ACME, "a").expect("a");
    let back: ProviderBook = serde_json::from_str(&serde_json::to_string(&book).expect("json")).expect("parse");
    assert_eq!(back, book);
}

#[test]
fn summaries_list_every_provider_with_its_active_account_and_all_of_them() {
    let mut book = ProviderBook::default();
    let (one, two) = (ModelProvider::new("one"), ModelProvider::new("two"));
    book.connect(two, account("…ff00", ""));
    book.connect(two, account("…ee11", "e"));
    let info = |provider, name: &str| ProviderInfo { provider, name: name.into(), access: Access::Key };
    let summaries = book.summaries([info(one, "One"), info(two, "Two")]);
    assert_eq!(summaries.iter().map(|s| s.provider).collect::<Vec<_>>(), [one, two]);
    assert_eq!((summaries[0].account.as_ref(), summaries[0].accounts.len()), (None, 0));
    assert_eq!(summaries[1].account.as_ref().map(|a| a.label.as_str()), Some("…ee11"));
    assert_eq!(summaries[1].accounts.len(), 2);
}

#[test]
fn each_provider_keeps_its_own_balance_and_where_the_account_in_use_sits() {
    use crate::account_pool::Balance;
    let mut book = ProviderBook::default();
    book.connect(ACME, account("a", ""));
    book.connect(ACME, account("b", "b"));
    assert_eq!((book.balance(ACME), book.active_index(ACME)), (Balance::Off, 1));
    book.set_balance(ACME, Balance::RoundRobin);
    let back: ProviderBook = serde_json::from_str(&serde_json::to_string(&book).expect("json")).expect("parse");
    assert_eq!(back.balance(ACME), Balance::RoundRobin);
    assert_eq!(back.balance(ModelProvider::new("other")), Balance::Off);
    let info = ProviderInfo { provider: ACME, name: "Acme".into(), access: Access::Key };
    assert_eq!(book.summaries([info])[0].balance, Balance::RoundRobin);
}
