//! Typed profile fields, translated into the validated domain model on save.
use superfik_core::browser_profiles::{BrowserProfile, GpuIdentity, Proxy, ProxyScheme, Screen};
use superfik_ui::TextEdit;

pub use superfik_core::browser_profiles::ProfileField;

pub const ORDER: [ProfileField; 18] = {
    use ProfileField::*;
    [
        Name,
        Cpu,
        Memory,
        Fonts,
        Width,
        Height,
        Scale,
        Depth,
        GpuVendor,
        GpuRenderer,
        GpuArchitecture,
        GpuDevice,
        GpuDescription,
        Seed,
        ProxyHost,
        ProxyPort,
        ProxyUsername,
        ProxyPassword,
    ]
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileToggle {
    Enabled,
    Canvas,
    Audio,
    Https,
}

#[derive(Clone)]
pub struct Editor {
    pub original: BrowserProfile,
    pub focus: ProfileField,
    fields: Vec<TextEdit>,
}

impl std::fmt::Debug for Editor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Editor").field("id", &self.original.id).finish_non_exhaustive()
    }
}

impl Editor {
    pub fn new(profile: BrowserProfile) -> Self {
        let s = &profile.settings;
        let gpu = s.gpu.clone().unwrap_or_default();
        let proxy = s.proxy.as_ref();
        let values = [
            profile.name.clone(),
            s.hardware_concurrency.map(|v| v.to_string()).unwrap_or_default(),
            s.device_memory.map(|v| v.to_string()).unwrap_or_default(),
            s.seed.to_string(),
            gpu.vendor,
            gpu.renderer,
            gpu.architecture,
            gpu.device,
            gpu.description,
            s.fonts.join(", "),
            s.screen.map(|v| v.width.to_string()).unwrap_or_default(),
            s.screen.map(|v| v.height.to_string()).unwrap_or_default(),
            s.screen.map(|v| v.scale.to_string()).unwrap_or_default(),
            s.screen.map(|v| v.depth.to_string()).unwrap_or_default(),
            proxy.map(|v| v.host.clone()).unwrap_or_default(),
            proxy.map(|v| v.port.to_string()).unwrap_or_default(),
            proxy.and_then(|v| v.username.clone()).unwrap_or_default(),
            String::new(),
        ];
        Self { original: profile, focus: ProfileField::Name, fields: values.into_iter().map(TextEdit::new).collect() }
    }

    pub fn field(&self, field: ProfileField) -> &TextEdit {
        &self.fields[field as usize]
    }
    pub fn field_mut(&mut self, field: ProfileField) -> &mut TextEdit {
        &mut self.fields[field as usize]
    }
    fn text(&self, field: ProfileField) -> &str {
        self.field(field).text().trim()
    }

    pub fn toggle(&mut self, value: ProfileToggle) {
        let s = &mut self.original.settings;
        match value {
            ProfileToggle::Enabled => s.enabled = !s.enabled,
            ProfileToggle::Canvas => s.canvas = !s.canvas,
            ProfileToggle::Audio => s.audio = !s.audio,
            ProfileToggle::Https => {
                let p = s.proxy.get_or_insert(Proxy {
                    scheme: ProxyScheme::Http,
                    host: String::new(),
                    port: 8080,
                    username: None,
                });
                p.scheme = if p.scheme == ProxyScheme::Http { ProxyScheme::Https } else { ProxyScheme::Http };
            }
        }
    }

    pub fn profile(&self) -> Result<BrowserProfile, String> {
        use ProfileField::*;
        let mut profile = self.original.clone();
        profile.name = self.text(Name).into();
        let s = &mut profile.settings;
        let number = |field| self.text(field).parse::<u32>().map_err(|_| format!("Invalid {field:?}"));
        s.hardware_concurrency = if self.text(Cpu).is_empty() {
            None
        } else {
            Some(u16::try_from(number(Cpu)?).map_err(|_| "Invalid CPU")?)
        };
        s.device_memory = if self.text(Memory).is_empty() {
            None
        } else {
            Some(self.text(Memory).parse().map_err(|_| "Invalid memory")?)
        };
        s.seed = number(Seed)?;
        s.gpu = if [GpuVendor, GpuRenderer, GpuArchitecture, GpuDevice, GpuDescription]
            .iter()
            .all(|f| self.text(*f).is_empty())
        {
            None
        } else {
            Some(GpuIdentity {
                vendor: self.text(GpuVendor).into(),
                renderer: self.text(GpuRenderer).into(),
                architecture: self.text(GpuArchitecture).into(),
                device: self.text(GpuDevice).into(),
                description: self.text(GpuDescription).into(),
            })
        };
        s.fonts = self.text(Fonts).split(',').map(str::trim).filter(|v| !v.is_empty()).map(str::to_owned).collect();
        s.screen = if [Width, Height, Scale, Depth].iter().all(|f| self.text(*f).is_empty()) {
            None
        } else {
            Some(Screen {
                width: number(Width)?,
                height: number(Height)?,
                scale: if self.text(Scale).is_empty() {
                    1.
                } else {
                    self.text(Scale).parse().map_err(|_| "Invalid screen scale")?
                },
                depth: if self.text(Depth).is_empty() { 24 } else { number(Depth)? },
            })
        };
        if self.text(ProxyHost).is_empty()
            && [ProxyPort, ProxyUsername, ProxyPassword].iter().any(|f| !self.text(*f).is_empty())
        {
            return Err("Proxy host is required".into());
        }
        s.proxy = if self.text(ProxyHost).is_empty() {
            None
        } else {
            Some(Proxy {
                scheme: s.proxy.as_ref().map(|p| p.scheme).unwrap_or_default(),
                host: self.text(ProxyHost).into(),
                port: u16::try_from(number(ProxyPort)?).map_err(|_| "Invalid proxy port")?,
                username: (!self.text(ProxyUsername).is_empty()).then(|| self.text(ProxyUsername).into()),
            })
        };
        if !self.text(ProxyPassword).is_empty() && self.text(ProxyUsername).is_empty() {
            return Err("Proxy username is required when supplying a password".into());
        }
        s.validate()?;
        if profile.name.trim().is_empty() {
            return Err("Profile name is required".into());
        }
        Ok(profile)
    }

    pub fn secret_change(&self) -> Option<superfik_browser::profiles::SecretChange> {
        if self.original.settings.proxy.is_some() && self.text(ProfileField::ProxyHost).is_empty() {
            return Some(superfik_browser::profiles::SecretChange { id: self.original.id, password: None });
        }
        let password = self.field(ProfileField::ProxyPassword).text();
        (!password.is_empty())
            .then(|| superfik_browser::profiles::SecretChange { id: self.original.id, password: Some(password.into()) })
    }
}
