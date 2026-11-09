//! The OpenAI plugins' audio over HTTP; what is sent and read is in
//! `superfik_plugins::providers::audio`.
use super::super::{ModelError, http::Response};
use super::ProviderClient;
use superfik_plugins::providers::audio::{
    self as plugin, Audio, RETRY_PAUSE_MS, RealtimeSession, SDP_LIMIT, TRANSCRIPT_LIMIT,
};

impl ProviderClient {
    pub fn transcribe(&self, audio: &Audio<'_>) -> Result<String, ModelError> {
        let provider = self.transport.provider;
        plugin::transcribes(provider)?;
        let _guard = self.transport.lock.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let config = self.transport.config(false)?;
        let upload = plugin::transcription(provider, &config, audio)?;
        let mut attempt = 0;
        let response = loop {
            let sent = self.transport.http.post_bytes(&upload.url, &upload.headers, &upload.body);
            let status = sent.as_ref().ok().map(|r| r.status);
            if plugin::retry_transcription(attempt, upload.attempts, status) {
                attempt += 1;
                std::thread::sleep(std::time::Duration::from_millis(RETRY_PAUSE_MS));
                continue;
            }
            break sent.map_err(ModelError::Network)?;
        };
        let mitigated = response.header("cf-mitigated");
        let content_type = response.header("content-type");
        if let Some(refused) =
            plugin::refused_transcription(provider, response.status, mitigated.as_deref(), content_type.as_deref())
        {
            return Err(refused.into());
        }
        Ok(plugin::transcript(&accepted(response, TRANSCRIPT_LIMIT)?)?)
    }

    pub fn realtime_call(&self, offer: &str, session: &RealtimeSession) -> Result<String, ModelError> {
        plugin::realtime_offer(self.transport.provider, offer)?;
        let _guard = self.transport.lock.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let config = self.transport.config(false)?;
        let upload =
            plugin::realtime(self.transport.provider, &config, offer, session, || uuid::Uuid::new_v4().to_string())?;
        let response =
            self.transport.http.post_bytes(&upload.url, &upload.headers, &upload.body).map_err(ModelError::Network)?;
        Ok(plugin::sdp_answer(accepted(response, SDP_LIMIT)?)?)
    }
}

fn accepted(response: Response, limit: u64) -> Result<String, ModelError> {
    super::transport::status(response.status)?;
    super::transport::read(response, limit)
}
