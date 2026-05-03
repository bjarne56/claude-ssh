//! sshops-rs: ssh-ops CLI (Phase B Rust 重写)
//!
//! 100% 兼容 bash 版的 CLI 接口和 JSON 输出.
//! 短命二进制, 每次 run 启动一次. Phase C 改为 daemon 客户端.

use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use serde_json::json;
use ssh_ops_core::{
    config, human_detect, pane,
    recorder::{gen_nonce, Recorder},
    safety::safety_gate,
    securecrt::CrtParser,
    selector::{resolve_crt, resolve_tmp, ResolvedSelector, Source},
    session::{execute, strip_ansi},
    sshops_home, state, state_dir,
    wezterm_mux::WezTermClient,
};
use std::path::PathBuf;
use std::time::Duration;

#[derive(Parser, Debug)]
#[command(version, about = "ssh-ops Rust 重写 (Phase B)")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// 在 pane 注入命令并等响应
    Run(CommonArgs),
    /// 仅 spawn pane, 不跑命令
    Open(CommonArgs),
    /// 关闭 pane
    Close(CommonArgs),
    /// 抓取 pane 当前可见文本 (strip ANSI)
    Peek(CommonArgs),
    /// 列出当前项目所有 pane
    ListPanes,
    /// 拿最近 N 秒 (基于 cast 字节区间) 的 human 活动
    Recent {
        #[command(flatten)]
        common: CommonArgs,
        #[arg(long, default_value_t = 60)]
        seconds: u64,
    },
}

/// 公共参数 (跟 bash 版 CLI 兼容)
#[derive(clap::Args, Debug, Clone)]
struct CommonArgs {
    /// SecureCRT 模式: @<path> 精确, <keyword> 模糊
    /// tmp 模式时省略, 用 --host/--user
    #[arg(value_name = "SELECTOR_OR_CMD")]
    args: Vec<String>,

    #[arg(long)]
    host: Option<String>,
    #[arg(long)]
    user: Option<String>,
    #[arg(long, default_value_t = 22)]
    port: u16,
    #[arg(long)]
    key: Option<PathBuf>,
    #[arg(long)]
    password: Option<String>,
    #[arg(long)]
    ask_password: bool,
    #[arg(long)]
    prod: bool,
    #[arg(long, default_value_t = false)]
    i_mean_it: bool,
    #[arg(long)]
    timeout: Option<u64>,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("SSHOPS_LOG")
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Run(c) => cmd_run(c),
        Cmd::Open(c) => cmd_open(c),
        Cmd::Close(c) => cmd_close(c),
        Cmd::Peek(c) => cmd_peek(c),
        Cmd::ListPanes => cmd_list_panes(),
        Cmd::Recent { common, seconds } => cmd_recent(common, seconds),
    }
}

struct Resolved {
    sel: ResolvedSelector,
    cmd: Option<String>,
    safety_selector: String,
    auth_type: String,
    password: Option<String>,
}

/// 解析公共参数 + selector + cmd (need_cmd 决定是否 require)
fn resolve(common: &CommonArgs, need_cmd: bool, cfg: &config::Config) -> Result<Resolved> {
    // tmp 模式: --host + --user
    if let (Some(host), Some(user)) = (&common.host, &common.user) {
        let sel = resolve_tmp(user, host, Some(common.port), common.key.clone());
        let cmd = if need_cmd {
            if common.args.is_empty() {
                return Err(anyhow!("缺命令文本"));
            }
            Some(common.args.join(" "))
        } else {
            None
        };
        let safety_sel = if common.prod {
            "__SSHOPS_TMP_PROD__".into()
        } else {
            "__SSHOPS_TMP_NONPROD__".into()
        };
        let (auth_type, password) = if common.password.is_some() {
            ("password".to_string(), common.password.clone())
        } else if common.ask_password {
            let pw = read_password_tty(&format!(
                "密码 (host={host} user={user}): "
            ))?;
            ("password".into(), Some(pw))
        } else {
            ("key".into(), None)
        };
        return Ok(Resolved {
            sel,
            cmd,
            safety_selector: safety_sel,
            auth_type,
            password,
        });
    }

    // SecureCRT 模式: 第一个 arg 是 selector
    if common.args.is_empty() {
        return Err(anyhow!("缺 selector; 用法: sshops run @aws/edge \"uptime\""));
    }
    let selector_input = &common.args[0];
    let parser = CrtParser::from_config(cfg)?;
    let sel = resolve_crt(&parser, selector_input)?;

    // password 模式 + 无 key + 无 password 显式 → 提示用户在 pane 手输
    let mut password = common.password.clone();
    let mut auth_type = "key".to_string();
    if sel.password_present && sel.key.is_none() && password.is_none() {
        if let Ok(pw) = read_password_tty(&format!(
            "密码 (该主机 {} 在 SecureCRT 是密码登录): ",
            sel.host
        )) {
            password = Some(pw);
            auth_type = "password".into();
            tracing::info!("已从 tty 取得密码 → sshpass 自动喂入 ssh");
        } else {
            tracing::info!(
                "未传密码且无 tty, spawn pane 后请在 WezTerm 手输密码 (host={} user={} port={})",
                sel.host,
                sel.user,
                sel.port
            );
        }
    }

    let cmd = if need_cmd {
        if common.args.len() < 2 {
            return Err(anyhow!("缺命令文本"));
        }
        Some(common.args[1..].join(" "))
    } else {
        None
    };
    let safety_sel = sel.display.clone();
    Ok(Resolved {
        sel,
        cmd,
        safety_selector: safety_sel,
        auth_type,
        password,
    })
}

