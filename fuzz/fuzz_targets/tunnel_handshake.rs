#![no_main]

use libfuzzer_sys::fuzz_target;
use tunnel::handshake::{Admission, Initiator, accept_with_fixed_ephemeral};
use tunnel::keys::machine_id;
use tunnel::link::{PairingId, pairing_id};
use tunnel::proof::{Challenge, answer};
use tunnel::{Key, PairingLink, StaticKey};

const MACHINE: Key = [7; 32];
const DEVICE: Key = [9; 32];
const STRANGER: Key = [11; 32];
const CODE: Key = [13; 32];
const EPHEMERAL: Key = [17; 32];
const NONCE: Key = [19; 32];

struct Known {
    device: Key,
}

impl Admission for Known {
    fn knows(&self, device: &Key) -> bool {
        *device == self.device
    }

    fn pending(&self, pairing: &PairingId) -> Option<Key> {
        (*pairing == pairing_id(&CODE)).then_some(CODE)
    }
}

fn hello(device: &StaticKey, machine: &StaticKey, mode: u8, payload: &[u8]) -> Option<Vec<u8>> {
    let mut initiator = if mode.is_multiple_of(2) {
        Initiator::paired_with(device, machine.public(), Some(&EPHEMERAL)).ok()?
    } else {
        let link = PairingLink { machine: *machine.public(), code: CODE };
        Initiator::pairing_with(device, &link, Some(&EPHEMERAL)).ok()?
    };
    initiator.hello(payload).ok()
}

fn bent(mut message: Vec<u8>, changes: &[u8]) -> Vec<u8> {
    for [at, flip] in changes.as_chunks::<2>().0 {
        if message.is_empty() {
            break;
        }
        let at = usize::from(*at) % message.len();
        message[at] ^= flip;
    }
    message
}

fuzz_target!(|data: &[u8]| {
    let (Ok(machine), Ok(device), Ok(stranger)) =
        (StaticKey::from_private(&MACHINE), StaticKey::from_private(&DEVICE), StaticKey::from_private(&STRANGER))
    else {
        return;
    };
    let admission = Known { device: *device.public() };
    let Some((&mode, rest)) = data.split_first() else { return };
    let (sent, split) = rest.split_at(rest.len() / 2);

    let genuine = match mode % 4 {
        0 => Some(rest.to_vec()),
        1 => hello(&device, &machine, mode / 4, sent).map(|hello| bent(hello, split)),
        2 => hello(&stranger, &machine, mode / 4, sent),
        _ => hello(&device, &machine, mode / 4, sent),
    };
    let Some(first) = genuine else { return };
    if let Ok(mut accepted) = accept_with_fixed_ephemeral(&machine, &first, &admission, &EPHEMERAL) {
        match &accepted.joined {
            None => assert_eq!(accepted.device, *device.public(), "an unknown device was let in"),
            Some(joined) => {
                assert_eq!(joined.pairing, pairing_id(&CODE));
                assert!(!joined.name.is_empty() && joined.name.len() <= 64);
            }
        }
        for sealed in split.split(|&byte| byte == 0) {
            assert!(accepted.transport.open(sealed).is_err(), "a forged message opened");
        }
        let _ = accepted.transport.seal(sent);
    }

    if let Ok(mut initiator) = Initiator::paired_with(&device, machine.public(), Some(&EPHEMERAL))
        && initiator.hello(&[]).is_ok()
    {
        let _ = initiator.finish(rest);
    }

    let text = String::from_utf8_lossy(rest);
    let _ = answer(&machine, &text);
    if let Ok(challenge) = Challenge::fixed_for_testing_only(&EPHEMERAL, NONCE) {
        let genuine = answer(&machine, &challenge.text()).ok();
        if genuine.is_some_and(|genuine| !genuine.eq_ignore_ascii_case(&text)) {
            assert!(challenge.check(&machine_id(machine.public()), &text).is_err(), "a forged proof passed: {text:?}");
        }
    }
});
