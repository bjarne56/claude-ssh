//! cast-recorder 启动 + 录像目录 + meta.json + commands.jsonl 管理
//!
//! 移植自 lib/recorder.sh — 录像布局完全兼容 bash 版:
//!   <log_dir>/<project_slug>/<sid>/  (log_dir 显式配置)
//!   <project>/.ssh-ops/recordings/<sid>/  (默认)
//!     stream.cast / stream.cast.sock / commands.jsonl / annotations.jsonl
//!     meta.json / .start_ts / .last_ai_byte

use crate::config::{expand_path, Config};
use crate::state::{project_id, project_slug};
use crate::{Error, Result};
use chrono::Utc;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandRecord {
    pub ts: String,
    pub actor: String,
    pub host: String,
    pub cmd: String,
    pub exit: i32,
    pub duration_ms: u64,
    pub cast_offset: f64,
    pub dangerous: bool,
    pub blocked: bool,
    pub nonce: String,
}

#[derive(Debug, Clone)]
pub struct Recorder {
    pub session_id: String,
    pub dir: PathBuf,
    pub cast_path: PathBuf,
    pub sock_path: PathBuf,
    pub meta_path: PathBuf,
    pub commands_path: PathBuf,
    pub last_ai_byte_path: PathBuf,
}

impl Recorder {
    /// session_id = host_slug-YYYYMMDD-HHMMSS-xxxxxx
    pub fn make_session_id(host: &str) -> String {
        let slug = host_slug(host);
        let stamp = Utc::now().format("%Y%m%d-%H%M%S").to_string();
        let mut buf = [0u8; 3];
        rand::thread_rng().fill_bytes(&mut buf);
        let short = format!("{:02x}{:02x}{:02x}", buf[0], buf[1], buf[2]);
        format!("{slug}-{stamp}-{short}")
    }

    /// 录像目录: log_dir 显式时 = log_dir/project_slug/sid; 否则 = project/.ssh-ops/recordings/sid
    pub fn dir_for(cfg: &Config, sid: &str) -> PathBuf {
        if let Some(log_dir) = cfg.log_dir.as_ref().filter(|s| !s.is_empty()) {
            expand_path(log_dir).join(project_slug()).join(sid)
        } else {
            PathBuf::from(project_id())
                .join(".ssh-ops/recordings")
                .join(sid)
        }
    }

    /// 创建录像目录 + meta.json + commands.jsonl + .start_ts
    pub fn init(
        cfg: &Config,
        sid: &str,
        selector: &str,
        host: &str,
        user: &str,
        auth_type: &str,
    ) -> Result<Self> {
        let dir = Self::dir_for(cfg, sid);
        std::fs::create_dir_all(&dir)?;
        let cast = dir.join("stream.cast");
        let sock = dir.join("stream.cast.sock");
        let meta = dir.join("meta.json");
        let commands = dir.join("commands.jsonl");
        let last_ai_byte = dir.join(".last_ai_byte");
        let _ = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&commands)?;
        let _ = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(dir.join("annotations.jsonl"))?;

        let now = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
        let meta_json = serde_json::json!({
            "session_id": sid,
            "project": project_slug(),
            "project_path": project_id(),
            "host_selector": selector,
            "host_resolved": host,
            "user": user,
            "auth_type": auth_type,
            "started_at": now,
            "ended_at": serde_json::Value::Null,
            "wezterm_pane_id": serde_json::Value::Null,
            "command_count": 0,
            "ai_command_count": 0,
            "human_command_count": 0,
            "dangerous_count": 0,
            "blocked_count": 0,
            "tags": [],
        });
        std::fs::write(&meta, serde_json::to_string_pretty(&meta_json)?)?;

        // .start_ts: unix sec.fraction
        let now_ts = chrono::Utc::now().timestamp_micros() as f64 / 1_000_000.0;
        std::fs::write(dir.join(".start_ts"), format!("{now_ts:.6}"))?;

        // 项目内模式自动维护 .gitignore
        if cfg.log_dir.as_deref().unwrap_or("").is_empty() {
            let _ = ensure_project_gitignore();
        }

