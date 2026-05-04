//! pane 生命周期: ensure_window / open / close / wait_login_complete
//!
//! 移植自 lib/project.sh — 复用 wezterm pane 或 spawn 新 tab + cast-recorder 包 ssh,
//! 智能等 ssh 登录 (检测 password / passphrase / yes/no prompt), auto_sudo 切 root, 注入 PS1.

use crate::config::Config;
use crate::recorder::{build_recorder_argv, Recorder};
use crate::session::strip_ansi;
use crate::state::{project_id, project_slug, StateStore};
use crate::wezterm_mux::WezTermClient;
use crate::{Error, Result};
use crate::state::DEFAULT_SESSION_KEY;
use regex::Regex;
use std::path::Path;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct OpenedPane {
    pub pane_id: u64,
    pub session_id: String,
    pub recorder: Recorder,
    pub reused: bool,
}

#[derive(Debug, Clone)]
pub struct SshTarget {
    pub user: String,
    pub host: String,
    pub port: u16,
    pub key: Option<std::path::PathBuf>,
    /// auth_type: "key" 或 "password"
    pub auth_type: String,
    /// 仅 password 模式: sshpass 喂入用
    pub password: Option<String>,
}

/// pane_open: 复用现有 pane, 否则 spawn 新 tab 跑 cast-recorder + ssh
///
/// 返回 OpenedPane (含 pane_id / session_id / recorder).
pub fn pane_open(
    cfg: &Config,
    sshops_home: &Path,
    wez: &WezTermClient,
    state: &StateStore,
    session_key: &str,
    selector: &str,
    target: &SshTarget,
) -> Result<OpenedPane> {
    let (just_started, initial_panes) = wez.ensure_running()?;

    let pid = project_id();

    // 已存在? alive 则复用
    if let Some(existing) = state.get_pane(&pid, session_key, selector)? {
        if wez.pane_alive(existing.pane_id) {
            let recorder = Recorder::open(cfg, &existing.session_id)?;
            return Ok(OpenedPane {
                pane_id: existing.pane_id,
                session_id: existing.session_id,
                recorder,
                reused: true,
            });
        }
        // pane 失效, 清掉
        state.remove_pane(&pid, session_key, selector)?;
    }

    // 录像准备
    let sid = Recorder::make_session_id(&target.host);
    let recorder = Recorder::init(cfg, &sid, selector, &target.host, &target.user, &target.auth_type)?;

    // ssh argv
    let mut ssh_argv: Vec<String> = vec!["ssh".into()];
    for opt in &cfg.ssh_options_base {
        ssh_argv.push(opt.clone());
    }
    if target.port != 22 {
        ssh_argv.push("-p".into());
        ssh_argv.push(target.port.to_string());
    }
    if let Some(k) = &target.key {
        ssh_argv.push("-i".into());
        ssh_argv.push(k.to_string_lossy().into_owned());
    }
    ssh_argv.push(format!("{}@{}", target.user, target.host));

    // password 模式: sshpass 包一层
    if target.auth_type == "password" {
        if let Some(pw) = &target.password {
            if which::which("sshpass").is_err() {
                return Err(Error::Other("sshpass 未安装, 密码模式不可用".into()));
            }
            let mut wrapped = vec!["sshpass".into(), "-p".into(), pw.clone()];
            wrapped.extend(ssh_argv);
            ssh_argv = wrapped;
        }
    }

    // cast-recorder argv
    let rec_argv = build_recorder_argv(sshops_home, &recorder.cast_path, &ssh_argv);
    let rec_argv_ref: Vec<&str> = rec_argv.iter().map(|s| s.as_str()).collect();

    // spawn pane: 该 session 是否已有窗口?
    //   有且 alive → spawn_tab_in_window 在该窗口加新 tab
    //   无 / 失效  → 第一次该 session, 强制 spawn_new_window 隔离
    //   特例: session_key == DEFAULT_SESSION_KEY 时退回旧行为 (复用 active window)
    let cwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "/".into());

    let existing_window = state
        .session_state(&pid, session_key)?
        .map(|s| s.wezterm_window_id)
        .filter(|w| *w > 0)
        .filter(|w| wez.window_alive(*w));

    let pane_id = if let Some(win) = existing_window {
        // 该 Claude session 已有 wezterm 窗口, 在里面加新 tab
        wez.spawn_tab_in_window(win, &cwd, &rec_argv_ref)?
    } else if session_key == DEFAULT_SESSION_KEY {
        // 无 session_key (非 wezterm 环境等), 退回旧行为: 用当前 active window
        wez.spawn_tab(&cwd, &rec_argv_ref)?
    } else {
        // 第一次该 session: 强制开新窗口
        wez.spawn_new_window(&cwd, &rec_argv_ref)?
    };

    // 写 window_id 到 state (按 session_key 隔离)
    let opened_window = wez.window_of_pane(pane_id);
    if let Some(win) = opened_window {
        state.set_window(&pid, session_key, win)?;
    }

    // 如果 wezterm 是我们刚启动的, 清理它自启的默认空窗口
    // 严格安全: 只 kill 既不是我们刚开的窗口, 也不在 panes.json 任何项目/session 里的 pane
    // 这样 ssh-ops / test2 / 其他项目的旧 pane 一律不动, 只关 wezterm 自启的默认 shell.
    if just_started {
        let known: std::collections::HashSet<u64> = state
            .read()
            .map(|st| {
                st.projects
                    .values()
                    .flat_map(|p| p.sessions.values())
                    .flat_map(|s| s.panes.values())
                    .map(|info| info.pane_id)
                    .collect()
            })
            .unwrap_or_default();
        for p in &initial_panes {
            if Some(p.window_id) == opened_window {
                continue; // 我们刚开的窗口, 不动
            }
            if known.contains(&p.pane_id) {
                continue; // 任何项目/session 记录过, 不动 (跨项目保护)
            }
            // 既不在我们刚开窗口, 也无任何项目记录 → wezterm 自启的 default, 关掉
            let _ = wez.kill_pane(p.pane_id);
        }
    }

    // tab 标题
    let _ = wez.set_tab_title(pane_id, &target.host);
    recorder.set_pane_id(pane_id)?;

    // 等 ssh 登录: 用 cast 文件实时检测 prompt 出现 (取代固定 sleep 3s)
    // 如果中途出现 password/yes-no, 内部 sleep 2s 等用户在 wezterm 输入
    let login_timeout = std::env::var("SSHOPS_LOGIN_TIMEOUT")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(120);
    if !wait_for_login_in_cast(wez, pane_id, &recorder, Duration::from_secs(login_timeout), "ssh 登录") {
        let _ = wez.kill_pane(pane_id);
        let _ = recorder.finalize();
        return Err(Error::Timeout(format!(
            "ssh 登录超时 ({login_timeout}s)"
        )));
    }

    // auto_sudo: 非 root 时 sudo -i (砍掉 sleep 1s)
    let mut sudo_active = false;
    if cfg.auto_sudo && !cfg.auto_sudo_skip_users.iter().any(|u| u == &target.user) {
        tracing::info!("auto_sudo: {} → root via sudo -i", target.user);
        let pre_sudo_size = recorder.cast_size();
        wez.send_text(pane_id, "sudo -i\r")?;
        if wait_for_login_in_cast_after(wez, pane_id, &recorder, pre_sudo_size, Duration::from_secs(60), "sudo -i") {
            sudo_active = true;
        } else {
            tracing::warn!("sudo -i 超时, 后续命令在原 user shell 跑");
        }
    }

    // PS1 注入 + OSC 1337 SetUserVar (sshops_project 给 wezterm format-window-title 用)
    // OSC 1337 必须由 pane 内的 shell 输出, wezterm 才能感知 (从外部 send-text 没用)
    use base64::{engine::general_purpose, Engine};
    let proj_b64 = general_purpose::STANDARD.encode(project_slug());
    let sid_b64 = general_purpose::STANDARD.encode(&sid);
    let ai_b64 = general_purpose::STANDARD.encode("ai");
    // 一条命令搞定: 设 PS1 + 输出 OSC 三条 (项目/actor/session) + clear
    let osc = format!(
        "printf '\\033]1337;SetUserVar=sshops_project={proj_b64}\\007'; \
         printf '\\033]1337;SetUserVar=sshops_actor={ai_b64}\\007'; \
         printf '\\033]1337;SetUserVar=sshops_session_id={sid_b64}\\007'"
    );
    let real_user = "claude";
    let login_label = if sudo_active { "root" } else { target.user.as_str() };
    let ps1_cmd = if login_label == real_user {
        format!(
            "export REAL_USER='{real_user}'; export PS1='[\\u@\\h \\W]\\$ '; {osc}; clear\r"
        )
    } else {
        format!(
            "export REAL_USER='{real_user}'; export PS1='[\\u({login_label}:$REAL_USER)@\\h \\W]\\$ '; {osc}; clear\r"
        )
    };
    wez.send_text(pane_id, &ps1_cmd)?;

    // 持久化
    state.add_pane(&pid, session_key, selector, pane_id, &sid)?;

    Ok(OpenedPane {
        pane_id,
        session_id: sid,
        recorder,
        reused: false,
    })
}

