# ssh-ops

Claude Code 通过 **WezTerm-SSH** (WezTerm fork) 进行 SSH 远程运维的 skill。

- **每个 CC 项目** 对应一个 WezTerm-SSH 窗口,**每台主机** 对应一个 pane
- pane 持有真 PTY,**字符级实时画面**,用户肉眼盯着看
- skill 通过 `WezTerm-SSH-cli send-text` 注入命令,通过 marker 切片回抓输出给 Claude
- 全程 **asciinema 录制**(命令级索引另存)
- 不写 `~/.ssh/config`、不缓存主机信息、所有 UI 在 WezTerm-SSH 内

> 状态:**Phase 1a**(MVP 主链可用,临时参数模式)。Phase 1b 起接入 SecureCRT。完整路线见 [`docs/Implementation_Plan.md`](docs/Implementation_Plan.md)。

> **关于 WezTerm vs WezTerm-SSH**: 本项目 `wezterm-src/` 是 WezTerm 的 fork, 编译并部署后产出三个改名后的二进制和独立 bundle, 跟原版 WezTerm **完全 namespace 隔离** (独立 socket / data / window class / bundle id), 可与官方 WezTerm 共存:
>
> | 部分 | WezTerm-SSH (本 fork) | 上游 WezTerm |
> |---|---|---|
> | macOS bundle | `~/Applications/WezTerm-SSH.app` | `/Applications/WezTerm.app` |
> | Bundle id | `com.wezterm-ssh.gui` | `com.github.wez.wezterm` |
> | GUI binary | `WezTerm-SSH` | `wezterm-gui` |
> | CLI 子命令 | `WezTerm-SSH-cli` | `wezterm` |
> | mux server | `WezTerm-SSH-mux` | `wezterm-mux-server` |
> | runtime dir | `~/.local/share/WezTerm-SSH/` | `~/.local/share/wezterm/` |
>
> ssh-ops 业务 CLI (`bin/sshops`) 跟 fork 的 GUI bundle 不冲突 — 前者是 PATH 里的 shell/Rust binary, 后者走 `~/Applications/WezTerm-SSH.app`.

## 安装

### 1. 系统依赖

```bash
# macOS — 注意 *不要* 装上游 wezterm cask, 本项目用 wezterm-src/ fork
brew install asciinema jq
brew install hudochenkov/sshpass/sshpass   # 可选,用 --password 才需要
brew install pass                          # 可选,密码后端

# Rust 工具链 (本地构建 wezterm-src fork 用)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 系统设置开启 "远程登录"(用于 self-test 的 ssh localhost)
# System Settings → General → Sharing → Remote Login
# 把当前用户的公钥加进 ~/.ssh/authorized_keys
```

> bash:**3.2 / 4 / 5 全部兼容**。macOS 系统自带 `/bin/bash` 3.2 即可,brew bash 5.x 也跑得通。空数组 + `set -u` 的 corner case 已显式守护。已实测 3.2.57 和 5.3.9。

### 2. 一键安装 (含 wezterm-src fork 构建 + 部署)

```bash
cd /Users/<u>/Code/ssh-ops
bash install.sh
```

`install.sh` 是**幂等**的, 反复跑只会跳过已就位项. 它会:

1. 检查 asciinema/jq/ssh/cargo 等系统依赖
2. **(macOS)** 跑 `wezterm-src/install-local.sh` 编译 + 部署 WezTerm-SSH bundle:
   - `cargo build --release` 三个核心 crate (wezterm/wezterm-gui/wezterm-mux-server, ~2 分钟初次编译)
   - 部署到 `~/Applications/WezTerm-SSH.app/Contents/MacOS/{WezTerm-SSH, WezTerm-SSH-cli, WezTerm-SSH-mux}`
   - ad-hoc codesign + `lsregister -f` 刷新 LaunchServices
   - `~/.local/bin/{WezTerm-SSH-cli, WezTerm-SSH-mux}` 部署 wrapper 脚本 (exec 进 .app, macOS 才能正确关联 .app icon)
3. 部署 skill 到 `~/.claude/skills/sshops/`(slash 命令是 `/sshops`)
   - `SKILL.md` 按系统 locale 选 description 翻译(31 种语言, 见 `skill-locales/descriptions.json`)
   - 不存在的 locale fallback 到英文
   - 业务文件 `bin/` `rust/` 等 symlink 到源仓库
