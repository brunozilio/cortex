use std::net::SocketAddr;

use futures_util::{SinkExt, StreamExt};
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tunnel::{Answer, Ask, Payload, forwardable};

const METHODS: [&str; 5] = ["GET", "POST", "PUT", "PATCH", "DELETE"];
const NOT_FORWARDED: u16 = 403;
const UNREACHABLE: u16 = 502;

#[derive(Debug, Clone)]
pub struct Local {
    address: SocketAddr,
    authorization: String,
    http: reqwest::Client,
}

pub type Answers = mpsc::Sender<Answer>;

impl Local {
    pub fn new(address: SocketAddr, token: &str) -> Self {
        let authorization = format!("Bearer {token}");
        let http = reqwest::Client::builder().no_proxy().build().unwrap_or_default();
        Self { address, authorization, http }
    }

    pub async fn fetch(&self, ask: Ask, answers: Answers) {
        let id = ask.id();
        let (method, path, body, content_type) = match ask {
            Ask::Fetch { method, path, body, content_type, .. } => (method, path, body, content_type),
            Ask::Events { path, .. } => ("GET".to_owned(), path, None, None),
            _ => return,
        };
        let Some(method) = METHODS.iter().find(|known| known.eq_ignore_ascii_case(&method)) else {
            return refuse(&answers, id, "That method is not forwarded.").await;
        };
        if !forwardable(&path) {
            return refuse(&answers, id, "Only the console's API is forwarded.").await;
        }
        let method = reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::GET);
        let mut request = self
            .http
            .request(method, format!("http://{}{path}", self.address))
            .header(AUTHORIZATION, &self.authorization);
        if let Some(kind) = content_type {
            request = request.header(CONTENT_TYPE, kind);
        }
        if let Some(body) = body {
            request = request.body(body);
        }
        let response = match request.send().await {
            Ok(response) => response,
            Err(error) => return unreachable(&answers, id, &error.to_string()).await,
        };
        let content_type = response.headers().get(CONTENT_TYPE).and_then(|kind| kind.to_str().ok()).map(str::to_owned);
        if answers.send(Answer::Head { id, status: response.status().as_u16(), content_type }).await.is_err() {
            return;
        }
        let mut body = response.bytes_stream();
        while let Some(chunk) = body.next().await {
            let answer = match chunk {
                Ok(bytes) => Answer::Data { id, data: Payload::bytes(&bytes) },
                Err(error) => {
                    let _ = answers.send(Answer::End { id, error: Some(error.to_string()) }).await;
                    return;
                }
            };
            if answers.send(answer).await.is_err() {
                return;
            }
        }
        let _ = answers.send(Answer::End { id, error: None }).await;
    }

    pub async fn socket(&self, id: u32, path: String, answers: Answers, mut typed: mpsc::Receiver<Payload>) {
        if !forwardable(&path) {
            return refuse(&answers, id, "Only the console's API is forwarded.").await;
        }
        let Ok(mut request) = format!("ws://{}{path}", self.address).into_client_request() else {
            return refuse(&answers, id, "That is not a socket address.").await;
        };
        if let Ok(authorization) = self.authorization.parse() {
            request.headers_mut().insert(AUTHORIZATION, authorization);
        }
        let socket = match tokio_tungstenite::connect_async(request).await {
            Ok((socket, _)) => socket,
            Err(tokio_tungstenite::tungstenite::Error::Http(response)) => {
                let _ = answers.send(Answer::Head { id, status: response.status().as_u16(), content_type: None }).await;
                let _ = answers.send(Answer::End { id, error: Some("the console refused the socket".into()) }).await;
                return;
            }
            Err(error) => return unreachable(&answers, id, &error.to_string()).await,
        };
        if answers.send(Answer::Head { id, status: 101, content_type: None }).await.is_err() {
            return;
        }
        let (mut sink, mut stream) = socket.split();
        loop {
            tokio::select! {
                message = stream.next() => {
                    let data = match message {
                        Some(Ok(Message::Text(text))) => Payload::Text(text.to_string()),
                        Some(Ok(Message::Binary(bytes))) => Payload::bytes(&bytes),
                        Some(Ok(Message::Close(_)) | Err(_)) | None => break,
                        Some(Ok(_)) => continue,
                    };
                    if answers.send(Answer::Data { id, data }).await.is_err() {
                        return;
                    }
                }
                payload = typed.recv() => {
                    let message = match payload {
                        Some(Payload::Text(text)) => Message::Text(text.into()),
                        Some(bytes) => Message::Binary(bytes.decoded().unwrap_or_default().into()),
                        None => break,
                    };
                    if sink.send(message).await.is_err() {
                        break;
                    }
                }
            }
        }
        let _ = sink.send(Message::Close(None)).await;
        let _ = answers.send(Answer::End { id, error: None }).await;
    }
}

async fn refuse(answers: &Answers, id: u32, why: &str) {
    let _ = answers.send(Answer::Head { id, status: NOT_FORWARDED, content_type: None }).await;
    let _ = answers.send(Answer::End { id, error: Some(why.to_owned()) }).await;
}

async fn unreachable(answers: &Answers, id: u32, why: &str) {
    let _ = answers.send(Answer::Head { id, status: UNREACHABLE, content_type: None }).await;
    let _ = answers.send(Answer::End { id, error: Some(why.to_owned()) }).await;
}
