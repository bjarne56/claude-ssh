//! IPC request → response 路由
//!
//! 现阶段直接调 core::pane / session (同步执行, 一次一个请求 await).
//! 真正的并发由 tokio runtime 多 worker 线程提供 — 不同请求独立处理.

use crate::{get_pane_lock, get_session_lock, pane_lock_key, session_lock_key, DaemonState};
use ssh_ops_core::{
    config::expand_path,
    human_detect, ipc::{
        ClientCtx, IpcRequest, IpcResponse, OpenResp, PaneEntry, PanesResp, RunResp, SelectorSpec,
        SendResp, StatusInfo, PROTO_VERSION,
    }, pane::{self, SshTarget}, recorder::{gen_nonce, Recorder}, safety::safety_gate, selector::{resolve_crt, resolve_tmp, ResolvedSelector, Source}, session::{execute, strip_ansi}, state::{self, StateStore}, state_dir, wezterm_mux::WezTermClient,
};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

pub async fn dispatch(req: IpcRequest, state: Arc<Mutex<DaemonState>>) -> IpcResponse {
    {
        let mut s = state.lock().await;
        s.req_count += 1;
        s.last_req_at = Instant::now();
    }
    match req {
        IpcRequest::Ping => IpcResponse::Pong,
        IpcRequest::Shutdown => {
            tracing::info!("收到 Shutdown 请求, 退出");
            let sock = {
                let s = state.lock().await;
                ssh_ops_core::ipc::default_sock_path(&s.home)
            };
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(50)).await;
                let _ = std::fs::remove_file(&sock);
                std::process::exit(0);
            });
            IpcResponse::Bye
        }
        IpcRequest::Status => {
            let s = state.lock().await;
            let st = StateStore::new(&state_dir(&s.home)).ok();
            let panes_count = st
                .as_ref()
                .and_then(|st| st.read().ok())
                .map(|ps| ps.projects.values().flat_map(|p| p.sessions.values()).map(|s| s.panes.len()).sum())
                .unwrap_or(0);
            IpcResponse::Status(StatusInfo {
                uptime_secs: s.started_at.elapsed().as_secs(),
                req_count: s.req_count,
                pane_count: panes_count,
                session_count: panes_count, // 简化: 1 pane = 1 session
                started_at: s.started_at_iso.clone(),
            })
        }
        IpcRequest::Run { ctx, selector, cmd, timeout_ms, i_mean_it, auto_human } => {
            handle_run(state, ctx, selector, cmd, timeout_ms, i_mean_it, auto_human)
                .await
                .unwrap_or_else(|e| IpcResponse::Error(format!("run: {e}")))
        }
        IpcRequest::Open { ctx, selector } => handle_open(state, ctx, selector)
            .await
            .unwrap_or_else(|e| IpcResponse::Error(format!("open: {e}"))),
        IpcRequest::Close { ctx, selector } => handle_close(state, ctx, selector)
            .await
            .unwrap_or_else(|e| IpcResponse::Error(format!("close: {e}"))),
        IpcRequest::Peek { ctx, selector } => handle_peek(state, ctx, selector)
            .await
            .unwrap_or_else(|e| IpcResponse::Error(format!("peek: {e}"))),
        IpcRequest::ListPanes { ctx } => handle_list_panes(state, ctx)
            .await
            .unwrap_or_else(|e| IpcResponse::Error(format!("list-panes: {e}"))),
        IpcRequest::Recent { ctx, selector, seconds } => {
            handle_recent(state, ctx, selector, seconds)
                .await
                .unwrap_or_else(|e| IpcResponse::Error(format!("recent: {e}")))
        }
        IpcRequest::Send { ctx, selector, keys, raw } => handle_send(state, ctx, selector, keys, raw)
            .await
            .unwrap_or_else(|e| IpcResponse::Error(format!("send: {e}"))),
    }
}

fn check_proto(ctx: &ClientCtx) -> Result<(), IpcResponse> {
    if ctx.proto != PROTO_VERSION {
        return Err(IpcResponse::Error(format!(
            "proto 版本不兼容: client={} server={}",
            ctx.proto, PROTO_VERSION
        )));
    }
    Ok(())
}

