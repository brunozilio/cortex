//! Audio: transcription uploads and realtime voice sessions.

use serde_json::json;
use std::sync::Arc;
use superfik_core::ModelProvider;
use superfik_core::provider::ProviderKey;
use superfik_native::model::http::ScriptedHttp;
use superfik_native::model::providers::ProviderClient;

use crate::{credentials, header};

#[test]
fn transcription_uses_the_correct_multipart_contract_and_accepts_silence() {
    use superfik_native::model::providers::Audio;
    let audio = Audio { data: b"audio fixture", filename: "audio", content_type: "audio/wav" };
    for provider in [ModelProvider::OpenAi, ModelProvider::ChatGptSubscription] {
        let http =
            Arc::new(ScriptedHttp::new([(200, "{\"text\":\"  words  \"}".into()), (200, "{\"text\":\"\"}".into())]));
        let client = ProviderClient::new(provider, http.clone(), Arc::new(ProviderKey::new(credentials(provider))));
        assert_eq!(client.transcribe(&audio).expect("transcribe"), "words");
        assert_eq!(client.transcribe(&audio).expect("silence"), "");
        let sent = &http.sent()[0];
        assert!(header(sent, "content-type").expect("type").starts_with("multipart/form-data; boundary="));
        assert!(sent.body.contains("filename=\"audio.wav\""));
        if provider == ModelProvider::OpenAi {
            assert_eq!(sent.url, "https://api.openai.com/v1/audio/transcriptions");
            assert!(sent.body.contains("gpt-4o-mini-transcribe"));
        } else {
            assert_eq!(sent.url, "https://chatgpt.com/backend-api/transcribe");
            assert!(!sent.body.contains("gpt-4o-mini-transcribe"));
            assert_eq!(header(sent, "chatgpt-account-id").as_deref(), Some("account-1"));
        }
    }
}

#[test]
fn realtime_returns_sdp_and_preserves_the_subscription_session_contract() {
    use superfik_native::model::providers::RealtimeSession;
    let http = Arc::new(ScriptedHttp::new([(200, "v=0\r\na=answer\r\n".into())]));
    let client = ProviderClient::new(
        ModelProvider::ChatGptSubscription,
        http.clone(),
        Arc::new(ProviderKey::new(credentials(ModelProvider::ChatGptSubscription))),
    );
    assert!(client.realtime_call("invalid", &RealtimeSession::default()).is_err());
    let answer = client.realtime_call("v=0\r\na=offer\r\n", &RealtimeSession::default()).expect("SDP");
    assert!(answer.starts_with("v=0"));
    let sent = &http.sent()[0];
    assert_eq!(sent.url, "https://chatgpt.com/realtime/wm?dcid=0");
    assert_eq!(header(sent, "oai-client-build-number").as_deref(), Some("8338682"));
    assert!(sent.body.contains("\"voice\":\"juniper\""));
    assert!(sent.body.contains("\"language_code\":\"pt-BR\""));
    assert!(!answer.contains("account-1"));
}

#[test]
fn transcription_upload_preserves_binary_audio_over_real_http() {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use superfik_native::model::http::UreqHttp;
    use superfik_native::model::providers::Audio;
    let listener = TcpListener::bind("127.0.0.1:0").expect("listen");
    let base = format!("http://{}", listener.local_addr().expect("address"));
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        stream.set_read_timeout(Some(std::time::Duration::from_secs(5))).expect("timeout");
        let mut raw = Vec::new();
        let header_end = loop {
            let mut chunk = [0; 1024];
            let read = stream.read(&mut chunk).expect("read headers");
            assert!(read > 0);
            raw.extend_from_slice(&chunk[..read]);
            if let Some(index) = raw.windows(4).position(|bytes| bytes == b"\r\n\r\n") {
                break index + 4;
            }
        };
        let headers = String::from_utf8_lossy(&raw[..header_end]);
        assert!(headers.starts_with("POST /audio/transcriptions "));
        let length = headers
            .lines()
            .find_map(|line| {
                line.split_once(':')
                    .filter(|(key, _)| key.eq_ignore_ascii_case("content-length"))
                    .and_then(|(_, value)| value.trim().parse::<usize>().ok())
            })
            .expect("content length");
        while raw.len() < header_end + length {
            let mut chunk = [0; 1024];
            let read = stream.read(&mut chunk).expect("read audio");
            assert!(read > 0);
            raw.extend_from_slice(&chunk[..read]);
        }
        assert!(raw[header_end..].windows(4).any(|bytes| bytes == [0, 255, 128, 1]));
        let response = b"{\"text\":\"binary ok\"}";
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            response.len()
        )
        .expect("headers");
        stream.write_all(response).expect("response");
    });
    let config = json!({"api_key":"sim-key","base_url":base});
    let client =
        ProviderClient::new(ModelProvider::OpenAi, Arc::new(UreqHttp), Arc::new(ProviderKey::new(config.to_string())));
    assert_eq!(
        client
            .transcribe(&Audio { data: &[0, 255, 128, 1], filename: "audio.wav", content_type: "audio/wav" })
            .expect("transcribe"),
        "binary ok"
    );
    server.join().expect("server");
}
