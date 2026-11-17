use rusqlite::types::Type;
use serde::Serialize;
use serde::de::DeserializeOwned;

pub(crate) fn name<T: Serialize>(value: &T) -> rusqlite::Result<String> {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(name)) => Ok(name),
        Ok(other) => Err(unwritable(format!("{other} is not a name"))),
        Err(error) => Err(unwritable(error)),
    }
}

pub(crate) fn named<T: DeserializeOwned>(column: usize, name: String) -> rusqlite::Result<T> {
    serde_json::from_value(serde_json::Value::String(name)).map_err(|error| unreadable(column, error))
}

pub(crate) fn json<T: Serialize>(value: &T) -> rusqlite::Result<String> {
    serde_json::to_string(value).map_err(unwritable)
}

pub(crate) fn from_json<T: DeserializeOwned>(column: usize, text: &str) -> rusqlite::Result<T> {
    serde_json::from_str(text).map_err(|error| unreadable(column, error))
}

pub(crate) fn stored(count: u64) -> i64 {
    i64::try_from(count).unwrap_or(i64::MAX)
}

pub(crate) fn counted(stored: i64) -> u64 {
    u64::try_from(stored).unwrap_or(0)
}

pub(crate) fn small(stored: i64) -> u32 {
    u32::try_from(stored.max(0)).unwrap_or(u32::MAX)
}

fn unwritable(error: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(error.into())
}

fn unreadable(column: usize, error: serde_json::Error) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(column, Type::Text, Box::new(error))
}

#[cfg(test)]
mod tests {
    use domain::conversation::SessionStatus;

    use super::*;

    #[test]
    fn an_enum_is_stored_under_the_name_its_json_has() {
        assert_eq!(name(&SessionStatus::NeedsYou).expect("name"), "needs_you");
        assert_eq!(named::<SessionStatus>(0, "needs_you".into()).expect("named"), SessionStatus::NeedsYou);
        assert!(named::<SessionStatus>(0, "unheard_of".into()).is_err());
        assert!(name(&7).is_err(), "only names are stored as names");
    }

    #[test]
    fn counts_outside_what_a_column_holds_are_clamped() {
        assert_eq!((stored(u64::MAX), counted(-1), small(-1), small(i64::MAX)), (i64::MAX, 0, 0, u32::MAX));
        assert_eq!(counted(stored(1_700_000_000)), 1_700_000_000);
    }
}