fn read_password_tty(prompt: &str) -> Result<String> {
    use std::io::{BufRead, Write};
    // /dev/tty 不可用时 (子进程无 tty), 失败让调用方 fallback
    let mut tty = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/tty")
        .map_err(|e| anyhow!("/dev/tty 不可用: {e}"))?;
    write!(tty, "{prompt}")?;
    tty.flush()?;
    // 简化: 不做 echo off (rpassword crate 才能干净禁回显; Phase C 再补)
    let mut line = String::new();
    let mut reader = std::io::BufReader::new(&tty);
    reader.read_line(&mut line)?;
    writeln!(tty)?;
    Ok(line.trim_end_matches(['\n', '\r']).to_string())
}

fn build_target(r: &Resolved) -> pane::SshTarget {
    pane::SshTarget {
        user: r.sel.user.clone(),
        host: r.sel.host.clone(),
        port: r.sel.port,
        key: r.sel.key.clone(),
        auth_type: r.auth_type.clone(),
        password: r.password.clone(),
    }
}

// ============================================================
// cmd_run
// ============================================================
fn cmd_run(common: CommonArgs) -> Result<()> {
    let home = sshops_home();
    let cfg = config::load(&home)?;
    let r = resolve(&common, true, &cfg)?;
    let cmd_text = r.cmd.clone().unwrap();

    // safety gate
    let gate = safety_gate(&cmd_text, &r.safety_selector, common.i_mean_it, &cfg);
    if gate.blocked {
        // 单独写一个录像目录留存 blocked 记录
        let sid = Recorder::make_session_id(&r.sel.host);
        let rec = Recorder::init(&cfg, &sid, &r.sel.display, &r.sel.host, &r.sel.user, &r.auth_type)?;
        let _ = rec.append_command("ai", &r.sel.display, &cmd_text, -1, 0, true, true, &gen_nonce());
        let _ = rec.finalize();
        let resp = json!({
            "exit": -1,
            "blocked": true,
            "dangerous": true,
            "reason": gate.reason,
            "selector": r.sel.display,
            "session_id": sid,
            "output": "(not executed)"
        });
        println!("{}", serde_json::to_string(&resp)?);
        std::process::exit(5);
    }

    // open / reuse pane
    let wez = WezTermClient::new(cfg.wezterm.cli_path.clone());
    let store = state::StateStore::new(&state_dir(&home))?;
    let target = build_target(&r);
    let opened = pane::pane_open(&cfg, &home, &wez, &store, &r.sel.display, &target)?;
    let pane_id = opened.pane_id;
    let recorder = opened.recorder;

    // recent_human_activity: 扫上次 ai run 之后到现在的 cast 区间
    let auto_human = std::env::var("SSHOPS_NO_AUTO_HUMAN").as_deref() != Ok("1");
    let mut recent_human: Vec<human_detect::HumanCmd> = Vec::new();
    if auto_human {
        let last_byte = recorder.read_last_ai_byte();
        let cur_byte = recorder.cast_size();
        if cur_byte > last_byte {
            let buf = recorder.read_cast_range(last_byte, cur_byte).unwrap_or_default();
            let header_ts = cast_header_timestamp(&recorder.cast_path).unwrap_or(0.0) as u64;
            recent_human = human_detect::extract_human_commands(&buf, header_ts);
            // 同步写入 commands.jsonl (actor=human)
            for h in &recent_human {
                let nonce = format!("human-{}", h.cast_offset);
                let _ = recorder.append_command(
                    "human",
                    &r.sel.display,
                    &h.cmd,
                    0,
                    0,
                    false,
                    false,
                    &nonce,
                );
            }
        }
    }

    // execute
    let timeout_secs = common.timeout.unwrap_or(30);
    let outcome = execute(&wez, pane_id, &recorder, &cmd_text, Duration::from_secs(timeout_secs))?;

    // 记录 ai 命令
    recorder.append_command(
        "ai",
        &r.sel.display,
        &cmd_text,
        outcome.exit,
        outcome.duration_ms,
        gate.dangerous,
        false,
        &gen_nonce(),
    )?;

    // 标记 last_ai_byte
    if auto_human {
        let _ = recorder.write_last_ai_byte(recorder.cast_size());
    }

    let resp = json!({
        "exit": outcome.exit,
        "duration_ms": outcome.duration_ms,
        "cast_offset": outcome.cast_offset,
        "selector": r.sel.display,
        "session_id": recorder.session_id,
        "dangerous": gate.dangerous,
        "blocked": false,
        "output": outcome.output,
        "recent_human_activity": recent_human,
    });
    println!("{}", serde_json::to_string(&resp)?);
    Ok(())
}

