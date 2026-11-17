use std::path::{Path, PathBuf};

use super::{Answer, Asker};

pub struct Process {
    pub program: PathBuf,
    pub model: Option<String>,
    pub trust: bool,
}

impl Asker for Process {
    fn ask(&self, dir: &Path, prompt: &str, mode: Option<&str>) -> Result<Answer, String> {
        let mut command = std::process::Command::new(&self.program);
        command.arg("-p").arg("--output-format").arg("json").arg("-C").arg(dir);
        if let Some(mode) = mode {
            command.arg("--mode").arg(mode);
        }
        if let Some(model) = &self.model {
            command.arg("--model").arg(model);
        }
        if self.trust {
            command.arg("--trust");
        }
        let output =
            command.arg("--").arg(prompt).stdin(std::process::Stdio::null()).output().map_err(|e| e.to_string())?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let result = stdout
            .lines()
            .rev()
            .find_map(|line| serde_json::from_str::<serde_json::Value>(line).ok().filter(|v| v["type"] == "result"))
            .ok_or_else(|| format!("no result: {}", String::from_utf8_lossy(&output.stderr).trim()))?;
        if result["is_error"] == true {
            return Err(result["error"].as_str().unwrap_or("the run failed").to_owned());
        }
        Ok(Answer {
            text: result["result"].as_str().unwrap_or_default().to_owned(),
            cost_usd: result["total_cost_usd"].as_f64().unwrap_or_default(),
        })
    }
}