        Ok(Self {
            session_id: sid.into(),
            dir,
            cast_path: cast,
            sock_path: sock,
            meta_path: meta,
            commands_path: commands,
            last_ai_byte_path: last_ai_byte,
        })
    }

    /// 已存在的录像目录构造 Recorder (复用 pane 时不调 init)
    pub fn open(cfg: &Config, sid: &str) -> Result<Self> {
        let dir = Self::dir_for(cfg, sid);
        if !dir.exists() {
            return Err(Error::NotFound(format!("session dir 不存在: {}", dir.display())));
        }
        Ok(Self {
            session_id: sid.into(),
            dir: dir.clone(),
            cast_path: dir.join("stream.cast"),
            sock_path: dir.join("stream.cast.sock"),
            meta_path: dir.join("meta.json"),
            commands_path: dir.join("commands.jsonl"),
            last_ai_byte_path: dir.join(".last_ai_byte"),
        })
    }

    /// 把 wezterm pane_id 写到 meta.json
    pub fn set_pane_id(&self, pane_id: u64) -> Result<()> {
        let mut v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&self.meta_path)?)?;
        v["wezterm_pane_id"] = serde_json::Value::from(pane_id);
        std::fs::write(&self.meta_path, serde_json::to_string_pretty(&v)?)?;
        Ok(())
    }

    pub fn cast_size(&self) -> u64 {
        std::fs::metadata(&self.cast_path)
            .map(|m| m.len())
            .unwrap_or(0)
    }

    /// 当前距离 cast 真正开始的秒数 (用于 commands.jsonl 的 cast_offset)
    pub fn cast_offset(&self) -> f64 {
        // 优先 cast 文件 header 的 timestamp
        let base = self
            .cast_header_timestamp()
            .or_else(|| self.start_ts_file())
            .unwrap_or(0.0);
        if base == 0.0 {
            return 0.0;
        }
        let now = chrono::Utc::now().timestamp_micros() as f64 / 1_000_000.0;
        ((now - base) * 1000.0).round() / 1000.0
    }

    fn cast_header_timestamp(&self) -> Option<f64> {
        let f = File::open(&self.cast_path).ok()?;
        let mut reader = std::io::BufReader::new(f);
        let mut line = String::new();
        use std::io::BufRead;
        reader.read_line(&mut line).ok()?;
        let v: serde_json::Value = serde_json::from_str(line.trim()).ok()?;
        v.get("timestamp").and_then(|t| t.as_f64())
    }

    fn start_ts_file(&self) -> Option<f64> {
        let s = std::fs::read_to_string(self.dir.join(".start_ts")).ok()?;
        s.trim().parse().ok()
    }

    /// 读 [start, end) 字节区间
    pub fn read_cast_range(&self, start: u64, end: u64) -> Result<Vec<u8>> {
        let mut f = File::open(&self.cast_path)?;
        use std::io::Seek;
        f.seek(std::io::SeekFrom::Start(start))?;
        let len = end.saturating_sub(start) as usize;
        let mut buf = vec![0u8; len];
        let n = f.read(&mut buf)?;
        buf.truncate(n);
        Ok(buf)
    }

    /// 等 cast 文件大小稳定: 连续 3 次相同 (asciinema flush 完毕), 最多 3s
    pub fn wait_cast_stable(&self, min_bytes_after: u64) {
        let mut prev: i64 = -1;
        let mut same = 0;
        for _ in 0..30 {
            let cur = self.cast_size() as i64;
            if cur == prev && cur > min_bytes_after as i64 {
                same += 1;
                if same >= 3 {
                    break;
                }
            } else {
                same = 0;
            }
            prev = cur;
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }

    /// 提取 [start, end) 区间所有 'o' 事件的 data 拼接 (用于切片命令输出)
    pub fn extract_output(&self, start: u64, end: u64) -> Result<String> {
        let data = self.read_cast_range(start, end)?;
        let mut out = String::new();
        for line in data.split(|&b| b == b'\n') {
            let s = match std::str::from_utf8(line) {
                Ok(s) => s.trim(),
                Err(_) => continue,
            };
            if s.is_empty() {
                continue;
            }
            let v: serde_json::Value = match serde_json::from_str(s) {
                Ok(v) => v,
                Err(_) => continue,
            };
            let arr = match v.as_array() {
                Some(a) if a.len() >= 3 => a,
                _ => continue,
            };
            if arr[1].as_str() != Some("o") {
                continue;
            }
            if let Some(d) = arr[2].as_str() {
                out.push_str(d);
            }
        }
        Ok(out)
    }

    /// 追加一条命令记录到 commands.jsonl + 更新 meta.json 计数
    #[allow(clippy::too_many_arguments)]
    pub fn append_command(
        &self,
        actor: &str,
        host: &str,
        cmd: &str,
        exit: i32,
        duration_ms: u64,
        dangerous: bool,
        blocked: bool,
        nonce: &str,
    ) -> Result<()> {
        let now = Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();
        let offset = self.cast_offset();
        let rec = CommandRecord {
            ts: now,
            actor: actor.into(),
            host: host.into(),
            cmd: cmd.into(),
            exit,
            duration_ms,
            cast_offset: offset,
            dangerous,
            blocked,
            nonce: nonce.into(),
        };
        let line = serde_json::to_string(&rec)?;
        let mut f = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.commands_path)?;
        f.write_all(line.as_bytes())?;
        f.write_all(b"\n")?;

        // meta 计数更新
        let meta_str = std::fs::read_to_string(&self.meta_path)?;
        let mut meta: serde_json::Value = serde_json::from_str(&meta_str)?;
        let inc = |meta: &mut serde_json::Value, k: &str| {
            let v = meta.get(k).and_then(|n| n.as_u64()).unwrap_or(0) + 1;
            meta[k] = serde_json::Value::from(v);
        };
        inc(&mut meta, "command_count");
        if actor == "ai" {
            inc(&mut meta, "ai_command_count");
        }
        if actor == "human" {
            inc(&mut meta, "human_command_count");
        }
        if dangerous {
            inc(&mut meta, "dangerous_count");
        }
        if blocked {
            inc(&mut meta, "blocked_count");
        }
        std::fs::write(&self.meta_path, serde_json::to_string_pretty(&meta)?)?;
        Ok(())
    }

    pub fn finalize(&self) -> Result<()> {
        if !self.meta_path.exists() {
            return Ok(());
        }
        let meta_str = std::fs::read_to_string(&self.meta_path)?;
        let mut meta: serde_json::Value = serde_json::from_str(&meta_str)?;
        meta["ended_at"] = serde_json::Value::from(Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string());
        std::fs::write(&self.meta_path, serde_json::to_string_pretty(&meta)?)?;
        Ok(())
    }

    pub fn read_last_ai_byte(&self) -> u64 {
        std::fs::read_to_string(&self.last_ai_byte_path)
            .ok()
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0)
    }

    pub fn write_last_ai_byte(&self, byte: u64) -> Result<()> {
        std::fs::write(&self.last_ai_byte_path, byte.to_string())?;
        Ok(())
    }
}

