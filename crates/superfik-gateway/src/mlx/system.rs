//! The real [`Launcher`]: `mlx-lm` installed with `uv` into an environment
//! of the app's own, and `mlx_lm.server` started per model on a free
//! loopback port.

use std::ffi::OsStr;
use std::fs::File;
use std::io;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::Value;

use super::tethered::Tethered;
use super::{Clock, Launcher, Served};

#[derive(Debug, Clone)]
pub struct SystemLauncher {
    env: PathBuf,
    logs: PathBuf,
    tools: PathBuf,
    python: PathBuf,
}

const DOWNLOAD_TICK: Duration = Duration::from_millis(500);

impl SystemLauncher {
    pub fn new(root: &Path) -> Self {
        Self {
            env: root.join("runtime"),
            logs: root.join("logs"),
            tools: root.join("tools"),
            python: root.join("python"),
        }
    }

    fn server(&self) -> PathBuf {
        self.env.join("bin").join("mlx_lm.server")
    }
}

impl Launcher for SystemLauncher {
    fn is_installed(&self) -> bool {
        self.server().is_file()
    }

    fn install(&self) -> Result<(), String> {
        let uv = super::uv::ensure(&self.tools)?;
        let uv = |args: &[&str]| {
            let mut command = superfik_process::command(&uv);
            command.args(args).env("UV_PYTHON_INSTALL_DIR", &self.python);
            command
        };
        run(uv(&["venv", "--quiet", "--python", "3.12", "--python-preference", "only-managed"]).arg(&self.env))?;
        let python = self.env.join("bin").join("python");
        run(uv(&["pip", "install", "--quiet", "--python"]).arg(&python).arg("mlx-lm"))
    }

    fn download(&self, id: &str, dest: &Path, tick: &mut dyn FnMut()) -> Result<(), String> {
        std::fs::create_dir_all(&self.logs).map_err(|error| error.to_string())?;
        let log_path = self.logs.join("download.log");
        let log = File::create(&log_path).map_err(|error| error.to_string())?;
        let hf = self.env.join("bin").join("hf");
        let mut child = superfik_process::command(&hf)
            .args(["download", id, "--local-dir"])
            .arg(dest)
            .env("HF_HUB_DISABLE_PROGRESS_BARS", "1")
            .stdout(log.try_clone().map_err(|error| error.to_string())?)
            .stderr(log)
            .spawn()
            .map_err(|error| error.to_string())?;
        let status = loop {
            if let Some(status) = child.try_wait().map_err(|error| error.to_string())? {
                break status;
            }
            tick();
            std::thread::sleep(DOWNLOAD_TICK);
        };
        tick();
        if status.success() {
            return Ok(());
        }
        let printed = std::fs::read_to_string(&log_path).unwrap_or_default();
        let last = printed.lines().rev().find(|line| !line.trim().is_empty()).unwrap_or("hf download failed");
        Err(last.trim().to_owned())
    }

    fn size(&self, id: &str) -> Option<u64> {
        let agent: ureq::Agent =
            ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(10))).build().into();
        let url = format!("https://huggingface.co/api/models/{id}/tree/main?recursive=true");
        let listing = agent.get(url).call().ok()?.body_mut().read_to_string().ok()?;
        super::repository_size(&listing)
    }

    fn launch(&self, model_dir: &Path) -> io::Result<Box<dyn Served>> {
        std::fs::create_dir_all(&self.logs)?;
        let port = free_port()?;
        let log = File::create(self.logs.join("server.log"))?;
        let port_text = port.to_string();
        let sampling = sampling(model_dir);
        let mut args: Vec<&OsStr> = vec!["--model".as_ref(), model_dir.as_os_str()];
        args.extend(["--host", "127.0.0.1", "--port", &port_text].map(OsStr::new));
        args.extend(sampling.iter().map(OsStr::new));
        let child = Tethered::spawn(self.server().as_os_str(), &args, log)?;
        tracing::info!(model = %model_dir.display(), port, "started mlx_lm.server");
        Ok(Box::new(SystemServed { child, base_url: format!("http://127.0.0.1:{port}") }))
    }
}

fn sampling(model_dir: &Path) -> Vec<String> {
    let config = std::fs::read_to_string(model_dir.join("generation_config.json"))
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .unwrap_or_default();
    [("temperature", "--temp"), ("top_p", "--top-p"), ("top_k", "--top-k")]
        .into_iter()
        .filter_map(|(key, flag)| config.get(key).filter(|v| v.is_number()).map(|v| [flag.to_owned(), v.to_string()]))
        .flatten()
        .collect()
}

fn run(command: &mut std::process::Command) -> Result<(), String> {
    let output = command.output().map_err(|error| error.to_string())?;
    if output.status.success() {
        return Ok(());
    }
    Err(String::from_utf8_lossy(&output.stderr).trim().to_owned())
}

fn free_port() -> io::Result<u16> {
    Ok(TcpListener::bind("127.0.0.1:0")?.local_addr()?.port())
}

struct SystemServed {
    child: Tethered,
    base_url: String,
}

impl Served for SystemServed {
    fn base_url(&self) -> String {
        self.base_url.clone()
    }

    fn is_ready(&mut self) -> bool {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(1)))
            .http_status_as_error(false)
            .build()
            .into();
        agent.get(format!("{}/v1/models", self.base_url)).call().is_ok_and(|r| r.status() == 200)
    }

    fn is_running(&mut self) -> bool {
        self.child.is_running()
    }

    fn stop(&mut self) {
        self.child.stop();
    }
}

#[derive(Debug)]
pub struct SystemClock(Instant);

impl Default for SystemClock {
    fn default() -> Self {
        Self(Instant::now())
    }
}

impl Clock for SystemClock {
    fn now(&self) -> Duration {
        self.0.elapsed()
    }

    fn sleep(&self, by: Duration) {
        std::thread::sleep(by);
    }
}
