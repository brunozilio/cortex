//! Locally valid signatures without a personal certificate or Keychain grant.
use super::*;

#[derive(Debug)]
pub struct AdHoc;

impl Signer for AdHoc {
    fn available(&self) -> bool {
        true
    }
    fn provisioned(&self) -> bool {
        false
    }
    fn sign(&self, path: &Path, _: Option<&Path>) -> io::Result<()> {
        sign_with("-", path, None)
    }
}
