//! A page's picture as the server streams it: the parts that changed
//! since the last frame, each encoded on its own.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::BrowserId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TileCodec {
    Jpeg,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameTile {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub codec: TileCodec,
    #[serde(serialize_with = "to_base64", deserialize_with = "from_base64")]
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageFrame {
    pub id: BrowserId,
    pub size: (u32, u32),
    pub whole: bool,
    pub tiles: Vec<FrameTile>,
}

impl PageFrame {
    pub fn bytes(&self) -> usize {
        self.tiles.iter().map(|tile| tile.data.len()).sum()
    }
}

fn to_base64<S: Serializer>(data: &[u8], serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&STANDARD.encode(data))
}

fn from_base64<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<u8>, D::Error> {
    let text = String::deserialize(deserializer)?;
    STANDARD.decode(text).map_err(serde::de::Error::custom)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiles_travel_as_base64() {
        let frame = PageFrame {
            id: BrowserId::new(),
            size: (64, 32),
            whole: true,
            tiles: vec![FrameTile { x: 0, y: 0, w: 64, h: 32, codec: TileCodec::Jpeg, data: vec![0xff, 0xd8, 1] }],
        };
        let json = serde_json::to_string(&frame).expect("serialises");
        assert!(json.contains("\"data\":\"/9gB\""), "{json}");
        assert_eq!(serde_json::from_str::<PageFrame>(&json).expect("parses"), frame);
        assert_eq!(frame.bytes(), 3);
    }
}
