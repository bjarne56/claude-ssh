//! SecureCRT .ini 解析器 (移植自 lib/crt.sh)
//!
//! 字段映射:
//!   S:"Hostname"             → host
//!   S:"Username"             → user
//!   D:"[SSH2] Port"          → port (8 位 hex 优先, 十进制兼容)
//!   S:"Identity Filename V2" → key (空时回退 SSH2.ini 全局)
//!   S:"PublicKey Filename V2"→ key (备选)
//!   S:"Firewall Name"        → 跳板机引用
//!   S:"Protocol Name"        → 仅 SSH2

use crate::config::Config;
use crate::{Error, Result};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default)]
pub struct CrtSession {
    pub display: String,
    pub host: String,
    pub user: String,
    pub port: u16,
    pub key: Option<PathBuf>,
    pub firewall: Option<String>,
    pub auth_type: AuthType,
    pub password_present: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum AuthType {
    #[default]
    Key,
    Password,
}

#[derive(Debug, Clone)]
pub struct CrtParser {
    sessions_dir: PathBuf,
    config_dir: PathBuf,
    path_mappings: Vec<(String, String)>,
    global_identity: Option<String>, // 来自 SSH2.ini, 已 expand
}

impl CrtParser {
    pub fn from_config(cfg: &Config) -> Result<Self> {
        let sd = cfg
            .securecrt
            .config_path
            .as_ref()
            .ok_or_else(|| Error::Config("securecrt.config_path 未配置".into()))?;
        let config_dir = expand_path(sd);
        // sessions 目录: <config_dir>/Sessions
        let sessions_dir = config_dir.join("Sessions");
        let path_mappings = cfg.securecrt.path_mappings.clone();
        let mut p = Self {
            sessions_dir,
            config_dir,
            path_mappings,
            global_identity: None,
        };
        p.global_identity = parse_global_ssh2(&p.config_dir);
        Ok(p)
    }

    /// @aws/edge → <Sessions>/aws/edge.ini
    pub fn resolve_at_path(&self, selector: &str) -> Result<PathBuf> {
        let rel = selector.strip_prefix('@').unwrap_or(selector);
        let ini = self.sessions_dir.join(format!("{rel}.ini"));
        if !ini.exists() {
            return Err(Error::SecureCrt(format!(
                "session 不存在: {}",
                ini.display()
            )));
        }
        Ok(ini)
    }

    /// 关键词模糊匹配 (文件名 或 Hostname 字段)
    pub fn fuzzy_search(&self, kw: &str) -> Result<Vec<PathBuf>> {
        if kw.is_empty() || !self.sessions_dir.is_dir() {
            return Ok(vec![]);
        }
        let mut found = Vec::new();
        let mut seen = std::collections::HashSet::new();
        walk_ini(&self.sessions_dir, &mut |path| {
            let name = path.file_stem().and_then(|n| n.to_str()).unwrap_or("");
            let matched = name.contains(kw) || {
                read_ini_field(path, "Hostname")
                    .map(|h| h.contains(kw))
                    .unwrap_or(false)
            };
            if matched && seen.insert(path.to_path_buf()) {
                found.push(path.to_path_buf());
            }
        });
        Ok(found)
    }

    /// 把绝对 .ini 路径转为相对 Sessions 的路径 (去 .ini)
    pub fn relative_path(&self, ini: &Path) -> Option<String> {
        ini.strip_prefix(&self.sessions_dir).ok().and_then(|rel| {
            let s = rel.to_string_lossy();
            s.strip_suffix(".ini").map(|x| x.to_string())
        })
    }

