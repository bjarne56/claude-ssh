//! IPC 客户端: 连 daemon, 失败时返回 None 让调用方 fallback in-process

use anyhow::Result;
use ssh_ops_core::ipc::{
    self, ClientCtx, IpcRequest, IpcResponse, SelectorSpec, PROTO_VERSION,
};
use std::path::Path;
use std::time::Duration;
use tokio::net::UnixStream;
use tokio::time::timeout;

pub fn build_ctx(home: &Path) -> ClientCtx {
    ClientCtx {
        sshops_home: home.to_path_buf(),
        project_id: ssh_ops_core::state::project_id(),
        proto: PROTO_VERSION,
        session_key: ssh_ops_core::state::current_session_key(),
    }
}

/// 同步包装: 一次性 IPC roundtrip. 用 current_thread tokio runtime, 不污染主进程.
/// 返回 Ok(None) = daemon 不可达 + 启动也失败, 调用方 fallback.
/// 返回 Ok(Some(resp)) = 拿到响应.
/// 返回 Err = 协议错误 / IO 严重错误.
///
/// 流程:
/// 1. socket 不存在 → 尝试 auto-spawn daemon (除非 SSHOPS_NO_AUTOSPAWN=1)
/// 2. 等 socket 出现 (max 1s)
/// 3. connect + 收发
/// 4. 任一步失败 → 返回 None
pub fn call_sync(home: &Path, req: IpcRequest) -> Result<Option<IpcResponse>> {
    let sock = ipc::default_sock_path(home);
    let no_spawn = std::env::var("SSHOPS_NO_AUTOSPAWN").as_deref() == Ok("1");
    let timing = std::env::var("SSHOPS_DEBUG_TIMING").as_deref() == Ok("1");
    let log = |name: &str, dt: f64| {
        if timing {
            eprintln!("[TIMING] cli {name:>22}: {dt:>7.2}ms");
        }
    };

    let t_rt = std::time::Instant::now();
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    log("rt build", t_rt.elapsed().as_micros() as f64 / 1000.0);

    rt.block_on(async {
        // 第一次尝试: 直连
        if sock.exists() {
            let t_conn = std::time::Instant::now();
            if let Ok(Ok(mut stream)) =
                timeout(Duration::from_millis(200), UnixStream::connect(&sock)).await
            {
                log("connect", t_conn.elapsed().as_micros() as f64 / 1000.0);
                let t_w = std::time::Instant::now();
                ipc::write_msg(&mut stream, &req).await?;
                log("write+encode", t_w.elapsed().as_micros() as f64 / 1000.0);
                let t_r = std::time::Instant::now();
                let resp: IpcResponse = ipc::read_msg(&mut stream).await?;
                log("read+decode (含服务端处理)", t_r.elapsed().as_micros() as f64 / 1000.0);
                return Ok(Some(resp));
            }
            // socket 残留但 connect 失败 → daemon 已死, 清理 stale socket
            let _ = std::fs::remove_file(&sock);
        }
        if no_spawn {
            return Ok(None);
        }
        // auto-spawn daemon
        let t_spawn = std::time::Instant::now();
        if try_spawn_daemon(home).is_err() {
            return Ok(None);
        }
        log("spawn daemon", t_spawn.elapsed().as_micros() as f64 / 1000.0);
        // 等 socket 出现 + connect 成功, max 1s
        for _ in 0..20 {
            if sock.exists() {
                if let Ok(Ok(mut stream)) =
                    timeout(Duration::from_millis(200), UnixStream::connect(&sock)).await
                {
                    let t_w = std::time::Instant::now();
                    ipc::write_msg(&mut stream, &req).await?;
                    log("write+encode", t_w.elapsed().as_micros() as f64 / 1000.0);
                    let t_r = std::time::Instant::now();
                    let resp: IpcResponse = ipc::read_msg(&mut stream).await?;
                    log("read+decode (含服务端处理)", t_r.elapsed().as_micros() as f64 / 1000.0);
                    return Ok(Some(resp));
                }
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        Ok(None)
    })
}

/// fork + exec sshops-daemon --detach. 找 daemon 二进制的策略:
/// 1. 跟 sshops-rs 同目录 (cargo build --release 后两者都在 target/release/)
/// 2. $SSHOPS_HOME/rust/target/release/sshops-daemon
/// 3. PATH 里的 sshops-daemon
fn try_spawn_daemon(home: &Path) -> Result<()> {
    let candidates: Vec<std::path::PathBuf> = {
        let mut v = Vec::new();
        if let Ok(exe) = std::env::current_exe() {
            if let Some(parent) = exe.parent() {
                v.push(parent.join("sshops-daemon"));
            }
        }
        v.push(home.join("rust/target/release/sshops-daemon"));
        v
    };
    let bin = candidates
        .iter()
        .find(|p| p.exists())
        .cloned()
        .unwrap_or_else(|| std::path::PathBuf::from("sshops-daemon"));
    // double fork: 让 daemon 完全脱离 cli 进程组 (避免 cli 退出时 SIGHUP 杀 daemon)
    use std::process::{Command, Stdio};
    Command::new(&bin)
        .arg("--detach")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| anyhow::anyhow!("spawn daemon {}: {e}", bin.display()))?;
    Ok(())
}

/// 把 cli 端 (host/user/port/key/prod/auth/password) 选项打包成 SelectorSpec.
/// args[0] 是 selector 字符串 (SecureCRT 模式).
pub fn build_selector_spec(
    args: &[String],
    host: Option<&str>,
    user: Option<&str>,
    port: u16,
    key: Option<&Path>,
    prod: bool,
    password: Option<&str>,
    ask_password_used: bool,
) -> Result<SelectorSpec> {
    if let (Some(h), Some(u)) = (host, user) {
        let auth_type = if password.is_some() || ask_password_used {
            "password".to_string()
        } else {
            "key".into()
        };
        return Ok(SelectorSpec::Tmp {
            user: u.into(),
            host: h.into(),
            port,
            key: key.map(|p| p.to_path_buf()),
            prod,
            auth_type,
            password: password.map(String::from),
        });
    }
    if args.is_empty() {
        anyhow::bail!("缺 selector");
    }
    Ok(SelectorSpec::Crt(args[0].clone()))
}

pub fn cmd_text_from(args: &[String], using_tmp: bool) -> Result<String> {
    if using_tmp {
        if args.is_empty() {
            anyhow::bail!("缺命令文本");
        }
        return Ok(args.join(" "));
    }
    if args.len() < 2 {
        anyhow::bail!("缺命令文本");
    }
    Ok(args[1..].join(" "))
}
