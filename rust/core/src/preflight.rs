//! 依赖检测 — sshops 启动前 (cmd_run / cmd_open 等子命令入口) 调用,
//! 缺失关键依赖时给出明确报错 + 安装命令, 避免后续隐晦失败.

use std::path::Path;
use std::process::Command;

#[derive(Debug)]
pub struct Missing {
    pub name: &'static str,
    pub reason: String,
    pub install_hint: &'static str,
}

impl std::fmt::Display for Missing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "✗ {}: {}\n  → 安装: {}",
            self.name, self.reason, self.install_hint
        )
    }
}

/// 检查所有关键依赖. 缺则 Err 列出全部缺失项.
pub fn check_all(sshops_home: &Path) -> Result<(), Vec<Missing>> {
    let mut missing = Vec::new();

    // 1. WezTerm-SSH-cli (必需 — spawn pane / send-text / list, ssh-ops fork 部署后)
    if !cmd_exists("WezTerm-SSH-cli") {
        missing.push(Missing {
            name: "WezTerm-SSH-cli",
            reason: "命令不存在 (PATH 中找不到 WezTerm-SSH-cli)".into(),
            install_hint: "在仓库根跑: bash install.sh (会自动构建 + 部署 wezterm-src fork)",
        });
    }

    // 2. asciinema (必需 — cast 录像)
    let asciinema_bin = sshops_home.join("bin/asciinema");
    if !asciinema_bin.exists() {
        missing.push(Missing {
            name: "asciinema",
            reason: format!("{} 不存在", asciinema_bin.display()),
            install_hint: "brew install asciinema && ln -sf $(which asciinema) bin/asciinema",
        });
    } else if !is_executable(&asciinema_bin) {
        missing.push(Missing {
            name: "asciinema",
            reason: format!("{} 存在但不可执行", asciinema_bin.display()),
            install_hint: "chmod +x bin/asciinema",
        });
    }

    // 3. ssh (必需 — 远程连接). macOS 自带, 几乎不会缺.
    if !cmd_exists("ssh") {
        missing.push(Missing {
            name: "ssh",
            reason: "命令不存在".into(),
            install_hint: "(系统自带, 检查 PATH)",
        });
    }

    if missing.is_empty() {
        Ok(())
    } else {
        Err(missing)
    }
}

/// 检测 `<cmd> --version` 是否能跑通 (PATH 可达 + 可执行)
fn cmd_exists(name: &str) -> bool {
    which::which(name).is_ok()
}

fn is_executable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    p.metadata()
        .map(|m| m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

/// cli 入口便捷: 检测 + 失败时打印 + exit 非零
pub fn check_or_exit(sshops_home: &Path) {
    if let Err(missing) = check_all(sshops_home) {
        eprintln!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        eprintln!("  ssh-ops 缺少必需依赖, 无法继续:");
        eprintln!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        for m in &missing {
            eprintln!("{m}\n");
        }
        eprintln!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        eprintln!("  装好后重试. 全检: $SSHOPS_HOME/install.sh --check");
        eprintln!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        std::process::exit(2);
    }
}
