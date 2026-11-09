//! Filesystem-derived prompt material, prepared under the tool worker identity.
use crate::{prompt, tools};
use serde::{Deserialize, Serialize};
use superfik_core::PromptRequest;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Brief {
    pub skills: Vec<tools::skill::FoundSkill>,
    pub agents: Vec<tools::subagent::FoundAgent>,
    pub settings: prompt::settings::Settings,
    pub rules: Vec<prompt::rules::Rule>,
    pub git: Option<prompt::GitSnapshot>,
    pub instructions: Vec<prompt::Instructions>,
    pub images: Vec<crate::image::Image>,
}
impl Brief {
    pub fn load(request: &PromptRequest) -> Result<Self, String> {
        let home = std::env::home_dir();
        let home = home.as_deref();
        let mut images = Vec::new();
        for reference in &request.references {
            if let Some(image) = crate::image::Image::load(&if reference.path.is_absolute() {
                reference.path.clone()
            } else {
                request.cwd.join(&reference.path)
            })? {
                images.push(image);
            }
        }
        Ok(Self {
            skills: tools::skill::discover(request, home),
            agents: tools::subagent::discover(request, home),
            settings: prompt::settings::load(&request.cwd, home)?,
            rules: prompt::rules::discover(&request.cwd, home),
            git: prompt::snapshot(&request.cwd),
            instructions: prompt::instructions(&request.cwd, home),
            images,
        })
    }
}
pub fn worker_main() -> Result<(), String> {
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).map_err(|e| e.to_string())?;
    let request: PromptRequest = serde_json::from_str(&input).map_err(|e| e.to_string())?;
    let brief = Brief::load(&request)?;
    serde_json::to_writer(std::io::stdout(), &brief).map_err(|e| e.to_string())
}
