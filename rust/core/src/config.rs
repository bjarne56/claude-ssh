//! 用户配置加载 (~/.config/ssh-ops/config.json 或 项目 config.json)

use crate::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    /// 录像目录, 默认 <project>/.ssh-ops/recordings
    #[serde(default)]
    pub log_dir: Option<String>,

    /// SecureCRT 配置目录 (含 Sessions/)
    #[serde(default)]
    pub securecrt: SecureCrtConfig,

    /// wezterm cli 路径 (默认 wezterm)
    #[serde(default)]
    pub wezterm: WezTermConfig,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SecureCrtConfig {
    pub config_path: Option<String>,
    #[serde(default)]
    pub path_mappings: Vec<(String, String)>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WezTermConfig {
    /// wezterm cli 路径
    pub cli_path: Option<String>,
    /// wezterm mux socket 路径 (默认 ~/.local/share/wezterm/sock/...)
    pub mux_socket: Option<PathBuf>,
}

/// 加载项目根的 config.json (或 fallback 到默认值)
pub fn load(project_root: &std::path::Path) -> Result<Config> {
    let path = project_root.join("config.json");
    if !path.exists() {
        return Ok(Config::default());
    }
    let data = std::fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&data)?)
}