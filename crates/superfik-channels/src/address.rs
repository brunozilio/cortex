//! Conversation ids and WhatsApp addresses (JIDs, `user@server`).
//!
//! A conversation is named by a contact's phone digits when WhatsApp gives
//! them, else by the full JID (a LID or a group).
//! Pure, so the mapping is tested without the protocol.

const PHONE_SERVER: &str = "s.whatsapp.net";
const PHONE_DIGITS: std::ops::RangeInclusive<usize> = 6..=20;

pub fn exchange_of(jid: &str) -> String {
    match jid.split_once('@') {
        Some((user, PHONE_SERVER)) => {
            let user = user.split(':').next().unwrap_or(user);
            if PHONE_DIGITS.contains(&user.len()) && user.bytes().all(|b| b.is_ascii_digit()) {
                user.to_owned()
            } else {
                jid.to_owned()
            }
        }
        _ => jid.to_owned(),
    }
}

pub fn addressed_to(mentioned: &[String], quoted_author: Option<&str>, me: &[String]) -> (bool, bool) {
    let user = |jid: &str| jid.split(['@', ':']).next().unwrap_or(jid).to_owned();
    let mine: Vec<String> = me.iter().map(|jid| user(jid)).filter(|u| !u.is_empty()).collect();
    let is_me = |jid: &str| mine.contains(&user(jid));
    (mentioned.iter().any(|jid| is_me(jid)), quoted_author.is_some_and(is_me))
}

pub fn recipient_of(exchange: &str) -> Option<String> {
    let digits: String = exchange.chars().filter(|c| !matches!(c, '+' | ' ' | '-' | '(' | ')')).collect();
    if PHONE_DIGITS.contains(&digits.len()) && digits.bytes().all(|b| b.is_ascii_digit()) {
        return Some(format!("{digits}@{PHONE_SERVER}"));
    }
    let (user, server) = exchange.split_once('@')?;
    (!user.is_empty() && !server.is_empty() && !exchange.contains(char::is_whitespace)).then(|| exchange.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contacts_are_named_by_their_phone_digits() {
        assert_eq!(exchange_of("5511999990000@s.whatsapp.net"), "5511999990000");
        assert_eq!(exchange_of("5511999990000:12@s.whatsapp.net"), "5511999990000");
    }

    #[test]
    fn groups_and_lids_keep_their_jid() {
        assert_eq!(exchange_of("120363040000000000@g.us"), "120363040000000000@g.us");
        assert_eq!(exchange_of("98765432101234@lid"), "98765432101234@lid");
        assert_eq!(exchange_of("abc@s.whatsapp.net"), "abc@s.whatsapp.net");
    }

    #[test]
    fn a_group_message_is_for_the_number_when_it_mentions_or_quotes_it() {
        let me = vec!["5511888880000:7@s.whatsapp.net".to_owned(), "98765@lid".to_owned()];
        let mention = vec!["5511888880000@s.whatsapp.net".to_owned()];
        assert_eq!(addressed_to(&mention, None, &me), (true, false));
        assert_eq!(addressed_to(&["98765@lid".to_owned()], None, &me), (true, false));
        assert_eq!(addressed_to(&[], Some("5511888880000@s.whatsapp.net"), &me), (false, true));
        assert_eq!(addressed_to(&["5511000000000@s.whatsapp.net".to_owned()], Some("1@lid"), &me), (false, false));
        assert_eq!(addressed_to(&mention, None, &[]), (false, false), "before pairing, no one is me");
    }

    #[test]
    fn a_conversation_sends_to_its_jid() {
        assert_eq!(recipient_of("5511999990000").as_deref(), Some("5511999990000@s.whatsapp.net"));
        assert_eq!(recipient_of("+55 (11) 99999-0000").as_deref(), Some("5511999990000@s.whatsapp.net"));
        assert_eq!(recipient_of("120363040000000000@g.us").as_deref(), Some("120363040000000000@g.us"));
        assert_eq!(recipient_of("not a number"), None);
        assert_eq!(recipient_of("@g.us"), None);
    }
}
