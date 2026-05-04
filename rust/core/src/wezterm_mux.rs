//! WezTerm cli 封装 (Phase B 用 fork wezterm cli, Phase C 改 mux socket 直连)

use crate::{Error, Result};
use serde::Deserialize;
use std::process::{Command, Stdio};

#[derive(Debug, Clone, Deserialize)]
pub struct PaneEntry {
    pub window_id: u64,
    pub tab_id: u64,
    pub pane_id: u64,
    #[serde(default)]
    pub is_active: bool,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub cwd: String,
}

pub struct WezTermClient {
    cli_path: String,
}

impl WezTermClient {
    pub fn new(cli_path: Option<String>) -> Self {
        Self {
            cli_path: cli_path.unwrap_or_else(|| "wezterm".to_string()),
        }
    }

    /// `wezterm cli list --format json`
    pub fn list(&self) -> Result<Vec<PaneEntry>> {
        let out = Command::new(&self.cli_path)
            .args(["cli", "list", "--format", "json"])
            .output()?;
        if !out.status.success() {
            return Err(Error::WezTerm(
                String::from_utf8_lossy(&out.stderr).into_owned(),
            ));
        }
        Ok(serde_json::from_slice(&out.stdout)?)
    }

    /// 检查 wezterm cli 是否可用; macOS 不可用时尝试 `open -a WezTerm` + 等 5s
    /// 返回 (just_started, initial_panes_at_launch):
    /// - just_started=true 时, initial_panes 是 wezterm 自启的默认窗口列表
    ///   pane_open 用完后应清理这些 default 窗口, 避免视觉残留
    /// - 已在跑则 just_started=false, initial_panes 空
    pub fn ensure_running(&self) -> Result<(bool, Vec<PaneEntry>)> {
        if let Ok(_) = self.list() {
            return Ok((false, Vec::new()));
        }
        if cfg!(target_os = "macos") {
            let _ = Command::new("open").args(["-a", "WezTerm"]).output();
        }
        for _ in 0..25 {
            if let Ok(panes) = self.list() {
                return Ok((true, panes));
            }
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
        Err(Error::WezTerm("WezTerm 启动失败".into()))
    }

    /// `wezterm cli send-text --pane-id <id> --no-paste -- <text>` (text 含 \r 末尾触发执行)
    pub fn send_text(&self, pane_id: u64, text: &str) -> Result<()> {
        let mut child = Command::new(&self.cli_path)
            .args([
                "cli",
                "send-text",
                "--pane-id",
                &pane_id.to_string(),
                "--no-paste",
            ])
            .stdin(Stdio::piped())
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

    /// `wezterm cli spawn --cwd <dir> -- <argv...>` 在当前 window 新开 tab
    pub fn spawn_tab(&self, cwd: &str, argv: &[&str]) -> Result<u64> {
        let mut cmd = Command::new(&self.cli_path);
        cmd.args(["cli", "spawn", "--cwd", cwd, "--"]);
        for a in argv {
            cmd.arg(a);
        }
        let out = cmd.output()?;
        if !out.status.success() {
            return Err(Error::WezTerm(
                String::from_utf8_lossy(&out.stderr).into_owned(),
            ));
        }
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        s.parse::<u64>()
            .map_err(|e| Error::WezTerm(format!("parse pane id: {e} ({s})")))
    }

    /// 在指定 window 内 spawn tab (--window-id <id>)
    pub fn spawn_tab_in_window(&self, window_id: u64, cwd: &str, argv: &[&str]) -> Result<u64> {
        let mut cmd = Command::new(&self.cli_path);
        cmd.args([
            "cli",
            "spawn",
            "--window-id",
            &window_id.to_string(),
            "--cwd",
            cwd,
            "--",
        ]);
        for a in argv {
            cmd.arg(a);
        }
        let out = cmd.output()?;
        if !out.status.success() {
            return Err(Error::WezTerm(
                String::from_utf8_lossy(&out.stderr).into_owned(),
            ));
        }
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        s.parse::<u64>()
            .map_err(|e| Error::WezTerm(format!("parse pane id: {e} ({s})")))
    }

    /// 检查 window_id 是否存在 (任何 pane 的 window_id 匹配即可)
    pub fn window_alive(&self, window_id: u64) -> bool {
        match self.list() {
            Ok(panes) => panes.iter().any(|p| p.window_id == window_id),
            Err(_) => false,
        }
    }

    /// `--new-window` 模式
    pub fn spawn_new_window(&self, cwd: &str, argv: &[&str]) -> Result<u64> {
        let mut cmd = Command::new(&self.cli_path);
        cmd.args(["cli", "spawn", "--new-window", "--cwd", cwd, "--"]);
        for a in argv {
            cmd.arg(a);
        }
        let out = cmd.output()?;
        if !out.status.success() {
            return Err(Error::WezTerm(
                String::from_utf8_lossy(&out.stderr).into_owned(),
            ));
        }
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        s.parse::<u64>()
            .map_err(|e| Error::WezTerm(format!("parse pane id: {e} ({s})")))
    }

    /// `wezterm cli get-text --pane-id <id> [--start-line <n>]`
    /// scrollback=None 时只取当前屏幕 (用于 prompt 检测)
    pub fn get_text(&self, pane_id: u64, scrollback: Option<i64>) -> Result<String> {
        let mut cmd = Command::new(&self.cli_path);
        cmd.args(["cli", "get-text", "--pane-id", &pane_id.to_string()]);
        if let Some(s) = scrollback {
            cmd.args(["--start-line", &(-s).to_string()]);
        }
        let out = cmd.output()?;
        if !out.status.success() {
            return Err(Error::WezTerm(
                String::from_utf8_lossy(&out.stderr).into_owned(),
            ));
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }

    pub fn kill_pane(&self, pane_id: u64) -> Result<()> {
        let _ = Command::new(&self.cli_path)
            .args(["cli", "kill-pane", "--pane-id", &pane_id.to_string()])
            .output();
        Ok(())
    }

    pub fn set_tab_title(&self, pane_id: u64, title: &str) -> Result<()> {
        let panes = self.list()?;
        let tab_id = panes
            .iter()
            .find(|p| p.pane_id == pane_id)
            .map(|p| p.tab_id);
        if let Some(t) = tab_id {
            let _ = Command::new(&self.cli_path)
                .args(["cli", "set-tab-title", "--tab-id", &t.to_string(), title])
                .output();
        }
        Ok(())
    }

    pub fn pane_alive(&self, pane_id: u64) -> bool {
        match self.list() {
            Ok(panes) => panes.iter().any(|p| p.pane_id == pane_id),
            Err(_) => false,
        }
    }

    pub fn window_of_pane(&self, pane_id: u64) -> Option<u64> {
        self.list()
            .ok()?
            .into_iter()
            .find(|p| p.pane_id == pane_id)
            .map(|p| p.window_id)
    }

    pub fn active_window(&self) -> Option<u64> {
        self.list()
            .ok()?
            .into_iter()
            .find(|p| p.is_active)
            .map(|p| p.window_id)
    }
}
