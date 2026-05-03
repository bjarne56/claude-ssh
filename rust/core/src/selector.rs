//! 主机选择器解析: @host_or_path / 关键词 / 临时参数 → CrtSession

use crate::Result;

#[derive(Debug, Clone)]
pub enum Selector {
    /// 直连主机, 不查 SecureCRT (e.g. user@host:port)
    Direct {
        user: String,
        host: String,
        port: u16,
    },
    /// SecureCRT 路径选择 (@path 或 keyword)
    Crt(String),
}

pub fn parse(s: &str) -> Result<Selector> {
    if let Some(rest) = s.strip_prefix('@') {
        Ok(Selector::Crt(rest.to_string()))
    } else {
        // TODO: 解析 user@host:port
        Ok(Selector::Crt(s.to_string()))
    }
}