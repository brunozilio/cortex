//! Isolated TLS fixture process; credentials and certificates are disposable.
use super::Error;
use serde::Deserialize;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

#[derive(Deserialize)]
pub struct Configuration {
    pub http_origin: String,
    pub https_origin: String,
    pub http_proxy: u16,
    pub https_proxy: u16,
    pub spki: String,
}

#[derive(Debug, Deserialize)]
pub struct Stats {
    pub origin: u64,
    pub direct: u64,
    pub authenticated: u64,
    pub denied: u64,
    pub connect: u64,
    pub udp: u64,
}

pub struct ProxyFixture {
    pub configuration: Configuration,
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
}
impl ProxyFixture {
    pub fn start(root: &Path) -> Result<Self, Error> {
        let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/native_smoke/tls_proxy_fixture.py");
        let mut child = Command::new("python3")
            .arg(script)
            .arg(root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;
        let input = child.stdin.take().ok_or("fixture stdin")?;
        let mut output = BufReader::new(child.stdout.take().ok_or("fixture stdout")?);
        let mut line = String::new();
        output.read_line(&mut line)?;
        let configuration = match serde_json::from_str(&line) {
            Ok(configuration) => configuration,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error.into());
            }
        };
        Ok(Self { configuration, child, input, output })
    }
    pub fn stats(&mut self) -> Result<Stats, Error> {
        writeln!(self.input, "stats")?;
        self.input.flush()?;
        let mut line = String::new();
        self.output.read_line(&mut line)?;
        Ok(serde_json::from_str(&line)?)
    }
}
impl Drop for ProxyFixture {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
