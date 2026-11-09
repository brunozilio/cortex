//! Joining a team's server (`Effect::JoinServer`, ADR 0017): signs in over
//! HTTP on a thread (the first access with its code, or name and
//! password), keeps the address, name and sign-in token in the keychain
//! (`Superfik` / `server`), opens the socket and hands the connection to
//! the shell, which talks to it instead of this app's own server. At
//! launch a kept sign-in connects again by itself. Leaving forgets it and
//! ends the sign-in on the server.
//!
//! Each join (and the launch's resume) is numbered; only the latest one's
//! outcome is answered, and leaving drops the one on its way, so two joins
//! never race. The sign-in is kept only once the shell has switched to
//! that server (`Connected::keep`): the keychain holds the server the app
//! is connected to.

use std::sync::atomic::{AtomicU64, Ordering};

use crossbeam_channel::{Receiver, Sender};
use serde::{Deserialize, Serialize};
use superfik_backend::Wake;
use superfik_remote::RemoteBackend;

use crate::state::server_link::server_urls;
use crate::state::{JoinRequest, Joined};

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Kept {
    address: String,
    name: String,
    token: String,
    #[serde(default)]
    insecure: bool,
}

const ITEM: superfik_keychain::Item<'static> = superfik_keychain::Item { service: "Superfik", account: "server" };

fn kept() -> Option<Kept> {
    serde_json::from_str(&ITEM.get()?).ok()
}

pub struct Connected {
    remote: RemoteBackend,
    joined: Joined,
    sign_in: Option<Kept>,
}

impl Connected {
    pub fn split(self) -> (RemoteBackend, Joined, Option<KeepSignIn>) {
        (self.remote, self.joined, self.sign_in.map(KeepSignIn))
    }
}

pub struct KeepSignIn(Kept);

impl KeepSignIn {
    pub fn keep(self) {
        if let Err(error) = serde_json::to_string(&self.0)
            .map_err(|e| e.to_string())
            .and_then(|json| ITEM.set(&json).map_err(|e| e.to_string()))
        {
            tracing::warn!(%error, "could not keep the server's sign-in; it lasts until the app quits");
        }
    }
}

type Outcome = Result<Connected, String>;

struct Latest<T> {
    current: AtomicU64,
    sender: Sender<(u64, T)>,
    outcomes: Receiver<(u64, T)>,
}

impl<T> Latest<T> {
    fn new() -> Self {
        let (sender, outcomes) = crossbeam_channel::unbounded();
        Self { current: AtomicU64::new(0), sender, outcomes }
    }

    fn begin(&self) -> u64 {
        self.current.fetch_add(1, Ordering::SeqCst) + 1
    }

    fn cancel(&self) {
        self.begin();
    }

    fn sender(&self) -> Sender<(u64, T)> {
        self.sender.clone()
    }

    fn take(&self) -> Option<T> {
        let current = self.current.load(Ordering::SeqCst);
        self.outcomes.try_iter().find_map(|(id, outcome)| (id == current).then_some(outcome))
    }
}

pub struct Joiner {
    latest: Latest<Outcome>,
    wake: Wake,
}

impl std::fmt::Debug for Joiner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Joiner").finish_non_exhaustive()
    }
}

impl Joiner {
    pub fn new(wake: Wake) -> Self {
        Self { latest: Latest::new(), wake }
    }

    pub fn join(&self, request: JoinRequest) {
        let wake = self.wake.clone();
        self.spawn(move || {
            let (base, socket) =
                server_urls(&request.address, request.insecure).map_err(|_| "That address cannot be used.")?;
            let token = sign_in(&base, &request)?;
            let kept = Kept {
                address: request.address.trim().to_owned(),
                name: request.name.trim().to_lowercase(),
                token,
                insecure: request.insecure,
            };
            connect(&socket, kept, true, wake).map_err(|error| error.to_string())
        });
    }

    pub fn resume(&self) -> bool {
        let Some(kept) = kept() else { return false };
        let Ok((_, socket)) = server_urls(&kept.address, kept.insecure) else { return false };
        let wake = self.wake.clone();
        self.spawn(move || {
            let address = kept.address.clone();
            connect(&socket, kept, false, wake).map_err(|error| match error {
                superfik_remote::RemoteError::Refused => {
                    ITEM.delete();
                    format!("{address} no longer accepts this sign-in. Sign in again.")
                }
                other => other.to_string(),
            })
        });
        true
    }

    pub fn leave(&self) {
        self.latest.cancel();
        let Some(kept) = kept() else { return };
        ITEM.delete();
        let Ok((base, _)) = server_urls(&kept.address, kept.insecure) else { return };
        let spawned = std::thread::Builder::new().name("superfik-sign-out".to_owned()).spawn(move || {
            let cookie = format!("superfik_session={}", kept.token);
            if let Err(error) = ureq::delete(&format!("{base}/auth/session")).header("Cookie", &cookie).call() {
                tracing::warn!(%error, "could not end the sign-in on the server");
            }
        });
        if let Err(error) = spawned {
            tracing::warn!(%error, "could not end the sign-in on the server");
        }
    }