4. 写入 PATH 到 `~/.zshenv` (`~/Code/ssh-ops/bin` + `~/.local/bin` 各自幂等)

完成后:

```bash
source ~/.zshenv          # 让 PATH 立即生效 (新开 shell 也行)
sshops setup              # 交互式向导写 config.json
open -a WezTerm-SSH       # 启动 GUI (双击 .app 同效果)
```

**Claude Code 中调用:**

```
/sshops <你的请求>          # 例: /sshops 帮我连 10.32.49.7 跑一下 uname
```

slash 命令名 `sshops` 跟 SKILL 目录名 + frontmatter `name` 一致。skill 列表
显示的描述按你的系统 locale 自动选择(macOS `defaults read -g AppleLocale`,
Linux `$LANG`)。

**安装可选项:**

```bash
bash install.sh --no-build-wezterm        # 跳过 wezterm 构建 (已部署或 Linux)
bash install.sh --link-only               # 只重链 skill (跳过依赖检查 + wezterm 构建)
bash install.sh --locale en               # 强制英文 description (默认: 系统 locale)
bash install.sh --link-only --locale ja   # 切日文 description, 不重编译
```

**多语言 description 维护:**

所有 31 种语言的 description 集中在 `skill-locales/descriptions.json`(英文 +
简繁中文 + 日韩法德西意葡俄乌等共 32 条 KV)。加新语言只需往 JSON 加一行,
正文 `SKILL.md` 是英文 master, 改正文只改一处.

### 3. 验证

```bash
bash tests/self-test.sh
```

通过则说明 marker 切片 + 录像 + state 写入主链全部 OK。

## 使用

### Claude 自动调用

在 Claude Code 对话里描述任务,Claude 据 [`SKILL.md`](SKILL.md) 决策何时使用本 skill。例:

> 帮我连一下 10.1.2.3,跑一下 uptime 看看负载,顺便录像。

Claude 会调:

```bash
sshops run --host 10.1.2.3 --user ec2-user --key ~/.ssh/aws.pem "uptime"
```

### CLI 直接用

```bash
# 跑一条短命令
sshops run --host 10.1.2.3 --user root --key ~/.ssh/k.pem "uptime"

# 仅 spawn pane,不跑命令(后续手敲)
sshops open --host 10.1.2.3 --user root --key ~/.ssh/k.pem

# 关 pane
sshops close --host 10.1.2.3 --user root --port 22

# 列当前项目所有 pane
sshops list-panes
```

JSON 输出:

```json
{
  "exit": 0,
  "duration_ms": 340,
  "cast_offset": 12.4,
  "session_id": "10.1.2.3-20260502-142301-a3f4b1",
  "dangerous": false,
  "blocked": false,
  "output": "14:23:01 up 47 days, load 0.5"
}
```

被拦截:

```json
{
  "exit": -1,
  "blocked": true,
  "dangerous": true,
  "reason": "...(pattern: rm\\s+-rf\\s+/)",
  "session_id": "...",
  "output": "(not executed)"
}
```

## 安全栏杆

内置危险命令模式拦截(`rm -rf /`、`reboot`、`mkfs`、`dd of=/dev/`、`shutdown`、`:(){`、`chmod -R 777 /` 等,可在 `config.json` 自定义)。

| 场景 | 行为 |
|---|---|
| 危险 + `--prod` 标志 + 无 `--i-mean-it` | **拒绝**,exit 5 |
| 危险 + `--prod` + `--i-mean-it` | 警告但放行 |
| 危险 + 非 prod | 警告但放行 |

`--i-mean-it` Claude 不会主动加,必须用户在对话里明确确认。

## 项目布局

```
ssh-ops/
├── SKILL.md               Claude 决策手册
├── README.md              本文档
├── bin/
│   ├── sshops             业务 CLI dispatcher (bash, 透传业务子命令到 Rust)
│   └── sshops-setup       初始化向导
├── rust/                  业务实现 (Rust + daemon 模式, 唯一主路径)
│   ├── core/              共享业务逻辑 (config / state / pane / wezterm_mux / safety / recorder ...)
│   ├── bin/               sshops-rs binary (短命 CLI)
│   └── daemon/            sshops-daemon (持久 IPC)
├── wezterm-src/           WezTerm fork → WezTerm-SSH (子目录, 独立 git 仓库)
│   └── install-local.sh   本地构建 + 部署 ~/Applications/WezTerm-SSH.app
├── cast-player/           Tauri 录像回放 GUI (独立)
├── recorder/              asciinema fork 备份 (现已切到 brew install asciinema)
├── config.example.json    配置模板
├── install.sh             依赖检测 + 调用 wezterm-src/install-local.sh + skill symlink
├── tests/self-test.sh     localhost echo 冒烟
└── docs/
    ├── PROJECT_OVERVIEW.md  架构总览
    └── Implementation_Plan.md  任务级状态
```

