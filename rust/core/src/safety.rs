//! 危险命令检测 + prod 主机判定

pub fn is_dangerous(cmd: &str) -> bool {
    const DANGEROUS_PATTERNS: &[&str] = &[
        "rm -rf",
        "shutdown",
        "reboot",
        "mkfs.",
        "dd ",
        "chmod -R 777",
        "> /dev/sd",
        "wipefs",
        "fdisk",
        ":(){",
    ];
    DANGEROUS_PATTERNS.iter().any(|p| cmd.contains(p))
}

pub fn is_prod_host(path: &str) -> bool {
    let lower = path.to_lowercase();
    lower.contains("/prod/") || lower.contains("prod_") || lower.starts_with("prod/")
}