/// SelectorSpec → ResolvedSelector + (auth_type, password, safety_selector)
fn resolve_selector(
    state_guard: &DaemonState,
    spec: &SelectorSpec,
) -> anyhow::Result<(ResolvedSelector, String, Option<String>, String)> {
    match spec {
        SelectorSpec::Crt(input) => {
            let parser = state_guard
                .crt_parser
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("CrtParser 未初始化 (检查 securecrt_config_dir)"))?;
            let sel = resolve_crt(parser, input).map_err(|e| anyhow::anyhow!("{e}"))?;
            let safety = sel.display.clone();
            // password 模式 + 无 key + cli 没传 password: daemon 端无法 prompt, 让用户在 pane 手输
            let auth = if sel.password_present && sel.key.is_none() {
                "password".to_string()
            } else {
                "key".into()
            };
            Ok((sel, auth, None, safety))
        }
        SelectorSpec::Tmp { user, host, port, key, prod, auth_type, password } => {
            let sel = resolve_tmp(user, host, Some(*port), key.clone());
            let safety = if *prod {
                "__SSHOPS_TMP_PROD__".to_string()
            } else {
                "__SSHOPS_TMP_NONPROD__".into()
            };
            Ok((sel, auth_type.clone(), password.clone(), safety))
        }
    }
}

fn build_target(sel: &ResolvedSelector, auth_type: &str, password: Option<String>) -> SshTarget {
    SshTarget {
        user: sel.user.clone(),
        host: sel.host.clone(),
        port: sel.port,
        key: sel.key.clone(),
        auth_type: auth_type.to_string(),
        password,
    }
}

// === Run ===================================================================

