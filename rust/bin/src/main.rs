//! sshops-rs: ssh-ops CLI (Phase B Rust 重写)
//!
//! 100% 兼容 bash 版的 CLI 接口和 JSON 输出.
//! 短命二进制, 每次 run 启动一次. Phase C 改为 daemon 客户端.

use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use serde_json::json;
use ssh_ops_core::{
    config, human_detect,
    ipc::{IpcRequest, IpcResponse},
    pane,
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

mod ipc_client;

#[derive(Parser, Debug)]
#[command(version, about = "ssh-ops Rust 重写 (Phase B/C)")]
struct Cli {
    /// 强制 in-process 模式 (跳过 daemon, 用于调试 / fallback)
    #[arg(long, global = true)]
    no_daemon: bool,
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
    /// 发送原始按键到 pane (不等 prompt 立即返回, 用于驱动远端交互式菜单)
    /// 例: sshops send my-host "1\\r"   (回车选择菜单选项 1)
    ///     sshops send my-host "yes\\n"
    ///     sshops send my-host --raw $'\x03'   (Ctrl-C 字面字节)
    Send {
        #[command(flatten)]
        common: CommonArgs,
        /// 字面发送, 不解释 \r \n \t 等 escape 序列
        #[arg(long)]
        raw: bool,
    },
    /// 查 daemon 状态 (不可达则报错)
    DaemonStatus,
    /// 优雅停 daemon
    DaemonStop,
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
    let no_daemon = cli.no_daemon;

    // 依赖 preflight: 需要 wezterm/asciinema 的子命令先验证依赖, 缺则友好报错 + exit 2
    // daemon-status / daemon-stop / list-panes 仅 IPC 不动 wezterm, 跳过
    let needs_deps = matches!(
        cli.cmd,
        Cmd::Run(_) | Cmd::Open(_) | Cmd::Close(_) | Cmd::Peek(_) | Cmd::Recent { .. } | Cmd::Send { .. }
    );
    if needs_deps {
        ssh_ops_core::preflight::check_or_exit(&sshops_home());
    }

    match cli.cmd {
        Cmd::Run(c) => cmd_run(c, no_daemon),
        Cmd::Open(c) => cmd_open(c, no_daemon),
        Cmd::Close(c) => cmd_close(c, no_daemon),
        Cmd::Peek(c) => cmd_peek(c, no_daemon),
        Cmd::ListPanes => cmd_list_panes(no_daemon),
        Cmd::Recent { common, seconds } => cmd_recent(common, seconds, no_daemon),
        Cmd::Send { common, raw } => cmd_send(common, raw, no_daemon),
        Cmd::DaemonStatus => cmd_daemon_status(),
        Cmd::DaemonStop => cmd_daemon_stop(),
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
fn cmd_run(common: CommonArgs, no_daemon: bool) -> Result<()> {
    let timing = std::env::var("SSHOPS_DEBUG_TIMING").as_deref() == Ok("1");
    let t_main = std::time::Instant::now();
    let log = |name: &str| {
        if timing {
            eprintln!(
                "[TIMING] cli {name:>22}: {:>7.2}ms (total)",
                t_main.elapsed().as_micros() as f64 / 1000.0
            );
        }
    };
    log("entry (clap parsed)");
    let home = sshops_home();
    log("sshops_home()");
    if !no_daemon {
        if let Some(resp) = try_ipc_run(&home, &common)? {
            log("ipc returned");
            let r = print_run_resp(resp);
            log("printed json");
            return r;
        }
    }
    cmd_run_inproc(common)
}

fn try_ipc_run(home: &std::path::Path, common: &CommonArgs) -> Result<Option<IpcResponse>> {
    let using_tmp = common.host.is_some() && common.user.is_some();
    let spec = ipc_client::build_selector_spec(
        &common.args,
        common.host.as_deref(),
        common.user.as_deref(),
        common.port,
        common.key.as_deref(),
        common.prod,
        common.password.as_deref(),
        common.ask_password,
    )?;
    let cmd_text = ipc_client::cmd_text_from(&common.args, using_tmp)?;
    let req = IpcRequest::Run {
        ctx: ipc_client::build_ctx(home),
        selector: spec,
        cmd: cmd_text,
        timeout_ms: common.timeout.unwrap_or(30) * 1000,
        i_mean_it: common.i_mean_it,
        auto_human: std::env::var("SSHOPS_NO_AUTO_HUMAN").as_deref() != Ok("1"),
    };
    ipc_client::call_sync(home, req)
}

fn print_run_resp(resp: IpcResponse) -> Result<()> {
    match resp {
        IpcResponse::Run(r) => {
            if r.password_prompted {
                eprintln!("⚠ ssh login / sudo 中曾需要密码, 已在 wezterm pane 完成输入");
            }
            let resp = json!({
                "exit": r.exit,
                "duration_ms": r.duration_ms,
                "cast_offset": r.cast_offset,
                "selector": r.selector,
                "session_id": r.session_id,
                "dangerous": r.dangerous,
                "blocked": r.blocked,
                "reason": r.reason,
                "output": r.output,
                "recent_human_activity": r.recent_human_activity,
                "password_prompted": r.password_prompted,
            });
            println!("{}", serde_json::to_string(&resp)?);
            if r.blocked {
                std::process::exit(5);
            }
            Ok(())
        }
        IpcResponse::Error(e) => Err(anyhow!("daemon: {e}")),
        other => Err(anyhow!("daemon 返回非预期类型: {:?}", other)),
    }
}

fn cmd_run_inproc(common: CommonArgs) -> Result<()> {
    let timing = std::env::var("SSHOPS_DEBUG_TIMING").as_deref() == Ok("1");
    let t_main = std::time::Instant::now();
    let mut t_prev = t_main;
    let mut log_step = |name: &str| {
        if timing {
            let now = std::time::Instant::now();
            let dt = now.duration_since(t_prev).as_micros() as f64 / 1000.0;
            let total = now.duration_since(t_main).as_micros() as f64 / 1000.0;
            eprintln!("[TIMING] {name:>30}: dt={dt:>7.2}ms  total={total:>7.2}ms");
            t_prev = now;
        }
    };
    log_step("entry");

    let home = sshops_home();
    log_step("sshops_home()");
    let cfg = config::load(&home)?;
    log_step("config::load");
    let r = resolve(&common, true, &cfg)?;
    log_step("resolve(selector + cmd)");
    let cmd_text = r.cmd.clone().unwrap();

    // safety gate
    let gate = safety_gate(&cmd_text, &r.safety_selector, common.i_mean_it, &cfg);
    log_step("safety_gate");
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
    let session_key = state::current_session_key();
    let opened = pane::pane_open(&cfg, &home, &wez, &store, &session_key, &r.sel.display, &target)?;
    let pane_id = opened.pane_id;
    let recorder = opened.recorder;
    let password_prompted = opened.password_prompted;
    if password_prompted {
        eprintln!("⚠ ssh login / sudo 中曾需要密码, 已在 wezterm pane 完成输入");
    }
    log_step(if opened.reused { "pane_open (reused)" } else { "pane_open (spawn)" });

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
    log_step("recent_human_activity");

    // execute
    let timeout_secs = common.timeout.unwrap_or(30);
    let outcome = execute(&wez, pane_id, &recorder, &cmd_text, Duration::from_secs(timeout_secs))?;
    log_step("execute (send+wait_prompt+slice)");

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

    log_step("append_command");

    // 标记 last_ai_byte
    if auto_human {
        let _ = recorder.write_last_ai_byte(recorder.cast_size());
    }
    log_step("write_last_ai_byte");

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
        "password_prompted": password_prompted,
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
fn cmd_open(common: CommonArgs, no_daemon: bool) -> Result<()> {
    let home = sshops_home();
    if !no_daemon {
        let spec = ipc_client::build_selector_spec(
            &common.args,
            common.host.as_deref(),
            common.user.as_deref(),
            common.port,
            common.key.as_deref(),
            common.prod,
            common.password.as_deref(),
            common.ask_password,
        )?;
        let req = IpcRequest::Open {
            ctx: ipc_client::build_ctx(&home),
            selector: spec,
        };
        if let Some(resp) = ipc_client::call_sync(&home, req)? {
            return match resp {
                IpcResponse::Open(o) => {
                    if o.password_prompted {
                        eprintln!("⚠ ssh login / sudo 中曾需要密码, 已在 wezterm pane 完成输入");
                    }
                    let v = json!({
                        "selector": o.selector,
                        "source": o.source,
                        "pane_id": o.pane_id,
                        "session_id": o.session_id,
                        "user": o.user,
                        "host": o.host,
                        "port": o.port,
                        "key": o.key,
                        "reused": o.reused,
                        "password_prompted": o.password_prompted,
                    });
                    println!("{}", serde_json::to_string(&v)?);
                    Ok(())
                }
                IpcResponse::Error(e) => Err(anyhow!("daemon: {e}")),
                other => Err(anyhow!("daemon 返回非预期类型: {:?}", other)),
            };
        }
    }
    cmd_open_inproc(common)
}

fn cmd_open_inproc(common: CommonArgs) -> Result<()> {
    let home = sshops_home();
    let cfg = config::load(&home)?;
    let r = resolve(&common, false, &cfg)?;
    let wez = WezTermClient::new(cfg.wezterm.cli_path.clone());
    let store = state::StateStore::new(&state_dir(&home))?;
    let target = build_target(&r);
    let session_key = state::current_session_key();
    let opened = pane::pane_open(&cfg, &home, &wez, &store, &session_key, &r.sel.display, &target)?;
    if opened.password_prompted {
        eprintln!("⚠ ssh login / sudo 中曾需要密码, 已在 wezterm pane 完成输入");
    }
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
        "password_prompted": opened.password_prompted,
    });
    println!("{}", serde_json::to_string(&resp)?);
    Ok(())
}

// ============================================================
// cmd_close
// ============================================================
fn cmd_close(common: CommonArgs, no_daemon: bool) -> Result<()> {
    let home = sshops_home();
    if !no_daemon {
        let spec = ipc_client::build_selector_spec(
            &common.args,
            common.host.as_deref(),
            common.user.as_deref(),
            common.port,
            common.key.as_deref(),
            common.prod,
            common.password.as_deref(),
            common.ask_password,
        )?;
        let req = IpcRequest::Close {
            ctx: ipc_client::build_ctx(&home),
            selector: spec,
        };
        if let Some(resp) = ipc_client::call_sync(&home, req)? {
            return match resp {
                IpcResponse::Closed => Ok(()),
                IpcResponse::Error(e) => Err(anyhow!("daemon: {e}")),
                other => Err(anyhow!("daemon 返回非预期类型: {:?}", other)),
            };
        }
    }
    cmd_close_inproc(common)
}

fn cmd_close_inproc(common: CommonArgs) -> Result<()> {
    let home = sshops_home();
    let cfg = config::load(&home)?;
    let r = resolve(&common, false, &cfg)?;
    let wez = WezTermClient::new(cfg.wezterm.cli_path.clone());
    let store = state::StateStore::new(&state_dir(&home))?;
    let session_key = state::current_session_key();
    pane::pane_close(&cfg, &wez, &store, &session_key, &r.sel.display)?;
    tracing::info!("closed: {}", r.sel.display);
    Ok(())
}

// ============================================================
// cmd_peek
// ============================================================
fn cmd_peek(common: CommonArgs, no_daemon: bool) -> Result<()> {
    let home = sshops_home();
    if !no_daemon {
        let spec = ipc_client::build_selector_spec(
            &common.args,
            common.host.as_deref(),
            common.user.as_deref(),
            common.port,
            common.key.as_deref(),
            common.prod,
            common.password.as_deref(),
            common.ask_password,
        )?;
        let req = IpcRequest::Peek {
            ctx: ipc_client::build_ctx(&home),
            selector: spec,
        };
        if let Some(resp) = ipc_client::call_sync(&home, req)? {
            return match resp {
                IpcResponse::Peek(text) => {
                    print!("{text}");
                    Ok(())
                }
                IpcResponse::Error(e) => Err(anyhow!("daemon: {e}")),
                other => Err(anyhow!("daemon 返回非预期类型: {:?}", other)),
            };
        }
    }
    cmd_peek_inproc(common)
}

fn cmd_peek_inproc(common: CommonArgs) -> Result<()> {
    let home = sshops_home();
    let cfg = config::load(&home)?;
    let r = resolve(&common, false, &cfg)?;
    let store = state::StateStore::new(&state_dir(&home))?;
    let pid = state::project_id();
    let session_key = state::current_session_key();
    let info = store
        .get_pane(&pid, &session_key, &r.sel.display)?
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
fn cmd_list_panes(no_daemon: bool) -> Result<()> {
    let home = sshops_home();
    if !no_daemon {
        let req = IpcRequest::ListPanes {
            ctx: ipc_client::build_ctx(&home),
        };
        if let Some(resp) = ipc_client::call_sync(&home, req)? {
            return match resp {
                IpcResponse::Panes(p) => {
                    let panes_obj: serde_json::Map<String, serde_json::Value> = p
                        .panes
                        .into_iter()
                        .map(|(s, info)| {
                            (
                                s,
                                json!({
                                    "pane_id": info.pane_id,
                                    "session_id": info.session_id,
                                    "started_at": info.started_at,
                                }),
                            )
                        })
                        .collect();
                    let v = json!({
                        "wezterm_window_id": p.wezterm_window_id,
                        "started_at": p.started_at,
                        "panes": panes_obj,
                    });
                    println!("{}", serde_json::to_string_pretty(&v)?);
                    Ok(())
                }
                IpcResponse::Error(e) => Err(anyhow!("daemon: {e}")),
                other => Err(anyhow!("daemon 返回非预期类型: {:?}", other)),
            };
        }
    }
    cmd_list_panes_inproc()
}

fn cmd_list_panes_inproc() -> Result<()> {
    let home = sshops_home();
    let store = state::StateStore::new(&state_dir(&home))?;
    let pid = state::project_id();
    let session_key = state::current_session_key();
    let sess = store.session_state(&pid, &session_key)?.unwrap_or_default();
    let panes_obj: serde_json::Map<String, serde_json::Value> = sess
        .panes
        .iter()
        .map(|(s, info)| (s.clone(), serde_json::to_value(info).unwrap()))
        .collect();
    let resp = json!({
        "session_key": session_key,
        "wezterm_window_id": sess.wezterm_window_id,
        "started_at": sess.started_at,
        "panes": panes_obj,
    });
    println!("{}", serde_json::to_string_pretty(&resp)?);
    Ok(())
}

// ============================================================
// cmd_recent: 主动查最近 N 秒的 human 活动 (不依赖上次 ai run 边界)
// ============================================================
fn cmd_recent(common: CommonArgs, seconds: u64, no_daemon: bool) -> Result<()> {
    let home = sshops_home();
    if !no_daemon {
        let spec = ipc_client::build_selector_spec(
            &common.args,
            common.host.as_deref(),
            common.user.as_deref(),
            common.port,
            common.key.as_deref(),
            common.prod,
            common.password.as_deref(),
            common.ask_password,
        )?;
        let req = IpcRequest::Recent {
            ctx: ipc_client::build_ctx(&home),
            selector: spec,
            seconds,
        };
        if let Some(resp) = ipc_client::call_sync(&home, req)? {
            return match resp {
                IpcResponse::Recent(list) => {
                    println!("{}", serde_json::to_string(&list)?);
                    Ok(())
                }
                IpcResponse::Error(e) => Err(anyhow!("daemon: {e}")),
                other => Err(anyhow!("daemon 返回非预期类型: {:?}", other)),
            };
        }
    }
    cmd_recent_inproc(common, seconds)
}

fn cmd_recent_inproc(common: CommonArgs, seconds: u64) -> Result<()> {
    let home = sshops_home();
    let cfg = config::load(&home)?;
    let r = resolve(&common, false, &cfg)?;
    let store = state::StateStore::new(&state_dir(&home))?;
    let pid = state::project_id();
    let session_key = state::current_session_key();
    let info = store
        .get_pane(&pid, &session_key, &r.sel.display)?
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

// ============================================================
// send: 发送原始按键到 pane (不等 prompt 立即返回)
// ============================================================
fn cmd_send(common: CommonArgs, raw: bool, no_daemon: bool) -> Result<()> {
    let home = sshops_home();
    let using_tmp = common.host.is_some() && common.user.is_some();
    let spec = ipc_client::build_selector_spec(
        &common.args,
        common.host.as_deref(),
        common.user.as_deref(),
        common.port,
        common.key.as_deref(),
        common.prod,
        common.password.as_deref(),
        common.ask_password,
    )?;
    let keys = ipc_client::cmd_text_from(&common.args, using_tmp)?;

    if !no_daemon {
        let req = IpcRequest::Send {
            ctx: ipc_client::build_ctx(&home),
            selector: spec,
            keys: keys.clone(),
            raw,
        };
        if let Some(resp) = ipc_client::call_sync(&home, req)? {
            return match resp {
                IpcResponse::Send(r) => {
                    let j = json!({
                        "selector": r.selector,
                        "session_id": r.session_id,
                        "bytes_sent": r.bytes_sent,
                        "duration_ms": r.duration_ms,
                    });
                    println!("{}", serde_json::to_string(&j)?);
                    Ok(())
                }
                IpcResponse::Error(e) => Err(anyhow!("daemon: {e}")),
                other => Err(anyhow!("daemon 返回非预期类型: {:?}", other)),
            };
        }
    }
    cmd_send_inproc(common, keys, raw)
}

fn cmd_send_inproc(common: CommonArgs, keys: String, raw: bool) -> Result<()> {
    let home = sshops_home();
    let cfg = config::load(&home)?;
    let r = resolve(&common, false, &cfg)?;
    let store = state::StateStore::new(&state_dir(&home))?;
    let pid = state::project_id();
    let session_key = state::current_session_key();
    let info = store
        .get_pane(&pid, &session_key, &r.sel.display)?
        .ok_or_else(|| anyhow!("未找到 pane: {} (先用 sshops open / run)", r.sel.display))?;
    let payload = if raw { keys } else { unescape_keys(&keys) };
    let bytes_sent = payload.len();
    let started = std::time::Instant::now();
    let wez = ssh_ops_core::wezterm_mux::WezTermClient::new(cfg.wezterm.cli_path.clone());
    if !wez.pane_alive(info.pane_id) {
        return Err(anyhow!("pane 已失效: {}", r.sel.display));
    }
    wez.send_text(info.pane_id, &payload)?;
    let j = json!({
        "selector": r.sel.display,
        "session_id": info.session_id,
        "bytes_sent": bytes_sent,
        "duration_ms": started.elapsed().as_millis() as u64,
    });
    println!("{}", serde_json::to_string(&j)?);
    Ok(())
}

/// 跟 daemon 端 unescape_keys 同语义: 仅处理 \r \n \t \\.
fn unescape_keys(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('r') => out.push('\r'),
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('\\') => out.push('\\'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

// ============================================================
// daemon 控制
// ============================================================
fn cmd_daemon_status() -> Result<()> {
    let home = sshops_home();
    let resp = ipc_client::call_sync(&home, IpcRequest::Status)?
        .ok_or_else(|| anyhow!("daemon 未运行"))?;
    match resp {
        IpcResponse::Status(s) => {
            let v = json!({
                "uptime_secs": s.uptime_secs,
                "req_count": s.req_count,
                "pane_count": s.pane_count,
                "session_count": s.session_count,
                "started_at": s.started_at,
            });
            println!("{}", serde_json::to_string_pretty(&v)?);
            Ok(())
        }
        IpcResponse::Error(e) => Err(anyhow!("daemon: {e}")),
        other => Err(anyhow!("daemon 返回非预期类型: {:?}", other)),
    }
}

fn cmd_daemon_stop() -> Result<()> {
    let home = sshops_home();
    let resp = ipc_client::call_sync(&home, IpcRequest::Shutdown)?
        .ok_or_else(|| anyhow!("daemon 未运行"))?;
    match resp {
        IpcResponse::Bye => {
            println!("{{\"shutdown\": true}}");
            Ok(())
        }
        IpcResponse::Error(e) => Err(anyhow!("daemon: {e}")),
        other => Err(anyhow!("daemon 返回非预期类型: {:?}", other)),
    }
}
