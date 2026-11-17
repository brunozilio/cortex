use sha2::{Digest, Sha256};
use snow::params::DHChoice;
use snow::resolvers::{CryptoResolver, DefaultResolver};
use snow::types::Dh;

use crate::keys::{KEY_BYTES, Key, StaticKey, hex, machine_id};

pub const CHALLENGE: &str = "challenge";
pub const PROOF: &str = "proof";
pub const PROVEN: &str = "proven";
pub const LABEL: &[u8] = b"relay-machine-proof";
pub const UNPROVEN_CLOSE: u16 = 4003;
pub const TAKEN_CLOSE: u16 = 4009;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProofError {
    #[error("the message is not a {0}")]
    Shape(&'static str),
    #[error("the key exchange failed")]
    Exchange,
    #[error("that key is not the machine's with this id")]
    Machine,
    #[error("the proof does not match")]
    Mismatch,
}

pub struct Challenge {
    ephemeral: Box<dyn Dh>,
    nonce: Key,
}

impl std::fmt::Debug for Challenge {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Challenge").field("nonce", &hex(&self.nonce)).finish_non_exhaustive()
    }
}

fn exchange() -> Result<Box<dyn Dh>, ProofError> {
    DefaultResolver.resolve_dh(&DHChoice::Curve25519).ok_or(ProofError::Exchange)
}

impl Challenge {
    pub fn new() -> Result<Self, ProofError> {
        let mut random = DefaultResolver.resolve_rng().ok_or(ProofError::Exchange)?;
        let mut ephemeral = exchange()?;
        ephemeral.generate(random.as_mut()).map_err(|_| ProofError::Exchange)?;
        let mut nonce = [0; KEY_BYTES];
        random.try_fill_bytes(&mut nonce).map_err(|_| ProofError::Exchange)?;
        Ok(Self { ephemeral, nonce })
    }

    pub fn fixed_for_testing_only(ephemeral: &Key, nonce: Key) -> Result<Self, ProofError> {
        let mut dh = exchange()?;
        dh.set(ephemeral);
        Ok(Self { ephemeral: dh, nonce })
    }

    pub fn text(&self) -> String {
        format!("{CHALLENGE} {} {}", hex(self.ephemeral.pubkey()), hex(&self.nonce))
    }

    pub fn check(&self, machine: &str, answer: &str) -> Result<(), ProofError> {
        let mut words = answer.split(' ');
        let (Some(PROOF), Some(public), Some(given), None) = (words.next(), words.next(), words.next(), words.next())
        else {
            return Err(ProofError::Shape(PROOF));
        };
        let public = unhex(public).ok_or(ProofError::Shape(PROOF))?;
        let given = unhex(given).ok_or(ProofError::Shape(PROOF))?;
        if machine_id(&public) != machine {
            return Err(ProofError::Machine);
        }
        let mut shared = [0; KEY_BYTES];
        self.ephemeral.dh(&public, &mut shared).map_err(|_| ProofError::Exchange)?;
        let wanted = tag(&shared, &self.nonce, machine);
        let differ = wanted.iter().zip(given.iter()).fold(0u8, |diff, (a, b)| diff | (a ^ b));
        if differ == 0 { Ok(()) } else { Err(ProofError::Mismatch) }
    }
}

pub fn answer(machine: &StaticKey, challenge: &str) -> Result<String, ProofError> {
    let mut words = challenge.split(' ');
    let (Some(CHALLENGE), Some(ephemeral), Some(nonce), None) =
        (words.next(), words.next(), words.next(), words.next())
    else {
        return Err(ProofError::Shape(CHALLENGE));
    };
    let ephemeral = unhex(ephemeral).ok_or(ProofError::Shape(CHALLENGE))?;
    let nonce = unhex(nonce).ok_or(ProofError::Shape(CHALLENGE))?;
    let mut own = exchange()?;
    own.set(machine.private());
    let mut shared = [0; KEY_BYTES];
    own.dh(&ephemeral, &mut shared).map_err(|_| ProofError::Exchange)?;
    Ok(format!("{PROOF} {} {}", hex(machine.public()), hex(&tag(&shared, &nonce, &machine.machine_id()))))
}

fn tag(shared: &[u8], nonce: &[u8], machine: &str) -> Key {
    Sha256::new()
        .chain_update(LABEL)
        .chain_update(shared)
        .chain_update(nonce)
        .chain_update(machine.as_bytes())
        .finalize()
        .into()
}

fn unhex(text: &str) -> Option<Key> {
    if text.len() != KEY_BYTES * 2 {
        return None;
    }
    let mut out = [0; KEY_BYTES];
    for (at, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(text.get(at * 2..at * 2 + 2)?, 16).ok()?;
    }
    Some(out)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "a failed step fails the test")]
mod tests {
    use super::*;

    #[test]
    fn the_machine_that_holds_the_key_proves_it_and_no_other_can() {
        let machine = StaticKey::generate().unwrap();
        let challenge = Challenge::new().unwrap();
        let proof = answer(&machine, &challenge.text()).unwrap();
        assert_eq!(challenge.check(&machine.machine_id(), &proof), Ok(()));

        let impostor = StaticKey::generate().unwrap();
        let theirs = answer(&impostor, &challenge.text()).unwrap();
        assert_eq!(challenge.check(&machine.machine_id(), &theirs), Err(ProofError::Machine));

        let claimed = format!("{PROOF} {} {}", hex(machine.public()), hex(&[7; KEY_BYTES]));
        assert_eq!(challenge.check(&machine.machine_id(), &claimed), Err(ProofError::Mismatch));

        let replayed = Challenge::new().unwrap();
        assert_eq!(
            replayed.check(&machine.machine_id(), &proof),
            Err(ProofError::Mismatch),
            "a proof is for one challenge"
        );
        assert!(matches!(challenge.check(&machine.machine_id(), "proof x"), Err(ProofError::Shape(_))));
    }
}
