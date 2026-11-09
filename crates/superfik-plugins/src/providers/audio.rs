//! The OpenAI plugins' audio: transcription uploads and the ChatGPT
//! subscription's realtime call (an SDP offer for an SDP answer).
use super::{Config, ProviderError};
use serde_json::json;
use superfik_core::ModelProvider;

pub struct Audio<'a> {
    pub data: &'a [u8],
    pub filename: &'a str,
    pub content_type: &'a str,
}

impl std::fmt::Debug for Audio<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Audio")
            .field("bytes", &self.data.len())
            .field("content_type", &self.content_type)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Default)]
pub struct RealtimeSession {
    pub voice: String,
    pub language: String,
}

pub const TRANSCRIPT_LIMIT: u64 = 8 << 20;
pub const SDP_LIMIT: u64 = 512 << 10;
pub const RETRY_PAUSE_MS: u64 = 300;

pub struct Upload {
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    pub attempts: u32,
}

impl std::fmt::Debug for Upload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Upload").field("url", &self.url).field("bytes", &self.body.len()).finish_non_exhaustive()
    }
}

pub fn transcribes(provider: ModelProvider) -> Result<(), ProviderError> {
    if matches!(provider, ModelProvider::OpenAi | ModelProvider::ChatGptSubscription) {
        Ok(())
    } else {
        Err(ProviderError::Invalid("this provider does not support transcription".into()))
    }
}

pub fn transcription(provider: ModelProvider, config: &Config, audio: &Audio<'_>) -> Result<Upload, ProviderError> {
    transcribes(provider)?;
    let subscription = provider == ModelProvider::ChatGptSubscription;
    let mut multipart = Multipart::new();
    multipart.file(
        "file",
        filename(audio),
        if subscription { audio.content_type } else { "application/octet-stream" },
        audio.data,
    );
    if !subscription {
        multipart.field("model", "gpt-4o-mini-transcribe");
    }
    let (content_type, body) = multipart.finish();
    let mut headers = config.headers(provider);
    headers.retain(|(key, _)| !matches!(key.as_str(), "accept" | "openai-beta"));
    headers.extend([("content-type".into(), content_type), ("accept".into(), "application/json".into())]);
    if subscription {
        headers.push(("user-agent".into(), "codex_cli_rs".into()));
    }
    let path = if subscription { "/transcribe" } else { "/audio/transcriptions" };
    Ok(Upload {
        url: format!("{}{path}", config.base(provider)),
        headers,
        body,
        attempts: if subscription { 2 } else { 1 },
    })
}

pub fn retry_transcription(attempt: u32, attempts: u32, status: Option<u16>) -> bool {
    attempt + 1 < attempts && status.is_none_or(|s| s == 429 || s >= 500)
}

pub fn refused_transcription(
    provider: ModelProvider,
    status: u16,
    mitigated: Option<&str>,
    content_type: Option<&str>,
) -> Option<ProviderError> {
    if matches!(status, 400 | 413 | 415 | 422) {
        return Some(ProviderError::Invalid("invalid transcription audio".into()));
    }
    let challenged =
        mitigated == Some("challenge") || content_type.is_some_and(|s| s.to_ascii_lowercase().contains("text/html"));
    (provider == ModelProvider::ChatGptSubscription && status == 403 && challenged)
        .then(|| ProviderError::Invalid("subscription transcription unavailable".into()))
}

pub fn transcript(body: &str) -> Result<String, ProviderError> {
    let value: serde_json::Value =
        serde_json::from_str(body).map_err(|_| ProviderError::Invalid("invalid transcription response".into()))?;
    Ok(value["text"].as_str().unwrap_or_default().trim().to_owned())
}

fn filename<'a>(audio: &Audio<'a>) -> &'a str {
    if !audio.filename.trim().is_empty() && audio.filename != "audio" {
        return audio.filename;
    }
    match audio.content_type.split(';').next().unwrap_or_default().trim() {
        "audio/wav" | "audio/x-wav" => "audio.wav",
        "audio/mpeg" => "audio.mp3",
        "audio/mp4" => "audio.m4a",
        "audio/ogg" => "audio.ogg",
        "audio/flac" => "audio.flac",
        _ => "audio.webm",
    }
}