fn cast_header_timestamp(cast: &std::path::Path) -> Option<f64> {
    let f = std::fs::File::open(cast).ok()?;
    use std::io::BufRead;
    let mut reader = std::io::BufReader::new(f);
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    let v: serde_json::Value = serde_json::from_str(line.trim()).ok()?;
    v.get("timestamp").and_then(|t| t.as_f64())
}

// ============================================================
// cmd_open
// ============================================================
fn cmd_open(common: CommonArgs) -> Result<()> {
    let home = sshops_home();
    let cfg = config::load(&home)?;
    let r = resolve(&common, false, &cfg)?;
    let wez = WezTermClient::new(cfg.wezterm.cli_path.clone());
    let store = state::StateStore::new(&state_dir(&home))?;
    let target = build_target(&r);
    let opened = pane::pane_open(&cfg, &home, &wez, &store, &r.sel.display, &target)?;
    let resp = json!({
        "selector": r.sel.display,
        "source": match r.sel.source { Source::Crt => "crt", Source::Tmp => "tmp" },
        "pane_id": opened.pane_id,
        "session_id": opened.session_id,
        "user": r.sel.user,
        "host": r.sel.host,
        "port": r.sel.port,
        "key": r.sel.key.as_ref().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default(),
        "reused": opened.reused,
    });
    println!("{}", serde_json::to_string(&resp)?);
    Ok(())
}

// ============================================================
// cmd_close
// ============================================================
fn cmd_close(common: CommonArgs) -> Result<()> {
    let home = sshops_home();
    let cfg = config::load(&home)?;
    let r = resolve(&common, false, &cfg)?;
    let wez = WezTermClient::new(cfg.wezterm.cli_path.clone());
    let store = state::StateStore::new(&state_dir(&home))?;
    pane::pane_close(&cfg, &wez, &store, &r.sel.display)?;
    tracing::info!("closed: {}", r.sel.display);
    Ok(())
}

// ============================================================
// cmd_peek
// ============================================================
fn cmd_peek(common: CommonArgs) -> Result<()> {
    let home = sshops_home();
    let cfg = config::load(&home)?;
    let r = resolve(&common, false, &cfg)?;
    let store = state::StateStore::new(&state_dir(&home))?;
    let pid = state::project_id();
    let info = store
        .get_pane(&pid, &r.sel.display)?
        .ok_or_else(|| anyhow!("未找到 pane: {} (先 sshops open)", r.sel.display))?;
    let wez = WezTermClient::new(cfg.wezterm.cli_path.clone());
    if !wez.pane_alive(info.pane_id) {
        return Err(anyhow!("pane 已失效: {} (pane_id={})", r.sel.display, info.pane_id));
    }
    // 默认 50000 行 scrollback
    let raw = wez.get_text(info.pane_id, Some(50000))?;
    print!("{}", strip_ansi(&raw));
    Ok(())
}

// ============================================================
// cmd_list_panes
// ============================================================
fn cmd_list_panes() -> Result<()> {
    let home = sshops_home();
    let store = state::StateStore::new(&state_dir(&home))?;
    let pid = state::project_id();
    let proj = store.project_state(&pid)?.unwrap_or_default();
    let panes_obj: serde_json::Map<String, serde_json::Value> = proj
        .panes
        .iter()
        .map(|(s, info)| (s.clone(), serde_json::to_value(info).unwrap()))
        .collect();
    let resp = json!({
        "wezterm_window_id": proj.wezterm_window_id,
        "started_at": proj.started_at,
        "panes": panes_obj,
    });
    println!("{}", serde_json::to_string_pretty(&resp)?);
    Ok(())
}

// ============================================================
// cmd_recent: 主动查最近 N 秒的 human 活动 (不依赖上次 ai run 边界)
// ============================================================
fn cmd_recent(common: CommonArgs, seconds: u64) -> Result<()> {
    let home = sshops_home();
    let cfg = config::load(&home)?;
    let r = resolve(&common, false, &cfg)?;
    let store = state::StateStore::new(&state_dir(&home))?;
    let pid = state::project_id();
    let info = store
        .get_pane(&pid, &r.sel.display)?
        .ok_or_else(|| anyhow!("未找到 pane: {}", r.sel.display))?;
    let recorder = Recorder::open(&cfg, &info.session_id)?;

    let cast_size = recorder.cast_size();
    let buf = recorder.read_cast_range(0, cast_size).unwrap_or_default();
    let header_ts = cast_header_timestamp(&recorder.cast_path).unwrap_or(0.0) as u64;
    let all = human_detect::extract_human_commands(&buf, header_ts);
    let now = chrono::Utc::now().timestamp() as u64;
    let cutoff = now.saturating_sub(seconds);
    let recent: Vec<_> = all.into_iter().filter(|h| h.ts_unix >= cutoff).collect();
    println!("{}", serde_json::to_string(&recent)?);
    Ok(())
}