async fn handle_run(
    state: Arc<Mutex<DaemonState>>,
    ctx: ClientCtx,
    selector: SelectorSpec,
    cmd: String,
    timeout_ms: u64,
    i_mean_it: bool,
    auto_human: bool,
) -> anyhow::Result<IpcResponse> {
    if let Err(r) = check_proto(&ctx) {
        return Ok(r);
    }
    // 锁内只做 selector 解析 + safety 计算, 然后释放, 让 spawn_blocking 跑重活
    let (sel, auth_type, password, safety_sel, cfg, home) = {
        let s = state.lock().await;
        let (sel, auth, pw, safety) = resolve_selector(&s, &selector)?;
        (sel, auth, pw, safety, s.cfg.clone(), s.home.clone())
    };
    let gate = safety_gate(&cmd, &safety_sel, i_mean_it, &cfg);
    if gate.blocked {
        let sid = Recorder::make_session_id(&sel.host);
        let rec = Recorder::init(&cfg, &sid, &sel.display, &sel.host, &sel.user, &auth_type)?;
        let _ = rec.append_command("ai", &sel.display, &cmd, -1, 0, true, true, &gen_nonce());
        let _ = rec.finalize();
        return Ok(IpcResponse::Run(RunResp {
            exit: -1,
            output: "(not executed)".into(),
            duration_ms: 0,
            cast_offset: 0.0,
            session_id: sid,
            selector: sel.display,
            dangerous: true,
            blocked: true,
            reason: Some(gate.reason),
            recent_human_activity: vec![],
            password_prompted: false,
        }));
    }

    // 锁顺序: session_lock (同 session 的 spawn 串行化, 防 race) → pane_lock (同 pane 串行化)
    let sess_lock = get_session_lock(&state, &session_lock_key(&ctx.project_id, &ctx.session_key)).await;
    let _sess_guard = sess_lock.lock().await;
    let lock_key = pane_lock_key(&ctx.project_id, &ctx.session_key, &sel.display);
    let pane_lock = get_pane_lock(&state, &lock_key).await;
    let _pane_guard = pane_lock.lock().await;

    // 重活在 blocking 池里跑 (内部都是同步 IO + thread::sleep)
    let sel_clone = sel.clone();
    let target = build_target(&sel, &auth_type, password);
    let project_id_str = ctx.project_id.clone();
    let session_key = ctx.session_key.clone();

    let res = tokio::task::spawn_blocking(move || -> anyhow::Result<RunResp> {
        // 把 cli 端的 project_id 通过 env 注入 (state::project_id 优先读 env)
        std::env::set_var("SSHOPS_PROJECT", &project_id_str);

        let wez = WezTermClient::new(cfg.wezterm.cli_path.clone());
        let store = StateStore::new(&state_dir(&home))?;
        let opened = pane::pane_open(&cfg, &home, &wez, &store, &session_key, &sel_clone.display, &target)?;
        let recorder = opened.recorder;
        let pane_id = opened.pane_id;
        let password_prompted = opened.password_prompted;

        // recent_human_activity
        let mut recent_human = Vec::new();
        if auto_human {
            let last = recorder.read_last_ai_byte();
            let cur = recorder.cast_size();
            if cur > last {
                let buf = recorder.read_cast_range(last, cur).unwrap_or_default();
                let header_ts = cast_header_ts(&recorder.cast_path).unwrap_or(0.0) as u64;
                recent_human = human_detect::extract_human_commands(&buf, header_ts);
                for h in &recent_human {
                    let nonce = format!("human-{}", h.cast_offset);
                    let _ = recorder.append_command(
                        "human",
                        &sel_clone.display,
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

        let outcome = execute(
            &wez,
            pane_id,
            &recorder,
            &cmd,
            Duration::from_millis(timeout_ms),
        )?;

        recorder.append_command(
            "ai",
            &sel_clone.display,
            &cmd,
            outcome.exit,
            outcome.duration_ms,
            gate.dangerous,
            false,
            &gen_nonce(),
        )?;
        if auto_human {
            let _ = recorder.write_last_ai_byte(recorder.cast_size());
        }

        Ok(RunResp {
            exit: outcome.exit,
            output: outcome.output,
            duration_ms: outcome.duration_ms,
            cast_offset: outcome.cast_offset,
            session_id: recorder.session_id,
            selector: sel_clone.display,
            dangerous: gate.dangerous,
            blocked: false,
            reason: None,
            recent_human_activity: recent_human,
            password_prompted,
        })
    })
    .await??;

    Ok(IpcResponse::Run(res))
}

// === Open ===================================================================

async fn handle_open(
    state: Arc<Mutex<DaemonState>>,
    ctx: ClientCtx,
    selector: SelectorSpec,
) -> anyhow::Result<IpcResponse> {
    if let Err(r) = check_proto(&ctx) {
        return Ok(r);
    }
    let (sel, auth_type, password, _, cfg, home) = {
        let s = state.lock().await;
        let (sel, auth, pw, safety) = resolve_selector(&s, &selector)?;
        (sel, auth, pw, safety, s.cfg.clone(), s.home.clone())
    };
    let target = build_target(&sel, &auth_type, password);
    let sel_clone = sel.clone();
    let project_id_str = ctx.project_id.clone();
    let session_key = ctx.session_key.clone();

    let sess_lock = get_session_lock(&state, &session_lock_key(&ctx.project_id, &ctx.session_key)).await;
    let _sess_guard = sess_lock.lock().await;
    let lock_key = pane_lock_key(&ctx.project_id, &ctx.session_key, &sel.display);
    let pane_lock = get_pane_lock(&state, &lock_key).await;
    let _pane_guard = pane_lock.lock().await;

    let resp = tokio::task::spawn_blocking(move || -> anyhow::Result<OpenResp> {
        std::env::set_var("SSHOPS_PROJECT", &project_id_str);
        let wez = WezTermClient::new(cfg.wezterm.cli_path.clone());
        let store = StateStore::new(&state_dir(&home))?;
        let opened = pane::pane_open(&cfg, &home, &wez, &store, &session_key, &sel_clone.display, &target)?;
        Ok(OpenResp {
            selector: sel_clone.display.clone(),
            source: match sel_clone.source {
                Source::Crt => "crt".into(),
                Source::Tmp => "tmp".into(),
            },
            pane_id: opened.pane_id,
            session_id: opened.session_id,
            user: sel_clone.user,
            host: sel_clone.host,
            port: sel_clone.port,
            key: sel_clone
                .key
                .as_ref()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default(),
            reused: opened.reused,
            password_prompted: opened.password_prompted,
        })
    })
    .await??;
    Ok(IpcResponse::Open(resp))
}

// === Close ==================================================================

async fn handle_close(
    state: Arc<Mutex<DaemonState>>,
    ctx: ClientCtx,
    selector: SelectorSpec,
) -> anyhow::Result<IpcResponse> {
    if let Err(r) = check_proto(&ctx) {
        return Ok(r);
    }
    let (sel, _, _, _, cfg, home) = {
        let s = state.lock().await;
        let (sel, auth, pw, safety) = resolve_selector(&s, &selector)?;
        (sel, auth, pw, safety, s.cfg.clone(), s.home.clone())
    };
    let project_id_str = ctx.project_id.clone();
    let sel_display = sel.display.clone();
    let session_key = ctx.session_key.clone();

    let sess_lock = get_session_lock(&state, &session_lock_key(&ctx.project_id, &ctx.session_key)).await;
    let _sess_guard = sess_lock.lock().await;
    let lock_key = pane_lock_key(&ctx.project_id, &ctx.session_key, &sel_display);
    let pane_lock = get_pane_lock(&state, &lock_key).await;
    let _pane_guard = pane_lock.lock().await;

    tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
        std::env::set_var("SSHOPS_PROJECT", &project_id_str);
        let wez = WezTermClient::new(cfg.wezterm.cli_path.clone());
        let store = StateStore::new(&state_dir(&home))?;
        pane::pane_close(&cfg, &wez, &store, &session_key, &sel_display)?;
        Ok(())
    })
    .await??;
    Ok(IpcResponse::Closed)
}

// === Peek ===================================================================

async fn handle_peek(
    state: Arc<Mutex<DaemonState>>,
    ctx: ClientCtx,
    selector: SelectorSpec,
) -> anyhow::Result<IpcResponse> {
    if let Err(r) = check_proto(&ctx) {
        return Ok(r);
    }
    let (sel, _, _, _, cfg, home) = {
        let s = state.lock().await;
        let (sel, auth, pw, safety) = resolve_selector(&s, &selector)?;
        (sel, auth, pw, safety, s.cfg.clone(), s.home.clone())
    };
    let project_id_str = ctx.project_id.clone();
    let sel_display = sel.display.clone();
    let session_key = ctx.session_key.clone();

    // peek 加锁: 等同 pane 的 run/open/close 完成, 避免读到执行中状态
    let lock_key = pane_lock_key(&ctx.project_id, &ctx.session_key, &sel_display);
    let pane_lock = get_pane_lock(&state, &lock_key).await;
    let _pane_guard = pane_lock.lock().await;

    let text = tokio::task::spawn_blocking(move || -> anyhow::Result<String> {
        std::env::set_var("SSHOPS_PROJECT", &project_id_str);
        let store = StateStore::new(&state_dir(&home))?;
        let pid = state::project_id();
        let info = store
            .get_pane(&pid, &session_key, &sel_display)?
            .ok_or_else(|| anyhow::anyhow!("未找到 pane: {sel_display}"))?;
        let wez = WezTermClient::new(cfg.wezterm.cli_path.clone());
        if !wez.pane_alive(info.pane_id) {
            anyhow::bail!("pane 已失效: {sel_display}");
        }
        let raw = wez.get_text(info.pane_id, Some(50000))?;
        Ok(strip_ansi(&raw))
    })
    .await??;
    Ok(IpcResponse::Peek(text))
}

// === ListPanes ==============================================================

async fn handle_list_panes(
    state: Arc<Mutex<DaemonState>>,
    ctx: ClientCtx,
) -> anyhow::Result<IpcResponse> {
    if let Err(r) = check_proto(&ctx) {
        return Ok(r);
    }
    let home = { state.lock().await.home.clone() };
    let project_id_str = ctx.project_id.clone();
    let session_key = ctx.session_key.clone();
    let resp = tokio::task::spawn_blocking(move || -> anyhow::Result<PanesResp> {
        std::env::set_var("SSHOPS_PROJECT", &project_id_str);
        let store = StateStore::new(&state_dir(&home))?;
        let pid = state::project_id();
        let sess = store.session_state(&pid, &session_key)?.unwrap_or_default();
        let panes: Vec<(String, PaneEntry)> = sess
            .panes
            .iter()
            .map(|(s, info)| {
                (
                    s.clone(),
                    PaneEntry {
                        pane_id: info.pane_id,
                        session_id: info.session_id.clone(),
                        started_at: info.started_at.clone(),
                    },
                )
            })
            .collect();
        Ok(PanesResp {
            wezterm_window_id: sess.wezterm_window_id,
            started_at: sess.started_at,
            panes,
        })
    })
    .await??;
    Ok(IpcResponse::Panes(resp))
}

// === Recent =================================================================

async fn handle_recent(
    state: Arc<Mutex<DaemonState>>,
    ctx: ClientCtx,
    selector: SelectorSpec,
    seconds: u64,
) -> anyhow::Result<IpcResponse> {
    if let Err(r) = check_proto(&ctx) {
        return Ok(r);
    }
    let (sel, _, _, _, cfg, home) = {
        let s = state.lock().await;
        let (sel, auth, pw, safety) = resolve_selector(&s, &selector)?;
        (sel, auth, pw, safety, s.cfg.clone(), s.home.clone())
    };
    let project_id_str = ctx.project_id.clone();
    let sel_display = sel.display.clone();
    let session_key = ctx.session_key.clone();

    // recent 加锁: 等同 pane 的 run 完成, 避免读到一半 cast
    let lock_key = pane_lock_key(&ctx.project_id, &ctx.session_key, &sel_display);
    let pane_lock = get_pane_lock(&state, &lock_key).await;
    let _pane_guard = pane_lock.lock().await;

    let recent = tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<human_detect::HumanCmd>> {
        std::env::set_var("SSHOPS_PROJECT", &project_id_str);
        let store = StateStore::new(&state_dir(&home))?;
        let pid = state::project_id();
        let info = store
            .get_pane(&pid, &session_key, &sel_display)?
            .ok_or_else(|| anyhow::anyhow!("未找到 pane: {sel_display}"))?;
        let recorder = Recorder::open(&cfg, &info.session_id)?;
        let size = recorder.cast_size();
        let buf = recorder.read_cast_range(0, size).unwrap_or_default();
        let header_ts = cast_header_ts(&recorder.cast_path).unwrap_or(0.0) as u64;
        let all = human_detect::extract_human_commands(&buf, header_ts);
        let now = chrono::Utc::now().timestamp() as u64;
        let cutoff = now.saturating_sub(seconds);
        Ok(all.into_iter().filter(|h| h.ts_unix >= cutoff).collect())
    })
    .await??;
    Ok(IpcResponse::Recent(recent))
}

// === Send (interactive keys, no prompt wait) ===============================

async fn handle_send(
    state: Arc<Mutex<DaemonState>>,
    ctx: ClientCtx,
    selector: SelectorSpec,
    keys: String,
    raw: bool,
) -> anyhow::Result<IpcResponse> {
    if let Err(r) = check_proto(&ctx) {
        return Ok(r);
    }
    let started = Instant::now();
    let (sel, _, _, _, cfg, home) = {
        let s = state.lock().await;
        let (sel, auth, pw, safety) = resolve_selector(&s, &selector)?;
        (sel, auth, pw, safety, s.cfg.clone(), s.home.clone())
    };
    let project_id_str = ctx.project_id.clone();
    let sel_display = sel.display.clone();
    let session_key = ctx.session_key.clone();

    // 跟 run/peek 同 pane 互斥, 避免并发写 PTY 导致键序错乱.
    let lock_key = pane_lock_key(&ctx.project_id, &ctx.session_key, &sel_display);
    let pane_lock = get_pane_lock(&state, &lock_key).await;
    let _pane_guard = pane_lock.lock().await;

    // 默认: 解释常见 escape 序列让 CLI 用户能传 \r \n \t \\;
    // raw=true: 字面发送, 用户需要自己负责所有 byte (二进制 keys 用例).
    let payload = if raw { keys.clone() } else { unescape_keys(&keys) };
    let bytes_sent = payload.len();

    let cfg_for_blocking = cfg.clone();
    let result = tokio::task::spawn_blocking(move || -> anyhow::Result<(u64, String)> {
        std::env::set_var("SSHOPS_PROJECT", &project_id_str);
        let store = StateStore::new(&state_dir(&home))?;
        let pid = state::project_id();
        let info = store
            .get_pane(&pid, &session_key, &sel_display)?
            .ok_or_else(|| anyhow::anyhow!("未找到 pane: {sel_display} (先用 sshops open / run 建立)"))?;
        let wez = WezTermClient::new(cfg_for_blocking.wezterm.cli_path.clone());
        if !wez.pane_alive(info.pane_id) {
            anyhow::bail!("pane 已失效: {sel_display}");
        }
        wez.send_text(info.pane_id, &payload)?;
        Ok((info.pane_id, info.session_id))
    })
    .await??;

    let (_pane_id, session_id) = result;
    Ok(IpcResponse::Send(SendResp {
        selector: sel.display,
        session_id,
        bytes_sent,
        duration_ms: started.elapsed().as_millis() as u64,
    }))
}

/// 解 escape: 仅处理 \r \n \t \\, 其余 \x 字面保留 (含反斜杠 + x).
/// CLI 端 shell 已经吃过一层引号, 这里是 second-level escape, 让用户能在
/// 单引号字符串里塞 \r 这种通用控制字符.
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

// === helpers ================================================================

fn cast_header_ts(cast: &std::path::Path) -> Option<f64> {
    use std::io::BufRead;
    let f = std::fs::File::open(cast).ok()?;
    let mut reader = std::io::BufReader::new(f);
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    let v: serde_json::Value = serde_json::from_str(line.trim()).ok()?;
    v.get("timestamp").and_then(|t| t.as_f64())
}

#[allow(dead_code)]
fn _unused() {
    let _ = expand_path;
}
