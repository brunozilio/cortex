#![allow(unsafe_code)]

use std::io;
use std::process::{Child, Command};

#[derive(Debug)]
pub struct Tree {
    child: Child,
}

impl Tree {
    pub fn spawn(command: &mut Command) -> io::Result<Self> {
        std::os::unix::process::CommandExt::process_group(command, 0);
        Ok(Self { child: command.spawn()? })
    }

    pub fn child(&mut self) -> &mut Child {
        &mut self.child
    }

    pub fn kill(&mut self) {
        if let Ok(group) = i32::try_from(self.child.id()) {
            unsafe { libc::kill(-group, libc::SIGKILL) };
        }
        let _ = self.child.kill();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::BufRead;
    use std::process::Stdio;
    use std::time::{Duration, Instant};

    fn alive(pid: i32) -> bool {
        unsafe { libc::kill(pid, 0) == 0 }
    }

    #[test]
    fn killing_the_tree_ends_what_the_root_started() -> io::Result<()> {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "sleep 30 & echo $!; wait"]).stdout(Stdio::piped());
        let mut tree = Tree::spawn(&mut command)?;
        let stdout = tree.child().stdout.take().ok_or_else(|| io::Error::other("no stdout"))?;
        let mut line = String::new();
        io::BufReader::new(stdout).read_line(&mut line)?;
        let grandchild: i32 = line.trim().parse().map_err(io::Error::other)?;
        assert!(alive(grandchild));

        tree.kill();
        tree.child().wait()?;
        let deadline = Instant::now() + Duration::from_secs(5);
        while alive(grandchild) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(!alive(grandchild), "the grandchild outlived the tree");
        Ok(())
    }
}
