//! Read the conversation's opt-in native profile before creating a renderer.

use std::io::Read;
use std::path::Path;

use superfik_core::browser_identity::BrowserIdentity;
use welding::macos_cef::{NativeHardwareOverrides, NativeProxy, NativeScreen};

use crate::Profile;

pub(super) fn load(
    profile: &Profile,
    root: &Path,
    credentials: &dyn crate::profiles::CredentialStore,
) -> Result<Option<NativeHardwareOverrides>, String> {
    if let Profile::Persistent(conversation) = profile {
        let book = crate::profiles::ProfileStore::new(root.to_path_buf()).load()?;
        if book.uses_actual_values(conversation) {
            return Ok(None);
        }
        if let Some(profile) = book.for_conversation(conversation) {
            let settings = &profile.settings;
            let mut transported = settings.clone();
            transported.proxy = None;
            let proxy = if settings.enabled {
                settings
                    .proxy
                    .as_ref()
                    .map(|proxy| -> Result<NativeProxy, String> {
                        let password = if proxy.username.is_some() {
                            Some(credentials.read(profile.id)?.ok_or("Proxy password is missing from Keychain")?)
                        } else {
                            None
                        };
                        let scheme = match proxy.scheme {
                            superfik_core::browser_profiles::ProxyScheme::Http => "http",
                            superfik_core::browser_profiles::ProxyScheme::Https => "https",
                        };
                        Ok(NativeProxy {
                            server: format!("{scheme}://{}:{}", proxy.host, proxy.port),
                            host: proxy.host.clone(),
                            port: proxy.port,
                            username: proxy.username.clone(),
                            password,
                        })
                    })
                    .transpose()?
            } else {
                None
            };
            return if settings.enabled {
                Ok(Some(NativeHardwareOverrides {
                    enabled: true,
                    concurrency: settings.hardware_concurrency,
                    device_memory: settings.device_memory,
                    configuration: Some(serde_json::to_string(&transported).map_err(|e| e.to_string())?),
                    screen: settings.screen.map(|s| NativeScreen {
                        width: s.width,
                        height: s.height,
                        scale: s.scale,
                        depth: s.depth,
                    }),
                    webrtc_policy: Some(
                        match settings.webrtc {
                            superfik_core::browser_profiles::WebRtc::System => "default",
                            _ => "disable_non_proxied_udp",
                        }
                        .into(),
                    ),
                    proxy,
                }))
            } else {
                Ok(None)
            };
        }
    }
    let Some(directory) = profile.dir(root) else { return Ok(None) };
    let path = directory.join("superfik-identity.json");
    let file = match std::fs::File::open(&path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("Could not read native browser identity: {error}")),
    };
    let mut json = String::new();
    file.take(65_537).read_to_string(&mut json).map_err(|error| error.to_string())?;
    if json.len() > 65_536 {
        return Err("Native browser identity exceeds 64 KiB.".into());
    }
    let identity = BrowserIdentity::parse(&json)?;
    if !identity.enabled {
        return Ok(None);
    }
    Ok(Some(NativeHardwareOverrides {
        enabled: identity.enabled,
        concurrency: identity.hardware_concurrency,
        device_memory: identity.device_memory,
        configuration: None,
        ..NativeHardwareOverrides::default()
    }))
}

impl super::CefHost {
    pub(super) fn resolve_identity(&mut self, profile: &Profile) -> Result<Option<NativeHardwareOverrides>, String> {
        let configuration = load(profile, &self.root, self.profile_credentials.as_ref())?;
        if let Some(previous) = self.identities.get(profile) {
            if previous != &configuration {
                return Err(
                    "Native identity changed: restart Superfik before opening this conversation's browser.".into()
                );
            }
        } else {
            self.identities.insert(profile.clone(), configuration.clone());
        }
        Ok(configuration)
    }

    pub(super) fn fail_open(&mut self, id: superfik_core::BrowserId, url: String, error: String) {
        self.failures.insert(id, error.clone());
        self.pending_events.push((id, crate::PageEvent::Failed { url, error: error.clone() }));
        self.pending_events.push((id, crate::PageEvent::Loading(false)));
        if let Some(opening) = self.opening.remove(&id) {
            for call in opening.held {
                if let crate::Call::Agent { request, .. } = call {
                    self.pending_events.push((id, crate::PageEvent::Answered { request, answer: Err(error.clone()) }));
                }
            }
        }
    }
}
