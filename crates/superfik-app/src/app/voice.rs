//! The voice agent's device on this computer: models under
//! `<app data>/models/voice`, fetched the first time the voice is turned on.

use superfik_backend::Wake;
use superfik_voice::VoiceDevice;

pub fn device(wake: Wake) -> Box<dyn VoiceDevice> {
    if std::env::var("SUPERFIK_VOICE").is_ok_and(|v| v == "off") {
        return Box::new(superfik_voice::NoVoice::default());
    }
    let root = superfik_backend::local_data_dir().unwrap_or_else(std::env::temp_dir).join("models").join("voice");
    Box::new(superfik_voice::native::LocalVoice::new(root, wake))
}
