//! 主机选择器三入口归一化 (移植自 lib/selector.sh)
//!
//! 入口:
//!   1. @<path>      → SecureCRT 精确 (e.g. @aws/edge)
//!   2. <keyword>    → SecureCRT 模糊匹配 (文件名 / Hostname 子串)
//!   3. tmp          → 直接传 user/host/port/key

use crate::securecrt::{CrtParser, CrtSession};
use crate::{Error, Result};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct ResolvedSelector {
    pub display: String,
    pub host: String,
    pub user: String,
    pub port: u16,
    pub key: Option<PathBuf>,
    pub source: Source,
    pub ini_path: Option<PathBuf>,
    pub password_present: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    Crt,
    Tmp,
}

/// 解析 SecureCRT 入口 (@path 或 keyword)
pub fn resolve_crt(parser: &CrtParser, input: &str) -> Result<ResolvedSelector> {
    let ini = if let Some(_rest) = input.strip_prefix('@') {
        parser.resolve_at_path(input)?
    } else {
        let matches = parser.fuzzy_search(input)?;
        match matches.len() {
            0 => return Err(Error::Selector(format!("没找到匹配主机: '{input}'"))),
            1 => matches.into_iter().next().unwrap(),
            n => {
                let mut hint = String::from("多个候选, 请用 @<path>:\n");
                for ini in matches.iter().take(10) {
                    if let Some(rel) = parser.relative_path(ini) {
                        hint.push_str(&format!("  @{rel}\n"));
                    }
                }
                return Err(Error::Selector(format!("'{input}' 匹配 {n} 个: {hint}")));
            }
        }
    };

    let CrtSession {
        display: _,
        host,
        user,
        port,
        key,
        firewall: _,
        auth_type: _,
        password_present,
    } = parser.parse(&ini)?;

    let display = if let Some(prefix) = input.strip_prefix('@') {
        format!("@{prefix}")
    } else {
        parser
            .relative_path(&ini)
            .map(|r| format!("@{r}"))
            .unwrap_or_else(|| input.to_string())
    };

    Ok(ResolvedSelector {
        display,
        host,
        user,
        port,
        key,
        source: Source::Crt,
        ini_path: Some(ini),
        password_present,
    })
}

/// tmp 入口: 直接给 user/host/...
pub fn resolve_tmp(user: &str, host: &str, port: Option<u16>, key: Option<PathBuf>) -> ResolvedSelector {
    let port = port.unwrap_or(22);
    ResolvedSelector {
        display: format!("{user}@{host}:{port}"),
        host: host.into(),
        user: user.into(),
        port,
        key,
        source: Source::Tmp,
        ini_path: None,
        password_present: false,
    }
}

/// prod 主机判定 (基于 display 中是否含 prod_keywords)
pub fn is_prod(sel: &ResolvedSelector, prod_keywords: &[String]) -> bool {
    if sel.source == Source::Tmp {
        return false; // tmp 模式由 --prod 标志单独处理
    }
    prod_keywords.iter().any(|kw| sel.display.contains(kw))
}