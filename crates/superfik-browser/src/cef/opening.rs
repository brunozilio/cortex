//! Asynchronous profile initialization and browser creation.

use std::path::Path;

use superfik_core::BrowserId;
use welding::CefSurfaceConfig;
use welding::macos_cef::{MacosCefConfig, MacosCefProducer};

use super::page::Page;
use super::{CefHost, Opening, log};

impl CefHost {
    pub(super) fn open(&mut self, id: BrowserId, url: String, profile: &crate::Profile, size: (u32, u32), scale: f32) {
        self.failures.remove(&id);
        if self.egress.is_some()
            && let Err(refused) = crate::address::server_may_open(&url)
        {
            return self.fail_open(id, url, refused.to_string());
        }
        let native_hardware = match self.resolve_identity(profile) {
            Ok(identity) => identity,
            Err(error) => return self.fail_open(id, url, error),
        };
        if self.start().is_none() {
            let error = self.broken.clone().unwrap_or_else(|| "The browser engine is unavailable.".into());
            return self.fail_open(id, url, error);
        }
        let root = self.root.clone();
        let Some(runtime) = self.runtime.as_ref() else { return };
        let user_data_dir = profile.dir(&root);
        let key = user_data_dir
            .as_deref()
            .and_then(Path::file_name)
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| format!("private-{id}"));
        let files = match super::files::FileArea::prepare(root.join("files").join(key)) {
            Ok(files) => files,
            Err(error) => return self.fail_open(id, url, error),
        };
        if let Some(parent) = user_data_dir.as_deref().and_then(Path::parent) {
            log(std::fs::create_dir_all(parent));
        }
        let config = MacosCefConfig {
            native_hardware,
            surface: CefSurfaceConfig {
                initial_url: url,
                initial_size: size.into(),
                scale_factor: scale,
                user_data_dir,
                devtools_protocol: true,
                managed_popups: true,
                download_dir: Some(files.downloads()),
                ..CefSurfaceConfig::default()
            },
        };
        match MacosCefProducer::prepare_profile(runtime, &config) {
            Ok(profile) => {
                self.opening.insert(id, Opening { config, profile, held: Vec::new(), files });
            }
            Err(error) => self.fail_open(id, config.surface.initial_url.clone(), error.to_string()),
        }
    }

    pub(super) fn finish_opening(&mut self) {
        let ids: Vec<BrowserId> = self.opening.keys().copied().collect();
        for id in ids {
            let Some(opening) = self.opening.get_mut(&id) else { continue };
            let producer = match MacosCefProducer::try_new_with_prepared_profile(&opening.config, &mut opening.profile)
            {
                Ok(None) => continue,
                Ok(Some(producer)) => producer,
                Err(error) => {
                    tracing::error!(%error, "could not create the browser page");
                    let url = opening.config.surface.initial_url.clone();
                    self.fail_open(id, url, error.to_string());
                    continue;
                }
            };
            let size = (opening.config.surface.initial_size.width, opening.config.surface.initial_size.height);
            let Some(opening) = self.opening.remove(&id) else {
                continue;
            };
            self.pages.insert(id, Page::new(producer, size, opening.files).guarded(self.egress.is_some()));
            let held = opening.held;
            for call in held {
                self.apply(call);
            }
        }
    }
}
