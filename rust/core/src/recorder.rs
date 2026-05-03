//! cast-recorder 启动 / cast 文件管理

use std::path::PathBuf;

pub fn build_recorder_argv(
    sshops_home: &std::path::Path,
    cast_path: &std::path::Path,
    ssh_argv: &[&str],
) -> Vec<String> {
    let recorder = sshops_home.join("bin/cast-recorder");
    let mut ssh_cmd = String::new();
    for (i, a) in ssh_argv.iter().enumerate() {
        if i > 0 {
            ssh_cmd.push(' ');
        }
        ssh_cmd.push_str(&shlex_quote(a));
    }
    vec![
        recorder.to_string_lossy().into_owned(),
        "rec".into(),
        "--quiet".into(),
        "--stdin".into(),
        "--command".into(),
        ssh_cmd,
        cast_path.to_string_lossy().into_owned(),
    ]
}

/// 给 cast 文件路径计算对应 socket 路径
pub fn sock_path_for(cast: &std::path::Path) -> PathBuf {
    let name = cast.file_name().and_then(|n| n.to_str()).unwrap_or("stream");
    cast.with_file_name(format!("{name}.sock"))
}

fn shlex_quote(s: &str) -> String {
    if s.bytes().all(|b| {
        b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.' | b'/' | b'@' | b':' | b'=' | b',' | b'+')
    }) {
        s.to_string()
    } else {
        let escaped: String = s.chars().map(|c| match c {
            '\'' => "'\\''".into(),
            other => other.to_string(),
        }).collect();
        format!("'{}'", escaped)
    }
}