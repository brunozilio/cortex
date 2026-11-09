//! Short words the user writes in Settings › Voice (the wake word, what
//! the agent calls them), kept inline so preferences stay `Copy`. Pure.

pub const SHORT_TEXT_BYTES: usize = 48;

macro_rules! short_text {
    ($(#[$meta:meta])* $name:ident = $default:literal) => {
        $(#[$meta])*
        #[derive(Clone, Copy, PartialEq, Eq, Hash)]
        pub struct $name {
            bytes: [u8; $crate::short_text::SHORT_TEXT_BYTES],
            len: u8,
        }

        impl $name {
            pub const DEFAULT: &'static str = $default;

            pub fn new(text: &str) -> Option<Self> {
                let text = text.trim();
                if text.is_empty() {
                    return None;
                }
                let mut end = text.len().min($crate::short_text::SHORT_TEXT_BYTES);
                while !text.is_char_boundary(end) {
                    end -= 1;
                }
                let mut bytes = [0; $crate::short_text::SHORT_TEXT_BYTES];
                bytes[..end].copy_from_slice(&text.as_bytes()[..end]);
                Some(Self { bytes, len: end as u8 })
            }

            pub fn as_str(&self) -> &str {
                std::str::from_utf8(&self.bytes[..usize::from(self.len)]).unwrap_or_default()
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new(Self::DEFAULT)
                    .unwrap_or(Self { bytes: [0; $crate::short_text::SHORT_TEXT_BYTES], len: 0 })
            }
        }

        impl std::fmt::Debug for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, concat!(stringify!($name), "({:?})"), self.as_str())
            }
        }

        impl serde::Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(self.as_str())
            }
        }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let text = <String as serde::Deserialize>::deserialize(deserializer)?;
                Ok(Self::new(&text).unwrap_or_default())
            }
        }
    };
}

pub(crate) use short_text;

short_text! {
    Address = "Capitão"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_user_is_called_captain_unless_changed() {
        assert_eq!(Address::default().as_str(), "Capitão");
        let chefe = Address::new(" Chefe ").expect("address");
        assert_eq!(serde_json::to_string(&chefe).expect("json"), r#""Chefe""#);
        let empty: Address = serde_json::from_str(r#""""#).expect("parses");
        assert_eq!(empty, Address::default());
    }
}