    /// 完整解析 .ini → CrtSession
    pub fn parse(&self, ini: &Path) -> Result<CrtSession> {
        if !ini.exists() {
            return Err(Error::SecureCrt(format!("ini 不存在: {}", ini.display())));
        }

        let protocol = read_ini_field(ini, "Protocol Name").unwrap_or_else(|| "SSH2".to_string());
        if protocol != "SSH2" {
            return Err(Error::SecureCrt(format!(
                "仅支持 SSH2 协议, 当前: {protocol}"
            )));
        }

        let host = read_ini_field(ini, "Hostname")
            .ok_or_else(|| Error::SecureCrt(format!("Hostname 字段空: {}", ini.display())))?;
        let user = read_ini_field(ini, "Username")
            .ok_or_else(|| Error::SecureCrt(format!("Username 字段空: {}", ini.display())))?;

        let port = read_ini_field(ini, "[SSH2] Port")
            .map(|raw| decode_port(&raw))
            .unwrap_or(22);

        // Identity: Sessions → 全局 SSH2.ini → PublicKey 字段
        let mut key_raw = read_ini_field(ini, "Identity Filename V2");
        if key_raw.is_none() {
            key_raw = self.global_identity.clone();
        }
        if key_raw.is_none() {
            key_raw = read_ini_field(ini, "PublicKey Filename V2");
        }
        let key = key_raw.map(|s| self.expand_vars(&s));

        // .ppk 检测
        if let Some(ref k) = key {
            if k.extension().and_then(|e| e.to_str()) == Some("ppk") {
                return Err(Error::SecureCrt(format!(
                    ".ppk 私钥不支持, 请用 puttygen 转 OpenSSH: {}",
                    k.display()
                )));
            }
        }

        let firewall = read_ini_field(ini, "Firewall Name").and_then(|f| {
            if f == "None" || f.is_empty() {
                None
            } else {
                Some(f)
            }
        });

        let password_present = read_ini_field(ini, "Password V2")
            .map(|p| !p.is_empty())
            .unwrap_or(false);
        let auth_type = if key.is_none() && password_present {
            AuthType::Password
        } else {
            AuthType::Key
        };

        Ok(CrtSession {
            display: self
                .relative_path(ini)
                .map(|s| format!("@{s}"))
                .unwrap_or_else(|| ini.display().to_string()),
            host,
            user,
            port,
            key,
            firewall,
            auth_type,
            password_present,
        })
    }

    /// 展开 ${VDS_CONFIG_PATH} + path_mappings (Windows → mac)
    fn expand_vars(&self, s: &str) -> PathBuf {
        let mut out = s.replace(
            "${VDS_CONFIG_PATH}",
            &self.config_dir.to_string_lossy(),
        );
        for (from, to) in &self.path_mappings {
            if out.starts_with(from) {
                let rest = &out[from.len()..];
                let rest = rest.replace('\\', "/");
                out = format!("{to}{rest}");
                break;
            }
        }
        expand_path(&out)
    }
}

/// Port 字段: 8 位 hex (SecureCRT 默认) 或十进制
fn decode_port(raw: &str) -> u16 {
    if raw.is_empty() {
        return 22;
    }
    if raw.len() == 8 && raw.chars().all(|c| c.is_ascii_hexdigit()) {
        if let Ok(p) = u32::from_str_radix(raw, 16) {
            return p as u16;
        }
    }
    raw.parse::<u16>().unwrap_or(22)
}

/// 读 .ini 中 S:/D:/B: 类型的字段值
fn read_ini_field(path: &Path, field: &str) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    let needle = format!("\"{field}\"=");
    for line in content.lines() {
        // 必须以 [SDB]:" 开头
        if line.len() < 3 || line.as_bytes()[1] != b':' || line.as_bytes()[2] != b'"' {
            continue;
        }
        if !matches!(line.as_bytes()[0], b'S' | b'D' | b'B') {
            continue;
        }
        if let Some(start) = line.find(&needle) {
            let val_start = start + needle.len();
            return Some(line[val_start..].to_string());
        }
    }
    None
}

/// 解析 SSH2.ini 取全局 Identity
fn parse_global_ssh2(config_dir: &Path) -> Option<String> {
    let p = config_dir.join("SSH2.ini");
    if !p.exists() {
        return None;
    }
    read_ini_field(&p, "Identity Filename V2")
}

/// 递归遍历目录找所有 .ini 文件
fn walk_ini(dir: &Path, visit: &mut dyn FnMut(&Path)) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk_ini(&p, visit);
            } else if p.extension().and_then(|x| x.to_str()) == Some("ini") {
                visit(&p);
            }
        }
    }
}

/// 展开 ~ + $HOME (基本路径展开, 不递归)
fn expand_path(s: &str) -> PathBuf {
    let mut s = s.to_string();
    if let Some(rest) = s.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            s = home.join(rest).to_string_lossy().into_owned();
        }
    } else if s == "~" {
        if let Some(home) = dirs::home_dir() {
            s = home.to_string_lossy().into_owned();
        }
    }
    PathBuf::from(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_port_hex() {
        assert_eq!(decode_port("0000ea76"), 60022);
    }
    #[test]
    fn decode_port_decimal() {
        assert_eq!(decode_port("22"), 22);
    }
    #[test]
    fn decode_port_empty() {
        assert_eq!(decode_port(""), 22);
    }
}
