//! A server's own users (ADR 0017): an admin adds a user by name and hands
//! them a first-access code; the code, used once, lets them choose a
//! password; from then on they sign in with name and password and get a
//! sign-in token, which the app keeps. Time is passed in (unix seconds).

mod directory;
mod file;
mod secret;
mod sign_in;
mod store;
#[cfg(test)]
mod tests;

pub use directory::{Actor, CODE_LIFETIME, Directory, IdentityError, TOKEN_LIFETIME, username};
pub use file::FileAccounts;
pub use secret::{MIN_PASSWORD, code_hash, first_access_code, random_token};
pub use sign_in::{FAILURES_BEFORE_LOCK, LOCK_SECONDS, MAX_TRACKED};
pub use store::{Account, AccountStore, FirstAccess, InMemoryAccounts, StoreError, Token};