录像数据(运行时产生,不入库)默认放在**当前项目根的 `.ssh-ops/recordings/<session_id>/`**(跟项目绑定,clone/move 时一起带走;首次写录像时 skill 会自动给项目根 `.gitignore` 追加 `.ssh-ops/` 排除项)。

如果你希望**全局集中存储**(所有项目录到同一位置,适合统一审计):在 `config.json` 里设 `"log_dir": "~/.ssh-recordings"`(或任意路径),目录结构为 `<log_dir>/<project_slug>/<session_id>/`。

## 默认审计方案(无需改目标主机)

skill **不需要在目标主机新建账号**,默认方案完全基于已有的 SecureCRT 配置 + PS1 字符串审计 + asciinema 录像:

| 信号 | 值 |
|---|---|
| ssh 登录用户 | 来自 SecureCRT .ini(如 roy)|
| auto_sudo 切 root | 默认开启,目标主机需配 NOPASSWD 或 pane 内手输 sudo 密码 |
| 远端 PS1 prompt | `[root(roy:claude)@host ~]#` 或 `[roy(roy:claude)@host ~]$` |
| 录像 | 项目内 `.ssh-ops/recordings/<session-id>/`(stream.cast + commands.jsonl + meta.json) |

`(roy:claude)` 这个 prompt 信号意思是「**原 ssh 登录是 roy,当前操作者是 claude (AI)**」 — 谁查 asciinema 回放或站旁边看 pane 都能立即识别 AI vs 人的操作。生产环境通常没权限在目标主机新建账号,这套基于现有账号 + PS1 字符串审计 + 完整录像的方案就是标准做法。

---

## 路线图

- **Phase 1a** ✅ 临时参数 + marker + 录像 + 安全栏杆
- **Phase 1b** SecureCRT 接入(`@路径` / 关键词 / 跳板机 / SSH2.ini 全局回退)
- **Phase 2** `bg` `fan` `health` `sync` `grid` `push/pull` `forward`
- **Phase 3** 回放 / 搜索 / 标注 / retention gc
- **Phase 4** WezTerm Lua 视觉(AI/HUMAN/REPLAY 边框色 + 回放快捷键)

## 常见问题

**问:Claude 调 sshops 失败,提示 "WezTerm-SSH-cli 不通"。**
答:WezTerm-SSH GUI 是否启动?skill 自带 `open -a WezTerm-SSH` 重试 5 秒,失败说明 fork 未部署 — 跑 `bash wezterm-src/install-local.sh` 重新部署即可 (幂等)。

**问:命令注入超时(`exit 4`)。**
答:目标命令是 TUI(`vim` `top`)或长任务。Phase 2 用 `sshops bg`,目前先 `sshops close` + 手敲 SecureCRT。

**问:`exit 3` shell 不支持。**
答:目标主机的登录 shell 必须是 bash 或 zsh。fish/tcsh/网络设备 CLI 都不行。

**问:`exit 5` 被拦截。**
答:危险命令在生产机被拦截。如确实要执行,在对话里明确告诉 Claude "我确认要在生产机执行 X",Claude 才会加 `--i-mean-it`。

**问:bash 版本要求?**
答:**3.2 / 4 / 5 全兼容**。macOS 默认 `/bin/bash 3.2` 够用,brew `bash 5.x` 也跑。代码用 `[[ ${#arr[@]} -gt 0 ]]` 显式守护空数组的 `set -u` corner case。已实测 3.2.57(macOS Sonoma 自带)和 5.3.9(Homebrew)。

## 文档

- [`SKILL.md`](SKILL.md) — 给 Claude 的决策手册
- [`ssh-ops-requirements.md`](ssh-ops-requirements.md) — 完整需求规格
- [`docs/PROJECT_OVERVIEW.md`](docs/PROJECT_OVERVIEW.md) — 架构总览
- [`docs/Implementation_Plan.md`](docs/Implementation_Plan.md) — 任务级状态
- [`CLAUDE.md`](CLAUDE.md) — 项目编码约束
