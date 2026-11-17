use crate::keys::{Key, key};

pub const MOST_MESSAGE_BYTES: usize = 16 * 1024 * 1024;
const LARGEST_PLAINTEXT: usize = 65_535 - 16;
const PIECE_BYTES: usize = LARGEST_PLAINTEXT - 1;
const LAST: u8 = 0;
const MORE: u8 = 1;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TransportError {
    #[error("a sealed message did not open: {0}")]
    Open(String),
    #[error("a message could not be sealed: {0}")]
    Seal(String),
    #[error("a message opened without its marker")]
    Marker,
    #[error("a message is larger than {MOST_MESSAGE_BYTES} bytes")]
    TooLarge,
}

#[derive(Debug)]
pub struct Transport {
    state: snow::TransportState,
    partial: Vec<u8>,
}

impl Transport {
    pub(crate) fn new(state: snow::TransportState) -> Self {
        Self { state, partial: Vec::new() }
    }

    pub fn remote(&self) -> Option<Key> {
        self.state.get_remote_static().and_then(|bytes| key(bytes).ok())
    }

    pub fn seal(&mut self, message: &[u8]) -> Result<Vec<Vec<u8>>, TransportError> {
        if message.len() > MOST_MESSAGE_BYTES {
            return Err(TransportError::TooLarge);
        }
        let pieces: Vec<&[u8]> = if message.is_empty() { vec![&[]] } else { message.chunks(PIECE_BYTES).collect() };
        let last = pieces.len() - 1;
        pieces
            .iter()
            .enumerate()
            .map(|(at, piece)| self.seal_piece(piece, if at == last { LAST } else { MORE }))
            .collect()
    }

    fn seal_piece(&mut self, piece: &[u8], marker: u8) -> Result<Vec<u8>, TransportError> {
        let mut plain = Vec::with_capacity(piece.len() + 1);
        plain.push(marker);
        plain.extend_from_slice(piece);
        let mut sealed = vec![0; plain.len() + 16];
        let written =
            self.state.write_message(&plain, &mut sealed).map_err(|error| TransportError::Seal(error.to_string()))?;
        sealed.truncate(written);
        Ok(sealed)
    }

    pub fn open(&mut self, sealed: &[u8]) -> Result<Option<Vec<u8>>, TransportError> {
        let mut plain = vec![0; sealed.len()];
        let read =
            self.state.read_message(sealed, &mut plain).map_err(|error| TransportError::Open(error.to_string()))?;
        let Some((&marker, piece)) = plain[..read].split_first() else { return Err(TransportError::Marker) };
        if self.partial.len() + piece.len() > MOST_MESSAGE_BYTES {
            self.partial.clear();
            return Err(TransportError::TooLarge);
        }
        self.partial.extend_from_slice(piece);
        match marker {
            LAST => Ok(Some(std::mem::take(&mut self.partial))),
            MORE => Ok(None),
            _ => Err(TransportError::Marker),
        }
    }
}
