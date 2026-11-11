use sha2::{Digest, Sha256};

use crate::model::TurnRequest;

pub fn conversation_key(request: &TurnRequest) -> uuid::Uuid {
    let mut hash = Sha256::new();
    hash.update(request.model.as_bytes());
    hash.update([0]);
    hash.update(request.system.as_bytes());
    hash.update([0]);
    if let Some(first) = request.messages.first() {
        hash.update(format!("{:?}", first.blocks).as_bytes());
    }
    let digest = hash.finalize();
    let mut bytes = [0; 16];
    bytes.copy_from_slice(&digest[..16]);
    uuid::Builder::from_random_bytes(bytes).into_uuid()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcript::Message;
    use domain::Effort;

    fn request(messages: Vec<Message>) -> TurnRequest {
        TurnRequest {
            model: "m".into(),
            system: "Be brief.".into(),
            messages,
            tools: Vec::new(),
            effort: Effort::Medium,
            max_tokens: 1000,
        }
    }

    #[test]
    fn a_conversation_keeps_its_key_as_it_grows() {
        let first = request(vec![Message::user("Fix the bug")]);
        let later = request(vec![Message::user("Fix the bug"), Message::assistant(vec![]), Message::user("Thanks")]);
        assert_eq!(conversation_key(&first), conversation_key(&later));
    }

    #[test]
    fn another_conversation_or_system_gets_another_key() {
        let one = request(vec![Message::user("Fix the bug")]);
        let other = request(vec![Message::user("Write docs")]);
        let resystemed = TurnRequest { system: "Be long.".into(), ..one.clone() };
        assert_ne!(conversation_key(&one), conversation_key(&other));
        assert_ne!(conversation_key(&one), conversation_key(&resystemed));
    }
}
