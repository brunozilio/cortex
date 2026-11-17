use serde_json::{Value, json};
use tunnel::handshake::accept_with_fixed_ephemeral;
use tunnel::link::{PairingId, pairing_id};
use tunnel::{Admission, Answer, Ask, Initiator, Key, PairingLink, Payload, StaticKey, Transport};

struct Book {
    device: Key,
    code: Key,
}

impl Admission for Book {
    fn knows(&self, device: &Key) -> bool {
        device == &self.device
    }

    fn pending(&self, pairing: &PairingId) -> Option<Key> {
        (pairing == &pairing_id(&self.code)).then_some(self.code)
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn key(fill: u8) -> StaticKey {
    StaticKey::from_private(&[fill; 32]).expect("a key")
}

fn sealed(transport: &mut Transport, plain: &[u8]) -> Vec<String> {
    transport.seal(plain).expect("sealed").iter().map(|piece| hex(piece)).collect()
}

fn exchanged(mut phone: Transport, mut machine: Transport, with_large: bool) -> Value {
    let ask = serde_json::to_vec(&Ask::Fetch {
        id: 1,
        method: "GET".into(),
        path: "/api/health".into(),
        body: None,
        content_type: None,
    })
    .expect("json");
    let answer =
        serde_json::to_vec(&Answer::Data { id: 1, data: Payload::bytes(b"{\"status\":\"ok\"}") }).expect("json");
    let size = if with_large { 70_000u32 } else { 0 };
    let large: Vec<u8> = (0..size).map(|at| b'a' + u8::try_from(at % 26).expect("a letter")).collect();
    json!({
        "phone_to_machine": { "plain": hex(&ask), "sealed": sealed(&mut phone, &ask) },
        "machine_to_phone": { "plain": hex(&answer), "sealed": sealed(&mut machine, &answer) },
        "machine_to_phone_large": { "plain": hex(&large), "sealed": sealed(&mut machine, &large) },
    })
}

fn pairing() -> Value {
    let (machine, device, ephemeral, responder_ephemeral) = (key(1), key(2), key(3), key(4));
    let link = PairingLink { machine: *machine.public(), code: [5; 32] };
    let book = Book { device: [0; 32], code: link.code };
    let mut initiator = Initiator::pairing_with(&device, &link, Some(ephemeral.private())).expect("initiator");
    let hello = initiator.hello(b"Test phone").expect("hello");
    let accepted =
        accept_with_fixed_ephemeral(&machine, &hello, &book, responder_ephemeral.private()).expect("accepted");
    let phone = initiator.finish(&accepted.reply).expect("finished");
    json!({
        "machine_public": hex(machine.public()),
        "device_private": hex(device.private()),
        "device_public": hex(device.public()),
        "ephemeral_private": hex(ephemeral.private()),
        "fragment": link.fragment(),
        "name": "Test phone",
        "hello": hex(&hello),
        "reply": hex(&accepted.reply),
        "transport": exchanged(phone, accepted.transport, false),
    })
}

fn paired() -> Value {
    let (machine, device, ephemeral, responder_ephemeral) = (key(11), key(12), key(13), key(14));
    let book = Book { device: *device.public(), code: [0; 32] };
    let mut initiator =
        Initiator::paired_with(&device, machine.public(), Some(ephemeral.private())).expect("initiator");
    let hello = initiator.hello(&[]).expect("hello");
    let accepted =
        accept_with_fixed_ephemeral(&machine, &hello, &book, responder_ephemeral.private()).expect("accepted");
    let phone = initiator.finish(&accepted.reply).expect("finished");
    json!({
        "machine_public": hex(machine.public()),
        "machine_id": machine.machine_id(),
        "device_private": hex(device.private()),
        "device_public": hex(device.public()),
        "ephemeral_private": hex(ephemeral.private()),
        "hello": hex(&hello),
        "reply": hex(&accepted.reply),
        "transport": exchanged(phone, accepted.transport, true),
    })
}

fn proof() -> Value {
    let machine = key(5);
    let ephemeral = [6; 32];
    let challenge = tunnel::proof::Challenge::fixed_for_testing_only(&ephemeral, [8; 32]).expect("challenge");
    let proof = tunnel::proof::answer(&machine, &challenge.text()).expect("proof");
    assert_eq!(challenge.check(&machine.machine_id(), &proof), Ok(()));
    json!({
        "machine_id": machine.machine_id(),
        "ephemeral_private": hex(&ephemeral),
        "nonce": hex(&[8; 32]),
        "challenge": challenge.text(),
        "proof": proof,
    })
}

pub fn transcript() -> Value {
    json!({ "pairing": pairing(), "paired": paired(), "proof": proof() })
}
