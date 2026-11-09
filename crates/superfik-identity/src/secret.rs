//! Passwords, first-access codes and sign-in tokens. Only hashes are ever
//! stored.

use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use password_hash::SaltString;
use password_hash::rand_core::OsRng;
use sha2::{Digest, Sha256};

pub const MIN_PASSWORD: usize = 10;

const CODE_LETTERS: &[u8; 30] = b"23456789ABCDEFGHJKMNPQRSTVWXYZ";
const CODE_LENGTH: usize = 12;

pub(crate) fn hash_password(password: &str) -> Option<String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default().hash_password(password.as_bytes(), &salt).ok().map(|hash| hash.to_string())
}

pub(crate) const UNKNOWN_ACCOUNT_HASH: &str =
    "$argon2id$v=19$m=19456,t=2,p=1$kA7ug/DkXViShiovmCcQFw$um2df5UkV53uPBrsh8H+zVUGIOlTCCWncBwIHBs+mMc";

pub(crate) fn password_matches(password: &str, hash: &str) -> bool {
    PasswordHash::new(hash).is_ok_and(|parsed| Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok())
}

pub fn random_token() -> Option<String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).ok()?;
    Some(hex(&bytes))
}

pub fn first_access_code() -> Option<String> {
    let mut bytes = [0u8; CODE_LENGTH];
    getrandom::fill(&mut bytes).ok()?;
    let letters: Vec<char> = bytes.iter().map(|byte| char::from(CODE_LETTERS[usize::from(byte % 30)])).collect();
    Some(letters.chunks(4).map(|group| group.iter().collect::<String>()).collect::<Vec<_>>().join("-"))
}

pub(crate) fn normalized_code(code: &str) -> String {
    code.chars().filter(char::is_ascii_alphanumeric).map(|c| c.to_ascii_uppercase()).collect()
}

pub fn code_hash(code: &str) -> String {
    hex(&Sha256::digest(code.as_bytes()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_password_matches_its_hash_only() {
        let hash = hash_password("correct horse battery").expect("hash");
        assert!(password_matches("correct horse battery", &hash));
        assert!(!password_matches("correct horse battery!", &hash));
        assert!(!password_matches("correct horse battery", "not a hash"));
    }

    #[test]
    fn the_stand_in_hash_costs_what_a_real_one_does() {
        let real = hash_password("correct horse battery").expect("hash");
        let params = |hash: &str| hash.split('$').nth(3).map(str::to_owned);
        assert_eq!(params(UNKNOWN_ACCOUNT_HASH), params(&real));
        assert!(PasswordHash::new(UNKNOWN_ACCOUNT_HASH).is_ok());
        assert!(!password_matches("correct horse battery", UNKNOWN_ACCOUNT_HASH));
    }

    #[test]
    fn tokens_are_long_and_never_repeat() {
        let (a, b) = (random_token().expect("token"), random_token().expect("token"));
        assert_eq!(a.len(), 64);
        assert_ne!(a, b);
    }

    #[test]
    fn codes_are_three_groups_of_four_unambiguous_letters() {
        let code = first_access_code().expect("code");
        assert_eq!(code.len(), 14, "{code}");
        assert_eq!(code.matches('-').count(), 2);
        assert!(code.chars().filter(|c| *c != '-').all(|c| CODE_LETTERS.contains(&(c as u8))), "{code}");
        assert_ne!(first_access_code(), Some(code));
    }

    #[test]
    fn a_code_is_read_back_however_it_was_typed() {
        assert_eq!(normalized_code("ab2c-def3 ghjk"), "AB2CDEF3GHJK");
    }

    #[test]
    fn a_hash_is_the_sha256_hex() {
        assert_eq!(code_hash("abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }
}
