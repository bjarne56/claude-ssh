//! 危险命令检测 + prod 主机判定 + safety_gate
//!
//! 移植自 lib/safety.sh, 用 regex crate 跑 config.dangerous_patterns 列表

use crate::config::Config;
use regex::Regex;

#[derive(Debug, Clone)]
pub struct GateResult {
    pub dangerous: bool,
    pub blocked: bool,
    pub reason: String,
}

impl GateResult {
    pub fn safe() -> Self {
        Self {
            dangerous: false,
            blocked: false,
            reason: String::new(),
        }
    }
}

/// selector 视为 prod 的判定
/// - `__SSHOPS_TMP_PROD__` / `__SSHOPS_TMP_NONPROD__` 是临时参数标记
/// - `@<path>` 走子串匹配 prod_keywords
pub fn is_prod_selector(selector: &str, prod_keywords: &[String]) -> bool {
    if selector == "__SSHOPS_TMP_PROD__" {
        return true;
    }
    if selector == "__SSHOPS_TMP_NONPROD__" {
        return false;
    }
    if let Some(path) = selector.strip_prefix('@') {
        return prod_keywords.iter().any(|k| !k.is_empty() && path.contains(k));
    }
    false
}

/// 命中 dangerous_patterns 任一即危险, 返回命中模式
pub fn match_dangerous(cmd: &str, patterns: &[String]) -> Option<String> {
    for pat in patterns {
        if pat.is_empty() {
            continue;
        }
        if let Ok(re) = Regex::new(pat) {
            if re.is_match(cmd) {
                return Some(pat.clone());
            }
        }
    }
    None
}

/// 危险 + prod + 无 --i-mean-it → blocked
/// 危险 + (非 prod 或 i_mean_it) → dangerous=true 但放行
/// 非危险 → safe
pub fn safety_gate(cmd: &str, selector: &str, i_mean_it: bool, cfg: &Config) -> GateResult {
    let pat = match match_dangerous(cmd, &cfg.dangerous_patterns) {
        Some(p) => p,
        None => return GateResult::safe(),
    };
    let prod = is_prod_selector(selector, &cfg.prod_keywords);
    if prod && !i_mean_it {
        return GateResult {
            dangerous: true,
            blocked: true,
            reason: format!("dangerous on prod blocked (pattern: {pat}); 加 --i-mean-it 才能执行"),
        };
    }
    GateResult {
        dangerous: true,
        blocked: false,
        reason: if prod {
            format!("dangerous on prod, forced by --i-mean-it (pattern: {pat})")
        } else {
            format!("dangerous on non-prod, allowed (pattern: {pat})")
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg_with(dangerous: Vec<&str>, prod: Vec<&str>) -> Config {
        Config {
            dangerous_patterns: dangerous.into_iter().map(String::from).collect(),
            prod_keywords: prod.into_iter().map(String::from).collect(),
            ..Config::default()
        }
    }

    #[test]
    fn dangerous_blocked_on_prod() {
        let cfg = cfg_with(vec![r"rm\s+-rf\s+/"], vec!["prod"]);
        let r = safety_gate("rm -rf /var", "@aws/prod/db", false, &cfg);
        assert!(r.blocked);
        assert!(r.dangerous);
    }

    #[test]
    fn dangerous_passed_with_i_mean_it() {
        let cfg = cfg_with(vec![r"rm\s+-rf\s+/"], vec!["prod"]);
        let r = safety_gate("rm -rf /var", "@aws/prod/db", true, &cfg);
        assert!(!r.blocked);
        assert!(r.dangerous);
    }

    #[test]
    fn dangerous_passed_on_nonprod() {
        let cfg = cfg_with(vec![r"rm\s+-rf\s+/"], vec!["prod"]);
        let r = safety_gate("rm -rf /var", "@dev/box", false, &cfg);
        assert!(!r.blocked);
        assert!(r.dangerous);
    }

    #[test]
    fn safe_passes() {
        let cfg = cfg_with(vec![r"rm\s+-rf\s+/"], vec!["prod"]);
        let r = safety_gate("uptime", "@aws/prod/db", false, &cfg);
        assert!(!r.blocked && !r.dangerous);
    }
}
