# ssh-ops 实施计划

> 任务级状态。架构层变更去 `PROJECT_OVERVIEW.md`。需求层变更去 `../ssh-ops-requirements.md`。

状态图例:`[ ]` 待办 / `[~]` 进行中 / `[x]` 完成 / `[!]` 阻塞 / `[-]` 砍掉。

---

## Phase 1a · MVP 主链 ✅ 实施完成

**目标**:打通"通过 WezTerm 跑命令并录"端到端,先在临时参数路径上验证 marker 切片这个技术心脏。

### 0. 骨架与文档

- [x] git init + .gitignore
- [x] docs/PROJECT_OVERVIEW.md
- [x] docs/Implementation_Plan.md
- [x] config.example.json
- [x] README.md(用户安装与使用文档)
- [x] 同步 A/B/C/D 决策回 `ssh-ops-requirements.md`(第 3.1 / 5.4 / 7.1 / 8.3 / 13 / 16 / 18 / 19 / 20 节)

### 1. 依赖与安装

- [x] `install.sh` 检测 wezterm/asciinema/jq/sshpass/ssh/perl/xxd,缺则报错给 brew 命令
- [x] `bin/sshops-setup` 极简向导(交互式写 `config.json`)
- [x] 链接到 `~/.claude/skills/ssh-ops/`(install.sh 的 ln -s)

### 2. 基础库

- [x] `lib/common.sh`:配置加载、log_*、`gen_nonce`、`strip_ansi`、`ssh_opts_for`、`state_with_lock`(mkdir 锁)、`now_ms`、`expand_path`、`project_id/slug`
- [x] `lib/safety.sh`:`is_prod_host`、`is_dangerous_cmd`、`safety_gate`(含 `--i-mean-it`)
- [x] `lib/wezterm.sh`:`wt_check`(自动启动)、`wt_list_json`、`wt_spawn_new_window`、`wt_split_pane`、`wt_send_text`、`wt_get_text`、`wt_kill_pane`、`wt_set_user_var`、`wt_pane_alive`、`wt_window_of_pane`
- [x] `lib/marker.sh`:`marker_inject_and_capture`(BEGIN/END + awk 精确行匹配 + ANSI strip + 30s 超时)、`marker_wait_for_text`
- [x] `lib/project.sh`:`panes_state_*`(锁内读改写)、`pane_ensure_window`、`pane_open`(spawn → 等 prompt → shell 检测 → 设 PS1)、`pane_close`、`panes_cleanup_stale`
- [x] `lib/recorder.sh`:`record_init`、`record_set_pane`、`record_cast_offset`、`record_append_command`、`record_finalize`

### 3. 命令面

- [x] `bin/sshops open`:临时参数 spawn + 等 prompt + shell 检测
- [x] `bin/sshops run`:`open`(若需) + 注入 + 切片 + 输出 JSON + 写 commands.jsonl
- [x] `bin/sshops close`:kill pane + finalize 录像
- [x] `bin/sshops list-panes`:从 state 列当前项目所有 pane

### 4. Claude 决策手册

- [x] `SKILL.md` 极简版(触发条件 + 命令决策树 + 危险命令处理 + Phase 1a 限制)

### 5. 自检

- [x] `tests/self-test.sh`:`ssh localhost echo hello`,验证 marker 切片 + JSON 解析 + 录像三件套 + cast header

### 6. 提交

- [x] git commit Phase 1a(commit `9cd0881`)

### Phase 1a 验证状态(待真实环境)

代码层 ✅ 全部通过 `bash -n` 语法检查。**功能层验证待 Bjarne 装好 WezTerm + asciinema + bash 4+ 后跑 self-test**。

