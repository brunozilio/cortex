// Copyright 2026 Bruno Zilio
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Opt-in native preferences, applied before this context creates a renderer.

use crate::WeldError;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct NativeHardwareOverrides {
    pub enabled: bool,
    pub concurrency: Option<u16>,
    pub device_memory: Option<f64>,
    /// Validated laboratory settings. Never contains proxy credentials.
    pub configuration: Option<String>,
    pub screen: Option<super::NativeScreen>,
    pub proxy: Option<super::NativeProxy>,
    pub webrtc_policy: Option<String>,
}

impl NativeHardwareOverrides {
    pub fn validate(&self) -> Result<(), WeldError> {
        if self.concurrency.is_some_and(|value| !(1..=1024).contains(&value)) {
            return Err(WeldError::BrowserOp("hardware concurrency must be 1..=1024".into()));
        }
        if self.device_memory.is_some_and(|value| ![0.25, 0.5, 1.0, 2.0, 4.0, 8.0].contains(&value)) {
            return Err(WeldError::BrowserOp("unsupported device memory bucket".into()));
        }
        Ok(())
    }
}

#[cfg(feature = "cef-runtime")]
pub(super) fn apply(
    context: &cef::RequestContext,
    configuration: Option<&NativeHardwareOverrides>,
) -> Result<(), WeldError> {
    use cef::{ImplPreferenceManager, ImplValue};
    let enabled_name: cef::CefString = "superfik.identity.enabled".into();
    let defaults = NativeHardwareOverrides::default();
    let configuration = configuration.unwrap_or(&defaults);
    configuration.validate()?;
    let extended_name: cef::CefString = "superfik.identity.configuration".into();
    if configuration.configuration.is_some() && context.has_preference(Some(&extended_name)) == 0 {
        return Err(WeldError::BrowserOp("This profile requires the updated Superfik native engine".into()));
    }
    if context.has_preference(Some(&enabled_name)) == 0 {
        return if configuration.enabled {
            Err(WeldError::BrowserOp("native hardware profiles require Superfik's custom CEF".into()))
        } else {
            Ok(())
        };
    }
    let mut value =
        cef::value_create().ok_or_else(|| WeldError::BrowserOp("CEF could not allocate a preference value".into()))?;
    let set = |name: &str, value: &mut cef::Value| {
        let name: cef::CefString = name.into();
        // Default is a null CEF string; SetPreference requires a non-null
        // mutable output string even when no error is expected.
        let mut error: cef::CefString = "".into();
        if context.set_preference(Some(&name), Some(value), Some(&mut error)) == 0 {
            Err(WeldError::BrowserOp(format!("native profile preference failed: {error}")))
        } else {
            Ok(())
        }
    };
    value.set_bool(0);
    set("superfik.identity.enabled", &mut value)?;
    value.set_int(i32::from(configuration.concurrency.unwrap_or(0)));
    set("superfik.identity.hardware_concurrency", &mut value)?;
    value.set_double(configuration.device_memory.unwrap_or(0.0));
    set("superfik.identity.device_memory", &mut value)?;
    if context.has_preference(Some(&extended_name)) != 0 {
        let text: cef::CefString = configuration.configuration.as_deref().unwrap_or("").into();
        value.set_string(Some(&text));
        set("superfik.identity.configuration", &mut value)?;
    }
    value.set_bool(i32::from(configuration.enabled));
    set("superfik.identity.enabled", &mut value)?;
    super::native_network::apply(context, configuration.proxy.as_ref(), configuration.webrtc_policy.as_deref())
}
