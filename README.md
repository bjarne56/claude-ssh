# ssh-ops

Claude Code 通过 WezTerm 进行 SSH 远程运维的 skill。

- **每个 CC 项目** 对应一个 WezTerm 窗口,**每台主机** 对应一个 pane
- WezTerm pane 持有真 PTY,**字符级实时画面**,用户肉眼盯着看
- skill 通过 `wezterm cli send-text` 注入命令,通过 marker 切片回抓输出给 Claude
- 全程 **asciinema 录制**(命令级索引另存)
- 不写 `~/.ssh/config`、不缓存主机信息、所有 UI 在 WezTerm 内

> 状态:**Phase 1a**(MVP 主链可用,临时参数模式)。Phase 1b 起接入 SecureCRT。完整路线见 [`docs/Implementation_Plan.md`](docs/Implementation_Plan.md)。

## 安装

### 1. 系统依赖

```bash
# macOS
brew install --cask wezterm
brew install asciinema jq bash
brew install hudochenkov/sshpass/sshpass   # 可选,用 --password 才需要
brew install pass                          # 可选,密码后端

# 系统设置开启 "远程登录"(用于 self-test 的 ssh localhost)
# System Settings → General → Sharing → Remote Login
# 把当前用户的公钥加进 ~/.ssh/authorized_keys
```

> macOS 默认 bash 是 3.2,本 skill 用了 4+ 特性,**必须** `brew install bash`。

### 2. 安装 skill

```bash
cd /Users/<u>/Code/ssh-ops
bash install.sh        # 自检依赖 + 链接到 ~/.claude/skills/ssh-ops/
bin/sshops setup       # 交互式向导写 config.json
```

把 `~/.claude/skills/ssh-ops/bin` 加进 `PATH`(可选):

```bash
echo 'export PATH="$HOME/.claude/skills/ssh-ops/bin:$PATH"' >> ~/.zshrc
```

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
├── ssh-ops-requirements.md  完整需求规格(source of truth)
├── bin/
│   ├── sshops             CLI 主入口
│   └── sshops-setup       初始化向导
├── lib/
│   ├── common.sh          配置、日志、ssh 选项、ANSI strip、nonce、锁
│   ├── safety.sh          危险命令模式 + prod 判定
│   ├── wezterm.sh         wezterm cli 封装
│   ├── marker.sh          注入 + 切片(技术心脏)
│   ├── project.sh         项目识别 + pane 生命周期
│   └── recorder.sh        asciinema + commands.jsonl
├── config.example.json    配置模板
├── install.sh             依赖检测 + symlink
├── tests/self-test.sh     localhost echo 冒烟
└── docs/
    ├── PROJECT_OVERVIEW.md  架构总览
    └── Implementation_Plan.md  任务级状态
```

录像数据(运行时产生,不入库)位于 `~/.ssh-recordings/<project_slug>/<session_id>/`。

## 路线图

- **Phase 1a** ✅ 临时参数 + marker + 录像 + 安全栏杆
- **Phase 1b** SecureCRT 接入(`@路径` / 关键词 / 跳板机 / SSH2.ini 全局回退)
- **Phase 2** `bg` `fan` `health` `sync` `grid` `push/pull` `forward`
- **Phase 3** 回放 / 搜索 / 标注 / retention gc
- **Phase 4** WezTerm Lua 视觉(AI/HUMAN/REPLAY 边框色 + 回放快捷键)

## 常见问题

**问:Claude 调 sshops 失败,提示 "wezterm cli 不通"。**
答:WezTerm GUI 是否启动?skill 自带 `open -a WezTerm` 重试 5 秒,失败说明 WezTerm 没装或装坏。

**问:命令注入超时(`exit 4`)。**
答:目标命令是 TUI(`vim` `top`)或长任务。Phase 2 用 `sshops bg`,目前先 `sshops close` + 手敲 SecureCRT。

**问:`exit 3` shell 不支持。**
答:目标主机的登录 shell 必须是 bash 或 zsh。fish/tcsh/网络设备 CLI 都不行。

**问:`exit 5` 被拦截。**
答:危险命令在生产机被拦截。如确实要执行,在对话里明确告诉 Claude "我确认要在生产机执行 X",Claude 才会加 `--i-mean-it`。

**问:macOS bash 是 3.2,装 brew bash 后 sshops 仍找老 bash。**
答:确保 `which bash` 指向 `/opt/homebrew/bin/bash` 或 `/usr/local/bin/bash`。`#!/usr/bin/env bash` 走 PATH 顺序,把 brew 路径放前面。

## 文档

- [`SKILL.md`](SKILL.md) — 给 Claude 的决策手册
- [`ssh-ops-requirements.md`](ssh-ops-requirements.md) — 完整需求规格
- [`docs/PROJECT_OVERVIEW.md`](docs/PROJECT_OVERVIEW.md) — 架构总览
- [`docs/Implementation_Plan.md`](docs/Implementation_Plan.md) — 任务级状态
- [`CLAUDE.md`](CLAUDE.md) — 项目编码约束
