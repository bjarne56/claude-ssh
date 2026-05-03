//! WezTerm mux socket 客户端
//!
//! 直接 connect ~/.local/share/wezterm/sock/<gui-pid> 用 wezterm 的 binary
//! protocol 通信, 替代 fork wezterm cli (省 ~50ms × N 次调用).
//!
//! Phase B 简化: 还是 fork wezterm cli (功能一致, 性能稍差), 后续优化为 mux 直连.

use crate::{Error, Result};
use std::process::Command;

pub struct WezTermClient {
    cli_path: String,
}

impl WezTermClient {
    pub fn new(cli_path: Option<String>) -> Self {
        Self {
            cli_path: cli_path.unwrap_or_else(|| "wezterm".to_string()),
        }
    }

    /// `wezterm cli list` JSON
    pub fn list(&self) -> Result<String> {
        let out = Command::new(&self.cli_path)
            .args(["cli", "list", "--format", "json"])
            .output()?;
        if !out.status.success() {
            return Err(Error::WezTerm(String::from_utf8_lossy(&out.stderr).into_owned()));
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }

    /// `wezterm cli send-text --pane-id <pane> <text>`
    pub fn send_text(&self, pane_id: u64, text: &str) -> Result<()> {
        let mut child = Command::new(&self.cli_path)
            .args(["cli", "send-text", "--pane-id", &pane_id.to_string(), "--no-paste"])
            .stdin(std::process::Stdio::piped())
            .spawn()?;
        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            stdin.write_all(text.as_bytes())?;
        }
        let status = child.wait()?;
        if !status.success() {
            return Err(Error::WezTerm("send-text failed".into()));
        }
        Ok(())
    }

    /// `wezterm cli spawn` 启动新 pane, 返回 pane id
    pub fn spawn_tab(&self, cwd: &str, argv: &[&str]) -> Result<u64> {
        let mut cmd = Command::new(&self.cli_path);
        cmd.args(["cli", "spawn", "--cwd", cwd, "--"]);
        for a in argv {
            cmd.arg(a);
        }
        let out = cmd.output()?;
        if !out.status.success() {
            return Err(Error::WezTerm(String::from_utf8_lossy(&out.stderr).into_owned()));
        }
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        s.parse::<u64>()
            .map_err(|e| Error::WezTerm(format!("parse pane id: {e} ({s})")))
    }

    /// 关闭 pane
    pub fn kill_pane(&self, pane_id: u64) -> Result<()> {
        let _ = Command::new(&self.cli_path)
            .args(["cli", "kill-pane", "--pane-id", &pane_id.to_string()])
            .output();
        Ok(())
    }
}