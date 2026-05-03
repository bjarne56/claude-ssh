//! 用户配置加载 (config.json) — schema 对齐现有 bash 版

use crate::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    /// SecureCRT 配置目录 (含 Sessions/, SSH2.ini)
    pub securecrt_config_dir: Option<String>,

    /// SecureCRT Sessions 目录 (覆盖 config_dir/Sessions)
    pub securecrt_sessions_dir: Option<String>,

    /// Windows → mac 路径映射 (从 ini 字段里转)
    #[serde(default)]
    pub path_mappings: Vec<(String, String)>,

    /// 录像目录, 空则用 <project>/.ssh-ops/recordings
    #[serde(default)]
    pub log_dir: Option<String>,

    /// prod 主机判定关键词 (selector 或 host 含其一即视为 prod)
    #[serde(default)]
    pub prod_keywords: Vec<String>,

    /// 危险命令正则 (POSIX bash 兼容; rust 端用 regex crate)
    #[serde(default)]
    pub dangerous_patterns: Vec<String>,

    /// ssh 公共参数
    #[serde(default)]
    pub ssh_options_base: Vec<String>,
    #[serde(default)]
    pub ssh_options_control_master: Vec<String>,

    /// auto_sudo 配置
    #[serde(default = "default_true")]
    pub auto_sudo: bool,
    #[serde(default)]
    pub auto_sudo_skip_users: Vec<String>,

    /// wezterm 子配置
    #[serde(default)]
    pub wezterm: WezTermConfig,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WezTermConfig {
    /// wezterm cli 路径
    pub cli_path: Option<String>,
}

fn default_true() -> bool {
    true
}

/// 加载 SSHOPS_HOME/config.json (或 SSHOPS_CONFIG)
pub fn load(home: &Path) -> Result<Config> {
    let path = std::env::var("SSHOPS_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|_| home.join("config.json"));
    if !path.exists() {
        return Ok(Config::default());
    }
    let data = std::fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&data)?)
}

/// 展开 ~/ 与 ${VAR}
pub fn expand_path(s: &str) -> PathBuf {
    let mut out = s.to_string();
    if out == "~" {
        if let Some(h) = dirs::home_dir() {
            return h;
        }
    } else if let Some(rest) = out.strip_prefix("~/") {
        if let Some(h) = dirs::home_dir() {
            return h.join(rest);
        }
    }
    while let Some(start) = out.find("${") {
        if let Some(end_rel) = out[start + 2..].find('}') {
            let end = start + 2 + end_rel;
            let var = &out[start + 2..end];
            let val = std::env::var(var).unwrap_or_default();
            out.replace_range(start..=end, &val);
        } else {
            break;
        }
    }
    PathBuf::from(out)
}
