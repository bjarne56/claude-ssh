//! SecureCRT .ini 解析器
//!
//! 复用 lib/crt.sh 的逻辑:
//! - Hostname / Username / Port (hex+decimal) / Identity / PublicKey
//! - SSH2.ini 全局 Identity / PublicKey 回退
//! - ${VDS_CONFIG_PATH} 路径变量展开
//! - path_mappings (Windows → macOS)
//! - 协议过滤 (仅 SSH2)
//! - .ppk 检测报错

use crate::{Error, Result};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default)]
pub struct CrtSession {
    pub display: String,           // 显示名 (path 或 keyword)
    pub hostname: String,
    pub username: Option<String>,
    pub port: u16,
    pub identity: Option<PathBuf>, // 私钥路径
    pub firewall: Option<String>,
    pub auth_type: AuthType,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum AuthType {
    #[default]
    Key,
    Password,
}

#[derive(Debug, Clone)]
pub struct CrtParser {
    pub config_path: PathBuf,
    pub path_mappings: Vec<(String, String)>,
    pub global_identity: Option<PathBuf>,
}

impl CrtParser {
    pub fn new(config_path: PathBuf, path_mappings: Vec<(String, String)>) -> Self {
        let global = parse_global_ssh2(&config_path).ok().flatten();
        Self {
            config_path,
            path_mappings,
            global_identity: global,
        }
    }

    /// 用 path (相对 Sessions/) 或 keyword 查找 session
    pub fn find_by_path(&self, _selector: &str) -> Result<CrtSession> {
        // TODO: 实现完整解析 (复用 lib/crt.sh 逻辑)
        Err(Error::SecureCrt("not implemented yet".into()))
    }
}

fn parse_global_ssh2(_config_path: &Path) -> Result<Option<PathBuf>> {
    // TODO: 解析 SSH2.ini 拿全局 Identity
    Ok(None)
}