| 验证点 | 状态 |
|---|---|
| WezTerm cli 自动启动 | [ ] 待 wezterm 装好 |
| `ssh localhost echo` 基线 | [ ] 待 macOS 远程登录开启 |
| marker 切片(普通输出) | [ ] 待 self-test |
| marker 切片(ANSI 颜色输出) | [ ] 待加测试用例 |
| marker 切片(命令含 marker 字符串) | [ ] 待加测试用例 |
| 录像三件套生成 | [ ] 待 self-test |
| `cast_offset` ↔ `commands.jsonl.ts` 时间一致 | [ ] 待加测试用例 |
| 危险命令拦截(prod) | [ ] 待加测试用例 |
| 非支持 shell 拒绝 | [ ] 待加测试用例(模拟 fish) |
| 多 CC 项目分窗口 | [ ] 手动验证 |
| state 并发安全(mkdir 锁) | [ ] 待加测试用例 |

self-test 当前只覆盖第 1-3 行,其余 Phase 1a 完整测试矩阵见 `requirements.md` 第 18.1 节,后续按需扩充 `tests/self-test.sh` 或拆出多个 `.sh`。

### Phase 1a 风险点

按 self-test 跑通前的风险等级排序:

1. **🔴 marker 切片在真实 PTY 下的鲁棒性**:精确行匹配 `^__SSHOPS_BEGIN_<nonce>__$`,但 PTY 折行 / CR-LF / OSC 序列可能让"孤行"破裂。整个 Phase 1a 最高风险点。一旦 self-test 第一条 echo 跑通,基本可以下调到中等。
2. **🟡 `pane_open` 的 `sleep 3`** 是给 ssh 认证留余量,粗暴但够 Phase 1a;真正鲁棒做法是先 `marker_wait_for_text` 等任意原始 shell prompt 出现。看 self-test 是否稳。
3. **🟡 `asciinema rec --command`** 把 ssh argv 用 `printf %q` 转义后塞给 shell 解析,常规用法 OK,但如果 ssh argv 含 `$` `\`等极端字符理论上有 corner case。
4. **🟢 macOS bash 3.2 兼容**:已要求 brew bash 4+,install.sh 检测拒绝。无风险但用户可能没装。
5. **🟢 中文路径与空格**:已确认大富的 SecureCRT Config 路径含中文(`安全工具`)和空格,所有路径处理已 quote。Phase 1b 解析 .ini 时复测。

---

## Phase 1b · SecureCRT 接入

按 `requirements.md` 第 19 节 Phase 1b 清单。

### MVP 已完成(commit 待确定)

- [x] `lib/crt.sh`:.ini 解析(Hostname / Username / Port hex+decimal 兼容 / Identity / PublicKey / Firewall Name / Protocol Name)
- [x] SSH2.ini 全局 Identity / PublicKey 回退
- [x] `${VDS_CONFIG_PATH}` 路径变量展开
- [x] `path_mappings`(Windows → macOS)转换
- [x] 协议过滤:仅 SSH2,Telnet/SSH1/Serial/Rlogin 拒绝
- [x] `.ppk` 检测报错(不自动转换)
- [x] `lib/selector.sh`:`@path` / 关键词 / 临时参数 归一(三入口优先级)
- [x] 生产机判定接入(从 `@<path>` 提取 prod 关键词)
- [x] **新增:Password V2 检测**:.ini 是密码登录(无 Identity + 有 Password V2)时,
      给清晰错误提示三种方案(--ask-password / SecureCRT 改 key / Phase 2 password_refs)
- [x] SKILL.md 完整版(主机解析三入口、prod 判定、`.ppk` 坑、密码登录处理)
- [x] 单元测试:`/Users/bjarne/Work/安全工具/SecureCRT/Config/Sessions/10.32.49.7.ini`
      实际解析,Port hex 解码 / 全局回退 / Password V2 检测全部正确

### Phase 1b 剩余(待后续)

- [ ] 跳板机递归 ≤3 层 + 循环引用检测,组装 `-J` 参数(MVP 仅警告不阻塞)
- [ ] `sshops list <pattern>`、`sshops show <selector>`、`sshops crt-find <kw>`
- [ ] 完整测试用例:`requirements.md` 第 18.2 节(跳板机 / 协议拒绝 / Windows 路径映射)
- [ ] 端到端验证:在真实主机 (10.32.49.7) 跑 `sshops run @10.32.49.7 "uptime"`

---

## Phase 2 · 进阶命令

按 `requirements.md` 第 19 节 Phase 2 清单。略(开 Phase 2 时再细化)。

---

## Phase 3 · 录像系统完善

略。

---

## Phase 4 · WezTerm Lua 视觉

略。

---

## Phase B · Rust 重写 (✅ 实施完成)

**目标**:把 bash 实现移植到 Rust,消除 jq/python/awk fork 开销,把命令 round-trip 从 1064ms 降到 300ms 以下。

### Day 1 — 工作区骨架
- [x] `rust/Cargo.toml` workspace + members [`core`, `bin`]
- [x] `core` 模块声明 + 公共类型 (ExecuteRequest/Response)

### Day 2-3 — SecureCRT + selector
- [x] `core/src/securecrt.rs`:CrtParser, .ini 解析 (S:/D:/B: 字段, hex/dec port, Identity 三级回退)
- [x] `core/src/selector.rs`:resolve_crt (@path 精确 / 模糊匹配), resolve_tmp

### Day 4-5 — pane_open + cmd_run
- [x] `core/src/pane.rs`:pane_open / pane_close / wait_for_input_complete (登录智能等待)
- [x] `core/src/recorder.rs`:make_session_id / init / cast_size / extract_output / append_command / finalize / read_last_ai_byte
- [x] `core/src/state.rs`:StateStore (项目级, fs2 file lock + atomic write, 兼容 bash 版 panes.json schema)
- [x] `core/src/safety.rs`:regex-based safety_gate
- [x] `core/src/session.rs`:byte-offset 切片 + cast prompt 检测
- [x] `core/src/wezterm_mux.rs`:完整 PaneEntry + ensure_running + active_window
- [x] `core/src/config.rs`:对齐 bash 版 config.json schema
- [x] `core/src/human_detect.rs`:extract_human_commands

### Day 6-7 — 完整 CLI
- [x] `bin/src/main.rs`:run / open / close / peek / list-panes / recent 全实现
- [x] 三入口: SecureCRT @path / 模糊关键词 / --host --user 临时模式
- [x] 100% 兼容 bash 版 JSON 输出 (含 recent_human_activity)
- [x] 12 个单测全过

### Day 8 — benchmark
- [x] release build (LTO + opt-level 3 + strip): `cargo build --release` 通过
- [x] 实测 echo round-trip: 530ms → 300ms (poll 100ms→50ms)
- [x] 对比基线: 比 bash 优化版快 3.5x, 比原始 bash 快 5-7x

### Day 9 — bash wrapper 切换
- [x] `bin/sshops` 检测 `rust/target/release/sshops-rs` 存在则透传 run/open/close/peek/list-panes/recent
- [x] setup/version/help 仍走 bash
- [x] `SSHOPS_NO_RUST=1` 强制 fallback

**性能数据**:
| 版本 | round-trip | 比例 |
|---|---|---|
| bash 原始 | 1500-2000ms | 1.0x |
| bash 优化版 (cast.sock) | 1064ms | 1.5x |
| Rust v1 (100ms poll) | 530ms | 3.0x |
| Rust v2 (50ms poll) | **300ms** | **5-7x** |

后续优化方向 (Phase C):
- inotify/kqueue 替代 polling
- wezterm cli fork → mux socket 直连
- daemon 模式 (避免每次 Rust 二进制重启 ~50ms)

---

## Phase C · daemon 模式 + 性能终态 (✅ 实施完成)

**目标**: 引入持久 daemon, 消除每次 binary 启动开销, 加速 polling, 把 round-trip 从 300ms 压到 < 200ms.

### C1 — IPC 协议 + daemon 骨架
- [x] `core/src/ipc.rs`: IpcRequest/Response 枚举, length-prefix bincode codec, proto v1
- [x] `daemon/` crate: tokio multi-thread, accept → handler 路由 → spawn_blocking
- [x] `daemon/src/handler.rs`: 6 个子命令路由 + DaemonState
- [x] `bin/main.rs --no-daemon` flag, 默认 IPC 优先 + fallback in-process
- [x] 新子命令: daemon-status / daemon-stop

### C2 — Auto-spawn + 单实例锁
- [x] `daemon` flock state/daemon.pid (Box::leak 保活), 已存在直接退
- [x] `--detach`: stderr → state/daemon.log
- [x] cli auto-spawn: socket 不存在/connect 失败 → 清 stale socket → spawn → 等 1s
- [x] SSHOPS_NO_AUTOSPAWN=1 关闭

### C3 — wait_prompt+stable 算法优化 (实际收益最大)
- [x] `session.rs`: 拆掉 wait_cast_stable, 合并到 wait_prompt_and_stable
- [x] 算法: 见 prompt 后等 1 个无变化窗口 (而非 50ms × 3)
- [x] poll: 50ms → 10ms

### C4 — idle 自动 shutdown
- [x] 默认 30min 无请求自动退 (SSHOPS_DAEMON_IDLE_SECS 覆盖)
- [x] 60s 心跳检查
- [x] 退出清理 socket

**最终性能数据**:

| 版本 | server_dur | wall | vs 基线 |
|---|---|---|---|
| bash 原始 | - | 1500-2000ms | 1.0x |
| bash 优化版 (cast.sock) | - | 1064ms | 1.5x |
| Rust Phase B (in-proc, 100ms poll) | - | 530ms | 3.0x |
| Rust Phase B (in-proc, 50ms poll) | - | 300ms | 5-7x |
| **Rust Phase C (daemon + 10ms poll)** | **81-144ms** | **113-178ms** | **10-18x** |

**剩余不可消除的开销**:
- pane_open (reused) ~22ms = 一次 wezterm cli list fork
- send_text ~14ms = 一次 wezterm cli send-text fork
- wait_prompt+stable ~70ms = 等 ssh round-trip + 1 个稳定窗口
- IPC + binary 启动 ~30ms

**未来优化方向 (Phase D)**:
- WezTerm mux socket 直连 → 砍 ~36ms (pane_open + send_text)
- inotify/kqueue cast 文件事件 → 砍 ~50ms (wait_prompt 部分)
- OSC 133 marker 协议 → 砍 ~50ms + 修真实 exit code

理论极限: < 30ms (跟 ssh 原生 round-trip 同量级).

---

## 变更日志

- 2026-05-02 创建,Phase 1a 启动
- 2026-05-02 Phase 1a 实施完成(commit `9cd0881`,17 文件 / 3054 行);文档完善:README.md 创建、PROJECT_OVERVIEW 关键不变量补全、requirements 第 18/20 节补全
- 2026-05-03 Phase B Rust 重写完成 (Day 1-9): 9 个 core 模块 + 完整 CLI + bash wrapper 透传; 性能 1064ms → 300ms (3.5x); 12 个单测全过
- 2026-05-03 Phase C daemon 模式完成 (C1-C4): IPC + auto-spawn + wait 算法优化 + idle shutdown; 性能 300ms → 113-178ms (2-3x); 对原始 bash 10-18x 提速
- 2026-05-04 WezTerm fork 改名 WezTerm-SSH 落地: 完整 namespace 隔离 (compute_runtime_dir/data_dir/cache_dir 全改 → ~/.local/share/WezTerm-SSH/; DEFAULT_WINDOW_CLASS → org.wezterm-ssh.gui; toast id / Windows mutex / bundle id 同步; macOS bundle WezTerm.app → WezTerm-SSH.app), Cargo crate 名不动, 改名只在 cp 阶段 (ci/deploy.sh + 新增 wezterm-src/install-local.sh); ssh-ops 端 lib/wezterm.sh + rust/core/{wezterm_mux,preflight}.rs + config 默认 cli_path 切到 WezTerm-SSH-cli; install.sh 接管 wezterm-src 本地构建+部署, 全幂等; 端到端 sshops open + run 验证通过 (69ms 返回 JSON); README + PROJECT_OVERVIEW 同步
