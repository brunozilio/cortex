use crate::keys::{Key, StaticKey};
use crate::link::{PairingId, PairingLink, pairing_id, psk};
use crate::transport::Transport;

pub const IK: &str = "Noise_IK_25519_ChaChaPoly_SHA256";
pub const IK_PSK2: &str = "Noise_IKpsk2_25519_ChaChaPoly_SHA256";
pub const MOST_NAME_BYTES: usize = 64;
const PAIRED: u8 = 1;
const PAIRING: u8 = 2;
const PAIRING_HEADER: usize = 1 + 8;
const LARGEST_MESSAGE: usize = 65_535;

pub trait Admission {
    fn knows(&self, device: &Key) -> bool;
    fn pending(&self, pairing: &PairingId) -> Option<Key>;
}

#[derive(Debug)]
pub struct Accepted {
    pub reply: Vec<u8>,
    pub transport: Transport,
    pub device: Key,
    pub joined: Option<Joined>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Joined {
    pub pairing: PairingId,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HandshakeError {
    #[error("the first message was empty")]
    Empty,
    #[error("no handshake has mode {0}")]
    Mode(u8),
    #[error("no pairing is pending under that id")]
    UnknownPairing,
    #[error("that device is not paired with this machine")]
    UnknownDevice,
    #[error("a pairing device names itself in 1 to {MOST_NAME_BYTES} bytes of UTF-8")]
    Name,
    #[error("the handshake failed: {0}")]
    Noise(String),
}

fn noise(error: impl std::fmt::Display) -> HandshakeError {
    HandshakeError::Noise(error.to_string())
}

fn builder<'a>(pattern: &str, local: &'a StaticKey, prologue: &'a [u8]) -> Result<snow::Builder<'a>, HandshakeError> {
    let params = pattern.parse().map_err(noise)?;
    snow::Builder::new(params).local_private_key(local.private()).map_err(noise)?.prologue(prologue).map_err(noise)
}

pub fn accept(machine: &StaticKey, hello: &[u8], admission: &dyn Admission) -> Result<Accepted, HandshakeError> {
    accept_with(machine, hello, admission, None)
}

pub fn accept_with_fixed_ephemeral(
    machine: &StaticKey,
    hello: &[u8],
    admission: &dyn Admission,
    ephemeral: &Key,
) -> Result<Accepted, HandshakeError> {
    accept_with(machine, hello, admission, Some(ephemeral))
}

fn accept_with(
    machine: &StaticKey,
    hello: &[u8],
    admission: &dyn Admission,
    ephemeral: Option<&Key>,
) -> Result<Accepted, HandshakeError> {
    let (&mode, _) = hello.split_first().ok_or(HandshakeError::Empty)?;
    let (header, message, pattern, pending) = match mode {
        PAIRED => (&hello[..1], &hello[1..], IK, None),
        PAIRING => {
            let (header, message) = hello.split_at_checked(PAIRING_HEADER).ok_or(HandshakeError::Empty)?;
            let id: PairingId = header[1..].try_into().map_err(|_| HandshakeError::Empty)?;
            let code = admission.pending(&id).ok_or(HandshakeError::UnknownPairing)?;
            (header, message, IK_PSK2, Some((id, psk(&code))))
        }
        other => return Err(HandshakeError::Mode(other)),
    };
    let mut builder = builder(pattern, machine, header)?;
    if let Some((_, key)) = &pending {
        builder = builder.psk(2, key).map_err(noise)?;
    }
    if let Some(ephemeral) = ephemeral {
        builder = builder.fixed_ephemeral_key_for_testing_only(ephemeral);
    }
    let mut state = builder.build_responder().map_err(noise)?;
    let mut payload = vec![0; message.len()];
    let read = state.read_message(message, &mut payload).map_err(noise)?;
    let device = state
        .get_remote_static()
        .and_then(|bytes| crate::keys::key(bytes).ok())
        .ok_or(HandshakeError::UnknownDevice)?;
    let joined = match pending {
        Some((pairing, _)) => Some(Joined { pairing, name: device_name(&payload[..read])? }),
        None if admission.knows(&device) => None,
        None => return Err(HandshakeError::UnknownDevice),
    };
    let mut reply = vec![0; LARGEST_MESSAGE];
    let written = state.write_message(&[], &mut reply).map_err(noise)?;
    reply.truncate(written);
    let transport = Transport::new(state.into_transport_mode().map_err(noise)?);
    Ok(Accepted { reply, transport, device, joined })
}

fn device_name(payload: &[u8]) -> Result<String, HandshakeError> {
    let name = std::str::from_utf8(payload).map_err(|_| HandshakeError::Name)?.trim();
    if name.is_empty() || name.len() > MOST_NAME_BYTES || name.chars().any(char::is_control) {
        return Err(HandshakeError::Name);
    }
    Ok(name.to_owned())
}

#[derive(Debug)]
pub struct Initiator {
    state: snow::HandshakeState,
    header: Vec<u8>,
}

impl Initiator {
    pub fn paired(device: &StaticKey, machine: &Key) -> Result<Self, HandshakeError> {
        Self::paired_with(device, machine, None)
    }

    pub fn paired_with(device: &StaticKey, machine: &Key, ephemeral: Option<&Key>) -> Result<Self, HandshakeError> {
        let header = vec![PAIRED];
        let builder = builder(IK, device, &header)?.remote_public_key(machine).map_err(noise)?;
        let builder = match ephemeral {
            Some(key) => builder.fixed_ephemeral_key_for_testing_only(key),
            None => builder,
        };
        let state = builder.build_initiator().map_err(noise)?;
        Ok(Self { state, header })
    }

    pub fn pairing(device: &StaticKey, link: &PairingLink) -> Result<Self, HandshakeError> {
        Self::pairing_with(device, link, None)
    }

    pub fn pairing_with(
        device: &StaticKey,
        link: &PairingLink,
        ephemeral: Option<&Key>,
    ) -> Result<Self, HandshakeError> {
        let mut header = vec![PAIRING];
        header.extend_from_slice(&pairing_id(&link.code));
        let shared = link.psk();
        let builder = builder(IK_PSK2, device, &header)?
            .remote_public_key(&link.machine)
            .map_err(noise)?
            .psk(2, &shared)
            .map_err(noise)?;
        let builder = match ephemeral {
            Some(key) => builder.fixed_ephemeral_key_for_testing_only(key),
            None => builder,
        };
        let state = builder.build_initiator().map_err(noise)?;
        Ok(Self { state, header })
    }

    pub fn hello(&mut self, payload: &[u8]) -> Result<Vec<u8>, HandshakeError> {
        let mut message = vec![0; LARGEST_MESSAGE];
        let written = self.state.write_message(payload, &mut message).map_err(noise)?;
        let mut hello = self.header.clone();
        hello.extend_from_slice(&message[..written]);
        Ok(hello)
    }

    pub fn finish(mut self, reply: &[u8]) -> Result<Transport, HandshakeError> {
        let mut payload = vec![0; reply.len()];
        self.state.read_message(reply, &mut payload).map_err(noise)?;
        Ok(Transport::new(self.state.into_transport_mode().map_err(noise)?))
    }
}
