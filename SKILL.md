---
name: ssh-ops
description: 通过 WezTerm 进行 SSH 远程运维。每个 CC 项目对应一个 WezTerm 窗口,每台主机对应一个 pane,字符级实时画面 + 全程 asciinema 录像 + 命令级索引。当用户要 SSH 跑命令、看实时画面、需要后续回放 / 审计时调用。Phase 1a 仅支持临时参数(--host --user),Phase 1b 起接 SecureCRT。
---

# ssh-ops Skill 决策手册

> Phase 1a 简版。完整流程见 `ssh-ops-requirements.md`。

## 1 触发条件

满足任一即调用本 skill,而不是用 `Bash(ssh ...)`:

- 用户要在远程主机上跑命令,且需要**看到实时画面**或**后续回放/审计**
- 用户提到"ssh 到 X 跑 Y"且 Y 不是一次性 trivial 命令
- 用户要做远程运维、配置变更、长任务监控
- 同一项目里要并行管理多台主机的会话

## 2 何时不该用本 skill

- 一次性 trivial 命令(如 `ssh host echo ok`)→ 直接 `Bash` 工具跑 ssh
- TUI 程序(`vim`、`top`、`less`、`htop`、`watch`)→ marker 切片不工作,会卡到超时;让用户在 SecureCRT/iTerm 里手敲
- 网络设备 CLI(华为 / 思科 / NetScaler / Juniper)→ Phase 1 不支持,目标 shell 必须是 bash/zsh
- 需要交互输入的命令(`sudo` 输密码、`git push` 输密码)→ 走 sshpass 或预配 sudoers

## 3 前置条件

- WezTerm 已安装并能在 GUI 启动(macOS:`brew install --cask wezterm`)
- `asciinema` `jq` `ssh` `perl` 已装,缺则报 `install.sh` 提示
- 已跑 `sshops setup`,`config.json` 就位
- (可选)`sshpass` 装好以支持 `--password`

## 4 命令决策树

```
任务                                       命令
───────────────────────────────────────────────────────────────
跑一条短命令(预期 < 30s,看输出)         sshops run --host H --user U [...] "cmd"
启动 pane 不跑命令(后续手敲或 run 复用)  sshops open --host H --user U [...]
关 pane                                    sshops close --host H --user U [...]
列当前项目所有 pane                        sshops list-panes
初始化配置                                  sshops setup
```

`run` 会自动复用同一 selector(`user@host:port`)的 pane,不会反复 spawn。

## 5 危险命令处理

skill 内置危险命令模式拦截(`rm -rf /`、`reboot`、`mkfs`、`dd of=/dev/...`、`shutdown`、`:(){`、`chmod -R 777 /` 等)。

| 场景 | 行为 |
|---|---|
| 危险 + `--prod` 标志 + 无 `--i-mean-it` | **拒绝执行**,exit 5,JSON `blocked:true exit:-1` |
| 危险 + `--prod` + `--i-mean-it` | 警告但放行,JSON `dangerous:true` |
| 危险 + 非 prod | 警告但放行,JSON `dangerous:true` |

**Claude(你)绝对不要主动加 `--i-mean-it`。** 必须用户在对话里明确说"我确认要在生产机上 X"才能加。看到 `blocked:true` 时,把 `reason` 字段原样转给用户并停下来等指令。

## 6 输出格式

`sshops run` 在成功 / 失败 / 拦截时都返回单行 JSON:

```json
// 成功
{"exit":0,"duration_ms":340,"cast_offset":12.4,"session_id":"...","dangerous":false,"blocked":false,"output":"..."}

// 命令失败(远端非零 exit)
{"exit":1,"duration_ms":1200,"output":"...","session_id":"..."}

// 拦截
{"exit":-1,"blocked":true,"dangerous":true,"reason":"...","session_id":"...","output":"(not executed)"}
```

**进程 exit code 与 JSON `exit` 字段语义不同**:
- 进程 exit:`5` = 拦截,`4` = 注入超时,`3` = 主机不可达 / shell 不支持,`2` = 选择器解析失败,`64` = 参数错
- JSON `exit`:`-1` 仅用于 `blocked:true`,其他时候是远端命令真实 exit code

## 7 选择器语法(Phase 1a)

只支持临时参数:

```
--host H --user U [--port P] (--key K | --password P | --ask-password) [--prod]
```

Phase 1b 起还会有:
- `@<相对路径>` SecureCRT session
- `<关键词>` 模糊匹配

## 8 常见坑(Phase 1a)

- 第一次连主机,host key 提示:已设 `StrictHostKeyChecking=accept-new`,自动接受
- `--password` 走 sshpass,需要本机装了 `sshpass`(`brew install hudochenkov/sshpass/sshpass`)
- 私钥是 `.ppk` 格式:不支持,让用户用 `puttygen` 转成 OpenSSH 格式
- 目标主机的 shell 不是 bash/zsh:报 `exit 3`,目前不支持 fish/tcsh/网络设备 CLI
- 命令含 `&` 后台执行:`$?` 拿不到真实 exit code,Phase 2 用 `sshops bg` 解决
- 命令是 TUI:会卡到 `marker_timeout_seconds` 超时,人工 `sshops close` 后让用户自己上 SecureCRT

## 9 录像位置

```
<log_dir>/<project_slug>/<session_id>/
  stream.cast        asciinema v2,完整 PTY
  commands.jsonl     每条命令一行
  meta.json          session 元数据
```

Phase 3 才有 `sshops log replay/search` 等回放能力。Phase 1a 只生成数据。

## 10 一句话决策

**用户要看实时画面 / 要录像 / 要后续审计 → ssh-ops。否则 → 普通 ssh。**
