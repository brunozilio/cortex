#![allow(clippy::unwrap_used, reason = "a failed step fails the test")]

use tunnel::handshake::Joined;
use tunnel::link::{PairingId, pairing_id};
use tunnel::{Admission, HandshakeError, Initiator, Key, PairingLink, StaticKey, Transport, accept};

#[derive(Default)]
struct Book {
    devices: Vec<Key>,
    codes: Vec<Key>,
}

impl Admission for Book {
    fn knows(&self, device: &Key) -> bool {
        self.devices.contains(device)
    }

    fn pending(&self, pairing: &PairingId) -> Option<Key> {
        self.codes.iter().find(|code| &pairing_id(code) == pairing).copied()
    }
}

fn exchange(phone: &mut Transport, machine: &mut Transport, message: &[u8]) -> Vec<u8> {
    let mut opened = None;
    for piece in phone.seal(message).unwrap() {
        opened = machine.open(&piece).unwrap();
    }
    opened.unwrap()
}

#[test]
fn a_paired_phone_and_its_machine_talk_both_ways() {
    let machine = StaticKey::generate().unwrap();
    let phone = StaticKey::generate().unwrap();
    let book = Book { devices: vec![*phone.public()], ..Book::default() };
    let mut initiator = Initiator::paired(&phone, machine.public()).unwrap();
    let accepted = accept(&machine, &initiator.hello(&[]).unwrap(), &book).unwrap();
    assert_eq!(accepted.device, *phone.public());
    assert_eq!(accepted.joined, None);
    let mut on_phone = initiator.finish(&accepted.reply).unwrap();
    let mut on_machine = accepted.transport;
    assert_eq!(exchange(&mut on_phone, &mut on_machine, b"ask"), b"ask");
    assert_eq!(exchange(&mut on_machine, &mut on_phone, b"answer"), b"answer");
    assert_eq!(on_machine.remote(), Some(*phone.public()));
}

#[test]
fn a_phone_the_machine_never_paired_is_refused() {
    let machine = StaticKey::generate().unwrap();
    let stranger = StaticKey::generate().unwrap();
    let mut initiator = Initiator::paired(&stranger, machine.public()).unwrap();
    let refused = accept(&machine, &initiator.hello(&[]).unwrap(), &Book::default()).unwrap_err();
    assert_eq!(refused, HandshakeError::UnknownDevice);
}

#[test]
fn a_phone_that_trusts_another_machine_key_cannot_open_the_handshake() {
    let machine = StaticKey::generate().unwrap();
    let impostor = StaticKey::generate().unwrap();
    let phone = StaticKey::generate().unwrap();
    let book = Book { devices: vec![*phone.public()], ..Book::default() };
    let mut initiator = Initiator::paired(&phone, impostor.public()).unwrap();
    assert!(matches!(accept(&machine, &initiator.hello(&[]).unwrap(), &book), Err(HandshakeError::Noise(_))));
}

#[test]
fn a_phone_with_a_pending_code_pairs_and_names_itself() {
    let machine = StaticKey::generate().unwrap();
    let phone = StaticKey::generate().unwrap();
    let link = PairingLink { machine: *machine.public(), code: [5; 32] };
    let book = Book { codes: vec![link.code], ..Book::default() };
    let mut initiator = Initiator::pairing(&phone, &link).unwrap();
    let accepted = accept(&machine, &initiator.hello(b" Bruno's phone ").unwrap(), &book).unwrap();
    assert_eq!(accepted.joined, Some(Joined { pairing: link.pairing_id(), name: "Bruno's phone".into() }));
    assert_eq!(accepted.device, *phone.public());
    let mut on_phone = initiator.finish(&accepted.reply).unwrap();
    let mut on_machine = accepted.transport;
    assert_eq!(exchange(&mut on_phone, &mut on_machine, b"hi"), b"hi");
}

#[test]
fn a_code_that_is_not_pending_pairs_nothing() {
    let machine = StaticKey::generate().unwrap();
    let phone = StaticKey::generate().unwrap();
    let link = PairingLink { machine: *machine.public(), code: [5; 32] };
    let book = Book { codes: vec![[6; 32]], ..Book::default() };
    let mut initiator = Initiator::pairing(&phone, &link).unwrap();
    let refused = accept(&machine, &initiator.hello(b"phone").unwrap(), &book).unwrap_err();
    assert_eq!(refused, HandshakeError::UnknownPairing);
}

#[test]
fn a_pairing_phone_must_give_a_short_printable_name() {
    let machine = StaticKey::generate().unwrap();
    let link = PairingLink { machine: *machine.public(), code: [5; 32] };
    let book = Book { codes: vec![link.code], ..Book::default() };
    for name in [&b"   "[..], &[b'a'; 65][..], b"bad\x07name", &[0xff, 0xfe][..]] {
        let mut initiator = Initiator::pairing(&StaticKey::generate().unwrap(), &link).unwrap();
        assert_eq!(accept(&machine, &initiator.hello(name).unwrap(), &book).unwrap_err(), HandshakeError::Name);
    }
}

#[test]
fn a_first_message_with_no_known_mode_is_refused() {
    let machine = StaticKey::generate().unwrap();
    assert_eq!(accept(&machine, &[], &Book::default()).unwrap_err(), HandshakeError::Empty);
    assert_eq!(accept(&machine, &[9, 1, 2], &Book::default()).unwrap_err(), HandshakeError::Mode(9));
    assert_eq!(accept(&machine, &[2, 1, 2], &Book::default()).unwrap_err(), HandshakeError::Empty);
}

fn connected() -> (Transport, Transport) {
    let machine = StaticKey::generate().unwrap();
    let phone = StaticKey::generate().unwrap();
    let book = Book { devices: vec![*phone.public()], ..Book::default() };
    let mut initiator = Initiator::paired(&phone, machine.public()).unwrap();
    let accepted = accept(&machine, &initiator.hello(&[]).unwrap(), &book).unwrap();
    (initiator.finish(&accepted.reply).unwrap(), accepted.transport)
}

#[test]
fn a_message_larger_than_one_noise_message_arrives_whole() {
    let (mut phone, mut machine) = connected();
    let large: Vec<u8> = (0..200_000u32).map(|at| (at % 251) as u8).collect();
    let pieces = phone.seal(&large).unwrap();
    assert_eq!(pieces.len(), 4);
    assert!(pieces.iter().all(|piece| piece.len() <= 65_535));
    let opened: Vec<_> = pieces.iter().map(|piece| machine.open(piece).unwrap()).collect();
    assert!(opened[..3].iter().all(Option::is_none));
    assert_eq!(opened[3].as_deref(), Some(&large[..]));
    assert_eq!(exchange(&mut phone, &mut machine, b""), b"");
}

#[test]
fn a_piece_changed_on_the_way_does_not_open() {
    let (mut phone, mut machine) = connected();
    let mut piece = phone.seal(b"rm -rf").unwrap().remove(0);
    piece[3] ^= 1;
    assert!(machine.open(&piece).is_err());
}

#[test]
fn a_piece_replayed_by_the_relay_does_not_open_twice() {
    let (mut phone, mut machine) = connected();
    let piece = phone.seal(b"once").unwrap().remove(0);
    assert_eq!(machine.open(&piece).unwrap().as_deref(), Some(&b"once"[..]));
    assert!(machine.open(&piece).is_err());
}
