use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::Response;
use serde_json::{Value, json};

pub const ORG: &str = "local";
pub const USER: &str = "local";

pub async fn socket(upgrade: WebSocketUpgrade) -> Response {
    upgrade.on_upgrade(talk)
}

async fn talk(mut socket: WebSocket) {
    if send(&mut socket, json!({ "type": "ready", "orgId": ORG, "userId": USER })).await.is_err() {
        return;
    }
    while let Some(Ok(message)) = socket.recv().await {
        let Message::Text(text) = message else { continue };
        let Ok(frame) = serde_json::from_str::<Value>(&text) else { continue };
        if let Some(answer) = answer(&frame)
            && send(&mut socket, answer).await.is_err()
        {
            return;
        }
    }
}

fn answer(frame: &Value) -> Option<Value> {
    match frame["type"].as_str()? {
        "ping" => Some(json!({ "type": "pong" })),
        "subscribe" => Some(json!({ "type": "subscribed", "channelId": frame["channelId"] })),
        "unsubscribe" => Some(json!({ "type": "unsubscribed", "channelId": frame["channelId"] })),
        _ => None,
    }
}

async fn send(socket: &mut WebSocket, frame: Value) -> Result<(), axum::Error> {
    socket.send(Message::Text(frame.to_string().into())).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_subscription_is_confirmed_for_its_channel_and_unknown_frames_get_no_answer() {
        assert_eq!(
            answer(&json!({ "type": "subscribe", "channelId": "c1" })),
            Some(json!({ "type": "subscribed", "channelId": "c1" }))
        );
        assert_eq!(
            answer(&json!({ "type": "unsubscribe", "channelId": "c1" })),
            Some(json!({ "type": "unsubscribed", "channelId": "c1" }))
        );
        assert_eq!(answer(&json!({ "type": "typing", "channelId": "c1" })), None);
        assert_eq!(answer(&json!({ "no": "type" })), None);
    }
}
