// Copyright 2026 Bruno Zilio
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Per-context fixed proxy policy. Credentials never enter preferences.
use crate::WeldError;

#[derive(Clone, PartialEq)]
pub struct NativeProxy {
    pub server: String,
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
}
impl std::fmt::Debug for NativeProxy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NativeProxy").field("server", &self.server).field("credentials", &"[redacted]").finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeScreen { pub width: u32, pub height: u32, pub scale: f32, pub depth: u32 }

#[cfg(feature = "cef-runtime")]
pub(super) fn apply(context: &cef::RequestContext, proxy: Option<&NativeProxy>, rtc: Option<&str>) -> Result<(), WeldError> {
    use cef::{ImplDictionaryValue, ImplPreferenceManager, ImplValue};
    let mut dict = cef::dictionary_value_create().ok_or_else(|| WeldError::BrowserOp("Cannot create proxy preference".into()))?;
    let put = |dict: &mut cef::DictionaryValue, key: &str, value: &str| {
        let key: cef::CefString = key.into();
        let value: cef::CefString = value.into();
        dict.set_string(Some(&key), Some(&value));
    };
    put(&mut dict, "mode", if proxy.is_some() { "fixed_servers" } else { "direct" });
    if let Some(proxy) = proxy {
        put(&mut dict, "server", &proxy.server);
        put(&mut dict, "bypass_list", "<-loopback>");
    }
    let set = |name: &str, value: &mut cef::Value| -> Result<(), WeldError> {
        let name: cef::CefString = name.into();
        let mut error: cef::CefString = "".into();
        if context.set_preference(Some(&name), Some(value), Some(&mut error)) == 0 {
            return Err(WeldError::BrowserOp(format!("Cannot configure browser network policy: {error}")));
        }
        Ok(())
    };
    let mut value = cef::value_create().ok_or_else(|| WeldError::BrowserOp("Cannot allocate network policy".into()))?;
    value.set_dictionary(Some(&mut dict));
    set("proxy", &mut value)?;
    let policy = if proxy.is_some() { "disable_non_proxied_udp" } else { rtc.unwrap_or("default") };
    let policy: cef::CefString = policy.into();
    value.set_string(Some(&policy));
    set("webrtc.ip_handling_policy", &mut value)
}