/// host_slug: 转文件名安全, 截 32
fn host_slug(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .take(32)
        .collect()
}

/// 构造 cast-recorder argv: <bin> rec --quiet --stdin --command <ssh_cmd_string> <cast_path>
pub fn build_recorder_argv(
    sshops_home: &Path,
    cast_path: &Path,
    ssh_argv: &[String],
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

fn shlex_quote(s: &str) -> String {
    if !s.is_empty()
        && s.bytes().all(|b| {
            b.is_ascii_alphanumeric()
                || matches!(
                    b,
                    b'_' | b'-' | b'.' | b'/' | b'@' | b':' | b'=' | b',' | b'+'
                )
        })
    {
        s.to_string()
    } else {
        let escaped: String = s
            .chars()
            .map(|c| match c {
                '\'' => "'\\''".into(),
                other => other.to_string(),
            })
            .collect();
        format!("'{escaped}'")
    }
}

pub fn gen_nonce() -> String {
    let mut buf = [0u8; 8];
    rand::thread_rng().fill_bytes(&mut buf);
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

/// 项目内默认 .ssh-ops/recordings 模式时给 .gitignore 追加 .ssh-ops/
fn ensure_project_gitignore() -> Result<()> {
    let proj = project_id();
    let gi = Path::new(&proj).join(".gitignore");
    if !gi.exists() {
        return Ok(());
    }
    let content = std::fs::read_to_string(&gi).unwrap_or_default();
    if content.lines().any(|l| {
        let t = l.trim();
        t == ".ssh-ops/" || t == ".ssh-ops"
    }) {
        return Ok(());
    }
    let mut f = OpenOptions::new().append(true).open(&gi)?;
    f.write_all("\n# ssh-ops skill 录像与状态(自动维护)\n.ssh-ops/\n".as_bytes())?;
    Ok(())
}

/// 给 cast 文件路径计算对应 socket 路径
pub fn sock_path_for(cast: &Path) -> PathBuf {
    cast.with_file_name(format!(
        "{}.sock",
        cast.file_name().and_then(|n| n.to_str()).unwrap_or("stream")
    ))
}
