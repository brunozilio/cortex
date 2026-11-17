const OPEN: u8 = 1;
const DATA: u8 = 2;
const CLOSE: u8 = 3;
const HEADER: usize = 5;

pub const MOST_PAYLOAD_BYTES: usize = 65_535;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Frame {
    Open(u32),
    Data(u32, Vec<u8>),
    Close(u32),
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FrameError {
    #[error("a frame is shorter than its header")]
    Short,
    #[error("no frame has kind {0}")]
    Kind(u8),
    #[error("an open or close frame carries nothing")]
    Payload,
    #[error("a frame carries more than {MOST_PAYLOAD_BYTES} bytes")]
    TooLarge,
}

impl Frame {
    pub fn channel(&self) -> u32 {
        match self {
            Self::Open(channel) | Self::Data(channel, _) | Self::Close(channel) => *channel,
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        let (kind, payload): (u8, &[u8]) = match self {
            Self::Open(_) => (OPEN, &[]),
            Self::Data(_, payload) => (DATA, payload),
            Self::Close(_) => (CLOSE, &[]),
        };
        let mut bytes = Vec::with_capacity(HEADER + payload.len());
        bytes.push(kind);
        bytes.extend_from_slice(&self.channel().to_be_bytes());
        bytes.extend_from_slice(payload);
        bytes
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, FrameError> {
        let Some((&kind, rest)) = bytes.split_first() else { return Err(FrameError::Short) };
        let (channel, payload) = rest.split_at_checked(HEADER - 1).ok_or(FrameError::Short)?;
        let channel = u32::from_be_bytes([channel[0], channel[1], channel[2], channel[3]]);
        if payload.len() > MOST_PAYLOAD_BYTES {
            return Err(FrameError::TooLarge);
        }
        match kind {
            OPEN | CLOSE if !payload.is_empty() => Err(FrameError::Payload),
            OPEN => Ok(Self::Open(channel)),
            CLOSE => Ok(Self::Close(channel)),
            DATA => Ok(Self::Data(channel, payload.to_vec())),
            other => Err(FrameError::Kind(other)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_frame_reads_back_as_it_was_written() {
        for frame in [Frame::Open(7), Frame::Data(u32::MAX, b"sealed".to_vec()), Frame::Close(0)] {
            assert_eq!(Frame::decode(&frame.encode()), Ok(frame));
        }
    }

    #[test]
    fn a_frame_without_its_whole_header_is_refused() {
        assert_eq!(Frame::decode(&[]), Err(FrameError::Short));
        assert_eq!(Frame::decode(&[DATA, 0, 0, 1]), Err(FrameError::Short));
    }

    #[test]
    fn an_unknown_kind_or_an_open_carrying_bytes_is_refused() {
        assert_eq!(Frame::decode(&[9, 0, 0, 0, 1]), Err(FrameError::Kind(9)));
        assert_eq!(Frame::decode(&[OPEN, 0, 0, 0, 1, 42]), Err(FrameError::Payload));
    }

    #[test]
    fn a_payload_larger_than_one_noise_message_is_refused() {
        let frame = Frame::Data(1, vec![0; MOST_PAYLOAD_BYTES + 1]).encode();
        assert_eq!(Frame::decode(&frame), Err(FrameError::TooLarge));
    }
}