pub fn pane_close(
    cfg: &Config,
    wez: &WezTermClient,
    state: &StateStore,
    session_key: &str,
    selector: &str,
) -> Result<()> {
    let pid = project_id();
    if let Some(info) = state.get_pane(&pid, session_key, selector)? {
        let _ = wez.kill_pane(info.pane_id);
        if let Ok(rec) = Recorder::open(cfg, &info.session_id) {
            let _ = rec.finalize();
        }
    }
    state.remove_pane(&pid, session_key, selector)?;
    Ok(())
}

/// 用 cast 文件实时检测 ssh 登录完成 (出现 shell prompt) 或需要用户输入
/// 比 wait_for_input_complete 快: 不依赖 wezterm cli get-text fork, 直接读 cast 字节
pub fn wait_for_login_in_cast(
    wez: &WezTermClient,
    pane_id: u64,
    recorder: &crate::recorder::Recorder,
    timeout: Duration,
    ctx: &str,
) -> bool {
    wait_for_login_in_cast_after(wez, pane_id, recorder, 0, timeout, ctx)
}

/// 同上, 但只看 start_byte 之后的 cast 内容 (用于 sudo 后等新 prompt)
pub fn wait_for_login_in_cast_after(
    wez: &WezTermClient,
    pane_id: u64,
    recorder: &crate::recorder::Recorder,
    start_byte: u64,
    timeout: Duration,
    ctx: &str,
) -> bool {
    use crate::incremental_parser::SessionParser;
    let re_input = Regex::new(
        r"(?i)([Pp]assword|[Pp]assphrase|\[sudo\]\s+[Pp]assword|[Vv]erification code|[Cc]ode):\s*$|\(yes/no(?:/\[fingerprint\])?\)\?\s*$|[Aa]re you sure you want to",
    ).expect("regex compile");

    let deadline = Instant::now() + timeout;
    let poll = Duration::from_millis(50);
    let mut parser = SessionParser::new(start_byte);
    let mut notified = false;

    while Instant::now() < deadline {
        let cur = recorder.cast_size();
        if cur > parser.cursor() {
            let _ = parser.poll_until(recorder, cur);
        }
        let tail = parser.out_tail_str();

        // 见到 password/yes-no prompt: 让用户在 wezterm 手输, 等 2s 再查
        if re_input.is_match(&tail) {
            if !notified {
                tracing::info!(
                    "{ctx} 在等用户输入, 在 WezTerm pane {pane_id} 完成输入 (timeout={}s)",
                    timeout.as_secs()
                );
                notified = true;
            }
            std::thread::sleep(Duration::from_secs(2));
            continue;
        }

        // 见到 shell prompt → ssh 登录完成
        if parser.has_prompt_at_end() {
            return true;
        }

        // 还没数据, 继续等
        std::thread::sleep(poll);
    }
    false
}

