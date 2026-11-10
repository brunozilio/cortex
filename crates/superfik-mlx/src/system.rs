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
        let Ok(client) = reqwest::blocking::Client::builder().timeout(Duration::from_secs(1)).build() else {
            return false;
        };
        client.get(format!("{}/v1/models", self.base_url)).send().is_ok_and(|r| r.status().as_u16() == 200)
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
