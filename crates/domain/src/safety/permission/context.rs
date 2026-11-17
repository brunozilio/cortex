use std::path::{Path, PathBuf};

use super::{Effect, Guard, Reason, Remembered};
use crate::PermissionMode;
use crate::permission_rules::PermissionRules;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Call<'a> {
    pub tool: &'a str,
    pub subject: &'a str,
    pub effects: &'a [Effect],
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Branches {
    pub current: Option<String>,
    pub default: Option<String>,
}

pub trait Surroundings {
    fn branches(&self) -> Branches {
        Branches::default()
    }

    fn address_concern(&self, _call: &Call<'_>) -> Option<String> {
        None
    }

    fn package_concern(&self, _call: &Call<'_>) -> Option<String> {
        None
    }

    fn real_path(&self, path: &Path) -> PathBuf {
        path.to_path_buf()
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct NothingAround;

impl Surroundings for NothingAround {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Protected {
    pub name: String,
    pub guard: Guard,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Bounds {
    pub roots: Vec<PathBuf>,
    pub readable: Vec<PathBuf>,
    pub protected: Vec<Protected>,
}

impl Bounds {
    pub fn of(roots: Vec<PathBuf>, readable: Vec<PathBuf>) -> Self {
        let protected = vec![
            Protected { name: ".git".to_owned(), guard: Guard::GitFolder },
            Protected { name: brand::DIR.to_owned(), guard: Guard::OwnConfiguration },
        ];
        Self { roots, readable, protected }
    }

    fn root_holding(&self, real: &Path) -> Option<&PathBuf> {
        self.roots.iter().find(|root| real.starts_with(root))
    }

    pub fn may_read(&self, real: &Path) -> bool {
        self.root_holding(real).is_some() || self.readable.iter().any(|folder| real.starts_with(folder))
    }

    pub fn guard(&self, real: &Path) -> Option<Guard> {
        let Some(root) = self.root_holding(real) else { return Some(Guard::OutsideTheProject) };
        let inside = real.strip_prefix(root).ok()?;
        inside.components().find_map(|part| {
            let name = part.as_os_str().to_string_lossy();
            self.protected.iter().find(|protected| name.eq_ignore_ascii_case(&protected.name)).map(|found| found.guard)
        })
    }
}

pub struct PolicyContext<'a> {
    pub mode: PermissionMode,
    pub ceiling: Option<PermissionMode>,
    pub rules: &'a PermissionRules,
    pub remembered: &'a Remembered,
    pub bounds: &'a Bounds,
    pub cwd: &'a str,
    pub home: Option<&'a str>,
    pub sandboxed: bool,
    pub sandbox_missing: Option<&'a str>,
    pub no_prompts: bool,
    pub surroundings: &'a dyn Surroundings,
}

impl std::fmt::Debug for PolicyContext<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PolicyContext")
            .field("mode", &self.mode)
            .field("ceiling", &self.ceiling)
            .field("sandboxed", &self.sandboxed)
            .field("no_prompts", &self.no_prompts)
            .finish_non_exhaustive()
    }
}

impl PolicyContext<'_> {
    pub fn mode(&self) -> PermissionMode {
        self.mode.capped(self.ceiling)
    }

    pub fn settled(&self, verdict: super::Verdict, tool: &str) -> super::Verdict {
        if self.no_prompts { verdict.without_the_user(tool) } else { verdict }
    }

    pub(super) fn concern(&self, call: &Call<'_>, risk: Option<String>, leaves: bool) -> Option<Reason> {
        risk.map(Reason::Risky)
            .or_else(|| self.surroundings.address_concern(call).map(Reason::Address))
            .or_else(|| leaves.then_some(Reason::LeavesTheSandbox))
            .or_else(|| self.surroundings.package_concern(call).map(Reason::Package))
    }
}