/// 智能轮询 pane 末尾, 直到不再出现 password/passphrase/yes-no prompt
/// 返回 true=已完成, false=超时
/// (旧 API, 还未删除以备 fallback)
#[allow(dead_code)]
pub fn wait_for_input_complete(
    wez: &WezTermClient,
    pane_id: u64,
    timeout: Duration,
    ctx: &str,
) -> bool {
    let re_input = Regex::new(
        r"(?i)([Pp]assword|[Pp]assphrase|\[sudo\]\s+[Pp]assword|[Vv]erification code|[Cc]ode):\s*$|\(yes/no(?:/\[fingerprint\])?\)\?\s*$|[Aa]re you sure you want to",
    )
    .expect("regex compile");

    let deadline = Instant::now() + timeout;
    let mut notified = false;
    while Instant::now() < deadline {
        // 只看当前屏幕 (最快)
        let raw = wez.get_text(pane_id, None).unwrap_or_default();
        let stripped = strip_ansi(&raw);
        let tail: String = stripped.lines().rev().take(5).collect::<Vec<_>>().join("\n");
        if re_input.is_match(&tail) {
            if !notified {
                tracing::info!(
                    "{ctx} 在等用户输入, 在 WezTerm pane {pane_id} 完成输入 (timeout={}s)",
                    timeout.as_secs()
                );
                notified = true;
            }
            std::thread::sleep(Duration::from_secs(2));
            continue;
        }
        return true;
    }
    false
}