    pub fn finished(&self) -> Option<Outcome> {
        self.latest.take()
    }

    fn spawn(&self, work: impl FnOnce() -> Outcome + Send + 'static) {
        let id = self.latest.begin();
        let (sender, wake) = (self.latest.sender(), self.wake.clone());
        let spawned = std::thread::Builder::new().name("superfik-join".to_owned()).spawn(move || {
            let _ = sender.send((id, work()));
            wake();
        });
        if let Err(error) = spawned {
            let _ = self.latest.sender().send((id, Err(error.to_string())));
        }
    }
}

fn connect(socket: &str, kept: Kept, new: bool, wake: Wake) -> Result<Connected, superfik_remote::RemoteError> {
    let remote = RemoteBackend::connect(socket, &kept.token, wake)?;
    let joined = Joined { address: kept.address.clone(), name: kept.name.clone() };
    Ok(Connected { remote, joined, sign_in: new.then_some(kept) })
}

fn sign_in(base: &str, request: &JoinRequest) -> Result<String, String> {
    let (path, body) = body(request);
    let agent: ureq::Agent = ureq::Agent::config_builder().http_status_as_error(false).build().into();
    let mut response = agent.post(&format!("{base}{path}")).send_json(body).map_err(|error| error.to_string())?;
    let status = response.status().as_u16();
    let answer: serde_json::Value = response.body_mut().read_json().unwrap_or_default();
    answered(status, &answer)
}

fn body(request: &JoinRequest) -> (&'static str, serde_json::Value) {
    match &request.code {
        Some(code) => (
            "/auth/first-access",
            serde_json::json!({ "name": request.name, "code": code, "password": request.password }),
        ),
        None => ("/auth/sign-in", serde_json::json!({ "name": request.name, "password": request.password })),
    }
}

fn answered(status: u16, answer: &serde_json::Value) -> Result<String, String> {
    let said = |key: &str| answer.get(key).and_then(serde_json::Value::as_str).map(str::to_owned);
    match (status, said("token")) {
        (200..=299, Some(token)) => Ok(token),
        (404, _) => Err("No Superfik server answers at that address.".to_owned()),
        _ => Err(said("error").unwrap_or_else(|| format!("The server refused the sign-in ({status})."))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(code: Option<&str>) -> JoinRequest {
        JoinRequest {
            address: "superfik.acme.io".into(),
            name: "ana".into(),
            password: "correct horse".into(),
            code: code.map(str::to_owned),
            insecure: false,
        }
    }

    #[test]
    fn the_first_access_sends_the_code_and_later_ones_the_password() {
        let (path, sent) = body(&request(Some("ABCD-EFGH-JKMN")));
        assert_eq!(path, "/auth/first-access");
        assert_eq!(sent["code"], "ABCD-EFGH-JKMN");
        let (path, sent) = body(&request(None));
        assert_eq!(path, "/auth/sign-in");
        assert!(sent.get("code").is_none());
    }

    #[test]
    fn a_good_answer_gives_the_token_and_a_refusal_what_the_server_said() {
        assert_eq!(answered(200, &serde_json::json!({ "token": "t" })), Ok("t".to_owned()));
        let refused = serde_json::json!({ "error": "wrong username or password" });
        assert_eq!(answered(401, &refused), Err("wrong username or password".to_owned()));
        assert!(answered(404, &serde_json::Value::Null).is_err_and(|said| said.contains("No Superfik server")));
        assert!(answered(500, &serde_json::Value::Null).is_err_and(|said| said.contains("500")));
    }

    #[test]
    fn only_the_latest_join_is_answered() {
        let latest = Latest::new();
        let (resume, join) = (latest.begin(), latest.begin());
        latest.sender().send((join, "joined")).expect("send");
        latest.sender().send((resume, "resumed")).expect("send");
        assert_eq!(latest.take(), Some("joined"));
        assert_eq!(latest.take(), None, "the earlier one finished later and is dropped");
    }

    #[test]
    fn leaving_drops_a_join_on_its_way() {
        let latest = Latest::new();
        let join = latest.begin();
        latest.cancel();
        latest.sender().send((join, "joined")).expect("send");
        assert_eq!(latest.take(), None);
    }

    #[test]
    fn what_is_kept_reads_back() {
        let kept = Kept { address: "superfik.acme.io".into(), name: "ana".into(), token: "t".into(), insecure: true };
        let json = serde_json::to_string(&kept).expect("json");
        assert!(serde_json::from_str::<Kept>(&json).is_ok_and(|back| back == kept));
        let older = r#"{"address":"http://10.0.0.5","name":"ana","token":"t"}"#;
        assert!(serde_json::from_str::<Kept>(older).is_ok_and(|back| !back.insecure), "kept before the say-so");
    }
}
