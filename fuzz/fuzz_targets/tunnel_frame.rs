#![no_main]

use libfuzzer_sys::fuzz_target;
use tunnel::{Answer, Ask, Frame, PairingLink, forwardable};

fn round_trip<T>(data: &[u8])
where
    T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
{
    let Ok(read) = serde_json::from_slice::<T>(data) else { return };
    let Ok(written) = serde_json::to_vec(&read) else { return };
    let again = serde_json::from_slice::<T>(&written);
    assert_eq!(again.as_ref().ok(), Some(&read), "a message did not read back as written");
}

fuzz_target!(|data: &[u8]| {
    if let Ok(frame) = Frame::decode(data) {
        assert_eq!(frame.encode(), data, "a frame did not encode back to its bytes");
        assert_eq!(Frame::decode(&frame.encode()).as_ref(), Ok(&frame));
    }

    round_trip::<Ask>(data);
    round_trip::<Answer>(data);
    if let Ok(ask) = serde_json::from_slice::<Ask>(data)
        && let Ask::Send { data: payload, .. } = &ask
    {
        let _ = payload.decoded();
    }
    if let Ok(answer) = serde_json::from_slice::<Answer>(data)
        && let Answer::Data { data: payload, .. } = &answer
    {
        let _ = payload.decoded();
    }

    let text = String::from_utf8_lossy(data);
    if let Ok(link) = PairingLink::parse(&text) {
        assert_eq!(PairingLink::parse(&link.fragment()), Ok(link.clone()));
        let _ = link.pairing_id();
    }
    if forwardable(&text) {
        let route = text.split('?').next().unwrap_or_default();
        assert!(route.starts_with("/api/"), "{text:?} is forwarded outside the api");
        assert!(!route.split('/').any(|part| part == ".." || part == "."), "{text:?} climbs out of the api");
        assert!(!text.contains(['#', '\\']), "{text:?} is forwarded with a fragment or a backslash");
    }
});
