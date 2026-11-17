use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::{Deserialize, Serialize};

pub const API: &str = "/api/";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Ask {
    Fetch {
        id: u32,
        method: String,
        path: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        body: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        content_type: Option<String>,
    },
    Events {
        id: u32,
        path: String,
    },
    Socket {
        id: u32,
        path: String,
    },
    Send {
        id: u32,
        data: Payload,
    },
    Close {
        id: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Answer {
    Head {
        id: u32,
        status: u16,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        content_type: Option<String>,
    },
    Data {
        id: u32,
        data: Payload,
    },
    End {
        id: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum Payload {
    Text(String),
    Bytes(String),
}

impl Payload {
    pub fn bytes(bytes: &[u8]) -> Self {
        Self::Bytes(STANDARD.encode(bytes))
    }

    pub fn decoded(&self) -> Option<Vec<u8>> {
        match self {
            Self::Text(text) => Some(text.as_bytes().to_vec()),
            Self::Bytes(encoded) => STANDARD.decode(encoded).ok(),
        }
    }
}

impl Ask {
    pub fn id(&self) -> u32 {
        match self {
            Self::Fetch { id, .. }
            | Self::Events { id, .. }
            | Self::Socket { id, .. }
            | Self::Send { id, .. }
            | Self::Close { id } => *id,
        }
    }
}

impl Answer {
    pub fn id(&self) -> u32 {
        match self {
            Self::Head { id, .. } | Self::Data { id, .. } | Self::End { id, .. } => *id,
        }
    }
}

pub fn forwardable(path: &str) -> bool {
    let route = path.split('?').next().unwrap_or_default();
    route.starts_with(API)
        && !route.split('/').any(|part| part == ".." || part == ".")
        && !["%2e", "%2f", "%5c"].iter().any(|encoded| route.to_ascii_lowercase().contains(encoded))
        && !path.contains(['#', '\\'])
        && !path.chars().any(|letter| letter.is_control() || letter.is_whitespace())
        && !path.contains("//")
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "a failed step fails the test")]
mod tests {
    use super::*;

    #[test]
    fn a_request_reads_back_from_its_json() {
        let ask = Ask::Fetch {
            id: 3,
            method: "POST".into(),
            path: "/api/sessions/watch".into(),
            body: Some("{}".into()),
            content_type: Some("application/json".into()),
        };
        let text = serde_json::to_string(&ask).unwrap();
        assert!(text.contains("\"type\":\"fetch\""));
        assert_eq!(serde_json::from_str::<Ask>(&text).unwrap(), ask);
    }

    #[test]
    fn an_answer_names_its_kind_and_leaves_out_what_it_lacks() {
        let text = serde_json::to_string(&Answer::End { id: 1, error: None }).unwrap();
        assert_eq!(text, r#"{"type":"end","id":1}"#);
    }

    #[test]
    fn bytes_travel_as_base64_and_text_as_itself() {
        assert_eq!(Payload::bytes(b"\x00\xff").decoded(), Some(vec![0, 255]));
        assert_eq!(Payload::Text("ls\r".into()).decoded(), Some(b"ls\r".to_vec()));
        assert_eq!(serde_json::to_string(&Payload::Text("a".into())).unwrap(), r#"{"kind":"text","value":"a"}"#);
    }

    #[test]
    fn only_the_console_api_is_forwarded() {
        for good in ["/api/health", "/api/terminal?session=abc&cols=80", "/api/sessions/watch"] {
            assert!(forwardable(good), "{good}");
        }
        for bad in [
            "/index.html",
            "/api",
            "api/health",
            "/api/../secret",
            "/api/./health",
            "/api/%2e%2e/index.html",
            "/api/%2E./x",
            "/api/.%2e/x",
            "/api/x%2f..%2findex.html",
            "/api/x%5C..",
            "//evil.example/api/x",
            "/api//x",
            "http://evil.example/api/x",
            "/api/x#y",
            "/api/x y",
            "/api/x\\y",
            "/api/x\n",
        ] {
            assert!(!forwardable(bad), "{bad:?}");
        }
    }
}
