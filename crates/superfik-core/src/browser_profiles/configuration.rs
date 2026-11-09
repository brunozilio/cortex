//! Values transported to the native browser. Passwords never belong here.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct Laboratory {
    pub enabled: bool,
    pub seed: u32,
    pub hardware_concurrency: Option<u16>,
    pub device_memory: Option<f64>,
    pub gpu: Option<GpuIdentity>,
    pub canvas: bool,
    pub audio: bool,
    pub fonts: Vec<String>,
    pub screen: Option<Screen>,
    pub webrtc: WebRtc,
    pub proxy: Option<Proxy>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct GpuIdentity {
    pub vendor: String,
    pub renderer: String,
    pub architecture: String,
    pub device: String,
    pub description: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Screen {
    pub width: u32,
    pub height: u32,
    pub scale: f32,
    pub depth: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum WebRtc {
    #[default]
    System,
    ProxyOnly,
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ProxyScheme {
    #[default]
    Http,
    Https,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Proxy {
    pub scheme: ProxyScheme,
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
}

impl Laboratory {
    pub fn validate(&self) -> Result<(), String> {
        if self.hardware_concurrency.is_some_and(|v| !(1..=1024).contains(&v)) {
            return Err("CPU must be between 1 and 1024".into());
        }
        if self.device_memory.is_some_and(|v| ![0.25, 0.5, 1., 2., 4., 8.].contains(&v)) {
            return Err("Memory must be 0.25, 0.5, 1, 2, 4 or 8 GiB".into());
        }
        if (self.canvas || self.audio) && self.seed == 0 {
            return Err("Canvas/audio require a nonzero stable seed".into());
        }
        if let Some(gpu) = &self.gpu {
            for s in [&gpu.vendor, &gpu.renderer, &gpu.architecture, &gpu.device, &gpu.description] {
                if s.len() > 512 || s.chars().any(char::is_control) {
                    return Err("GPU identity contains an invalid value".into());
                }
            }
            if gpu.vendor.is_empty() || gpu.renderer.is_empty() {
                return Err("GPU vendor and renderer are required".into());
            }
        }
        if self.fonts.len() > 128
            || self.fonts.iter().any(|s| s.trim().is_empty() || s.len() > 128 || s.chars().any(char::is_control))
        {
            return Err("Invalid local font list".into());
        }
        if let Some(s) = self.screen
            && (!(320..=16384).contains(&s.width)
                || !(240..=16384).contains(&s.height)
                || !s.scale.is_finite()
                || !(0.5..=4.).contains(&s.scale)
                || ![24, 30, 32].contains(&s.depth))
        {
            return Err("Invalid screen dimensions, scale or depth".into());
        }
        if let Some(p) = &self.proxy {
            p.validate()?;
        }
        Ok(())
    }
}

impl Proxy {
    pub fn validate(&self) -> Result<(), String> {
        if self.host.is_empty() {
            return Err("Proxy host is required".into());
        }
        url::Host::parse(&self.host).map_err(|_| "Invalid proxy host")?;
        if self.port == 0
            || self.host.chars().any(char::is_control)
            || self.username.as_ref().is_some_and(|s| s.len() > 256 || s.chars().any(char::is_control))
        {
            return Err("Invalid proxy port or username".into());
        }
        Ok(())
    }
}