pub fn realtime(
    provider: ModelProvider,
    config: &Config,
    offer: &str,
    session: &RealtimeSession,
    mut ids: impl FnMut() -> String,
) -> Result<Upload, ProviderError> {
    realtime_offer(provider, offer)?;
    let token =
        config.auth.as_ref().ok_or_else(|| ProviderError::Unauthorized("subscription tokens missing".into()))?;
    let voice = if session.voice.trim().is_empty() { "juniper" } else { session.voice.trim() };
    let language = if session.language.trim().is_empty() { "pt-BR" } else { session.language.trim() };
    let id = ids();
    let payload = json!({"conversation_id":null,"language_code":language,"requested_default_model":"","model_slug":"","model_slug_advanced":"",
        "voice":voice,"voice_session_id":id,"voice_status_request_id":id,"timezone":"UTC","timezone_offset_min":0,"voice_mode":"wingman",
        "client_tools":[],"conversation_mode":{"kind":"primary_assistant"},"history_and_training_disabled":false,"enable_message_streaming":true,
        "backend_reasoning_effort":"high","chat_mode":"chat"});
    let mut multipart = Multipart::new();
    multipart.field("sdp", offer);
    multipart.field("session", &payload.to_string());
    let (content_type, body) = multipart.finish();
    let base = if config.base_url.is_empty() {
        "https://chatgpt.com".to_owned()
    } else {
        config.base_url.trim_end_matches('/').to_owned()
    };
    let headers = vec![
        ("content-type".into(), content_type),
        ("accept".into(), "application/sdp".into()),
        ("authorization".into(), format!("Bearer {}", token.access_token)),
        ("chatgpt-account-id".into(), token.account_id.clone()),
        ("oai-language".into(), language.into()),
        ("oai-device-id".into(), ids()),
        ("oai-client-version".into(), "prod-e8f67765f24e1c6773b4b61b4310d5ff9297439a".into()),
        ("oai-client-build-number".into(), "8338682".into()),
        ("oai-session-id".into(), ids()),
        ("x-openai-target-path".into(), "/realtime/wm".into()),
        ("x-openai-target-route".into(), "/realtime/wm".into()),
        ("origin".into(), base.clone()),
        ("referer".into(), format!("{base}/")),
    ];
    Ok(Upload { url: format!("{base}/realtime/wm?dcid=0"), headers, body, attempts: 1 })
}

pub fn realtime_offer(provider: ModelProvider, offer: &str) -> Result<(), ProviderError> {
    if provider != ModelProvider::ChatGptSubscription {
        return Err(ProviderError::Invalid("realtime requires ChatGPT Subscription".into()));
    }
    if !offer.trim_start().starts_with("v=0") {
        return Err(ProviderError::Invalid("invalid SDP offer".into()));
    }
    Ok(())
}

pub fn sdp_answer(answer: String) -> Result<String, ProviderError> {
    if answer.trim_start().starts_with("v=0") {
        Ok(answer)
    } else {
        Err(ProviderError::Invalid("invalid SDP answer".into()))
    }
}

pub struct Multipart {
    boundary: String,
    body: Vec<u8>,
}

impl std::fmt::Debug for Multipart {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Multipart").field("bytes", &self.body.len()).finish_non_exhaustive()
    }
}

impl Default for Multipart {
    fn default() -> Self {
        Self::new()
    }
}

impl Multipart {
    pub fn new() -> Self {
        Self::with_boundary(uuid::Uuid::new_v4().simple().to_string())
    }

    pub fn with_boundary(boundary: String) -> Self {
        Self { boundary, body: Vec::new() }
    }

    pub fn field(&mut self, name: &str, value: &str) {
        self.body.extend_from_slice(
            format!("--{}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n", self.boundary)
                .as_bytes(),
        );
    }

    pub fn file(&mut self, name: &str, filename: &str, content_type: &str, data: &[u8]) {
        let filename = filename.replace(['\r', '\n'], "").replace('\\', "\\\\").replace('"', "\\\"");
        let content_type = if content_type.trim().is_empty() {
            "application/octet-stream".to_owned()
        } else {
            content_type.replace(['\r', '\n'], "")
        };
        self.body.extend_from_slice(
            format!(
                "--{}\r\nContent-Disposition: form-data; name=\"{name}\"; filename=\"{filename}\"\r\nContent-Type: {content_type}\r\n\r\n",
                self.boundary
            )
            .as_bytes(),
        );
        self.body.extend_from_slice(data);
        self.body.extend_from_slice(b"\r\n");
    }

    pub fn finish(mut self) -> (String, Vec<u8>) {
        self.body.extend_from_slice(format!("--{}--\r\n", self.boundary).as_bytes());
        (format!("multipart/form-data; boundary={}", self.boundary), self.body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_nameless_upload_is_named_by_its_type() {
        let audio = Audio { data: b"x", filename: "audio", content_type: "audio/mpeg; codecs=mp3" };
        assert_eq!(filename(&audio), "audio.mp3");
    }

    #[test]
    fn multipart_escapes_the_file_name() {
        let mut multipart = Multipart::with_boundary("b".into());
        multipart.file("file", "a\"b\r\n.wav", "", b"\x00\xff");
        let (content_type, body) = multipart.finish();
        assert_eq!(content_type, "multipart/form-data; boundary=b");
        let text = String::from_utf8_lossy(&body);
        assert!(text.contains("filename=\"a\\\"b.wav\"") && text.contains("application/octet-stream"));
    }

    #[test]
    fn only_the_subscription_retries_and_only_when_busy() {
        assert!(retry_transcription(0, 2, Some(503)) && retry_transcription(0, 2, None));
        assert!(!retry_transcription(0, 2, Some(200)) && !retry_transcription(1, 2, Some(503)));
        assert!(!retry_transcription(0, 1, Some(503)));
    }

    #[test]
    fn realtime_needs_an_sdp_offer_and_the_subscription() {
        let config = Config::default();
        let session = RealtimeSession::default();
        assert!(realtime(ModelProvider::OpenAi, &config, "v=0", &session, String::new).is_err());
        assert!(realtime(ModelProvider::ChatGptSubscription, &config, "hello", &session, String::new).is_err());
        assert!(sdp_answer("<html>".into()).is_err());
    }
}
