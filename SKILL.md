---
name: ssh-ops
description: 通过 WezTerm 进行 SSH 远程运维。每个 CC 项目对应一个 WezTerm 窗口,每台主机对应一个 pane,字符级实时画面 + 全程 asciinema 录像 + 命令级索引。**优先从用户的 SecureCRT 配置自动解析主机连接信息**(Hostname/Username/Port/Identity 含 SSH2.ini 全局回退),用户只需说一个 IP / 关键词 / 路径就能连。当用户要 SSH 跑命令、看实时画面、需要后续回放 / 审计时调用。
---

# ssh-ops Skill 决策手册

> Phase 1b。Phase 2/3/4 见 `docs/Implementation_Plan.md`。

## 1 触发条件

满足任一即调用本 skill,而不是用 `Bash(ssh ...)`:

- 用户要在远程主机上跑命令,且需要**看到实时画面**或**后续回放/审计**
- 用户提到"ssh 到 X 跑 Y" / "连一下 X" / "帮我看一下 X 的状态"
- 用户给出主机标识(IP / 主机名 / SecureCRT 路径关键词),即便没说 SSH 也是这个意图
- 用户要做远程运维、配置变更、长任务监控
- 同一项目里要并行管理多台主机的会话

## 2 主机解析三入口(关键!)

**用户给的主机标识有三种形式,本 skill 自动选择正确路径,无需问用户更多信息**:

### 2.1 `@<相对路径>` — SecureCRT 精确

例:`@aws/edge` → `<SecureCRT_Sessions>/aws/edge.ini`

```bash
sshops run @aws/edge "uptime"
```

skill 直接解析该 .ini,提取 Hostname / Username / Port / Identity(含 SSH2.ini 全局回退)。

### 2.2 `<关键词>` — SecureCRT 模糊匹配

例:`cvm-01` / `10.32.49.7` / `nutanix-cvm` 等。skill 在 SecureCRT Sessions 目录下递归匹配:

- 文件名(去 .ini 后缀)子串匹配
- Hostname 字段子串匹配
- **唯一命中**自动用;**多个候选**报错并列出,要求用 `@<路径>` 精确指定;零命中报错

```bash
sshops run 10.32.49.7 "uptime"        # IP 直查 SecureCRT
sshops run cvm-01 "ls /var/log"        # 关键词
```

**这是最常用路径**:用户只说 IP 或主机名,你直接传给 sshops,SecureCRT 已经存好的连接参数自动拉出来。

### 2.3 `--host H --user U` — 临时参数

完全绕过 SecureCRT,适合 SecureCRT 里没配的主机:

```bash
sshops run --host 10.1.2.3 --user root --key ~/.ssh/k.pem "uptime"
```

可选:`--port P` `--password STR` `--ask-password` `--prod`(用于危险命令拦截)

## 3 决策流(用户给主机时怎么走)

```
用户提到 X(IP / 主机名 / 关键词):
   1. 先尝试 sshops run X "<cmd>"(让 skill 自己走 SecureCRT 模糊匹配)
   2. 若失败提示「多个候选」→ 把候选列表给用户挑,带 @<路径>
   3. 若失败提示「该主机是密码登录」→ 让用户加 --ask-password 重跑,或建议
      他在 SecureCRT 给主机配 Identity 改 key 登录
   4. 若失败提示「没找到」→ 主机不在 SecureCRT,询问用户:
      - 你能给我 host/user/key 临时参数吗?
      - 或者先在 SecureCRT 加这台主机
```

**绝对不要**默认让用户手输 `--user --port --key` 等参数 — SecureCRT 已经存了你不用是浪费用户时间。

## 4 何时不该用本 skill

- 一次性 trivial 命令(如 `ssh host echo ok`)→ 直接 `Bash` 工具跑 ssh
- TUI 程序(`vim`、`top`、`less`、`htop`、`watch`)→ marker 切片不工作,会卡到超时;让用户在 SecureCRT/iTerm 里手敲
- 网络设备 CLI(华为 / 思科 / NetScaler / Juniper)→ 目标 shell 必须是 bash/zsh
- 需要交互输入的命令(`sudo` 输密码、`git push` 输密码)→ 走 sshpass 或预配 sudoers / NOPASSWD

## 5 密码登录主机的处理

**SecureCRT 的 `Password V2` 是私有加密格式,skill 不会、也不应该解码**(SecureCRT 9.0+ 的 `03:` 前缀加密 + master password 派生 key,无可靠公开解码方案)。

如果用户跑 `sshops run 10.88.220.201 "..."`,skill 检测到该 .ini 是密码登录(无 Identity + 有 Password V2),会**直接报错并列出三种解决方案**:

1. 现场输入:加 `--ask-password`
2. 在 SecureCRT 中给该主机配 Identity(改 key 登录)
3. 用 `pass` / macOS Keychain 后端(Phase 2 的 `password_refs` 配置)

Claude 看到这种错误时,**告诉用户三种方案中哪种最合适,等他确认再重跑**。

## 6 危险命令处理

skill 内置危险命令模式拦截(`rm -rf /`、`reboot`、`mkfs`、`dd of=/dev/`、`shutdown`、`:(){`、`chmod -R 777 /` 等)。

| 场景 | 行为 |
|---|---|
| 危险 + prod 主机(@路径含 prod 关键词,或临时参数 + `--prod`) | **拒绝**,exit 5,JSON `blocked:true exit:-1` |
| 危险 + prod + `--i-mean-it` | 警告但放行,JSON `dangerous:true blocked:false` |
| 危险 + 非 prod | 警告但放行,JSON `dangerous:true blocked:false` |

**Claude 绝对不要主动加 `--i-mean-it`**。必须用户在对话里明确说"我确认要在生产机上 X"才能加。看到 `blocked:true` 时,把 `reason` 字段原样转给用户并停下来等指令。

## 7 输出格式(`sshops run` 的 JSON)

```json
// 成功
{"exit":0,"duration_ms":340,"cast_offset":12.4,
 "session_id":"...","selector":"@aws/edge",
 "dangerous":false,"blocked":false,"output":"..."}

// 命令失败(远端非零 exit)
{"exit":1,"duration_ms":1200,"output":"...","selector":"@aws/edge","session_id":"..."}

// 拦截
{"exit":-1,"blocked":true,"dangerous":true,"reason":"...",
 "selector":"@prod/cvm-01","session_id":"...","output":"(not executed)"}
```

**进程 exit code vs JSON `exit` 字段**:
- 进程 exit:`5`=拦截,`4`=注入超时,`3`=主机不可达 / shell 不支持,`2`=选择器解析失败,`64`=参数错
- JSON `exit`:`-1` 仅用于 `blocked:true`,其他时候是远端命令真实 exit code

## 8 命令决策树

| 任务 | 命令 |
|---|---|
| 跑一条短命令(<30s,看输出) | `sshops run <selector> "<cmd>"` |
| 启动 pane 不跑命令(后续手敲或 run 复用) | `sshops open <selector>` |
| 关 pane | `sshops close <selector>` |
| 列当前项目所有 pane | `sshops list-panes` |
| 长任务后台 | Phase 2 `sshops bg`,目前先 `run` + `nohup &` |
| 多机并行 | Phase 2 `sshops fan` |
| 文件传输 | Phase 2 `sshops push/pull` |
| 端口转发 | Phase 2 `sshops forward` |

## 9 录像位置

**默认每个项目录到自己根下** `<project>/.ssh-ops/recordings/<session-id>/`(跟项目绑定,clone/move 时一起带走;首次写录像时 skill 自动给项目 `.gitignore` 加 `.ssh-ops/`)。

如果配置 `config.log_dir = "<path>"` 非空,改为全局集中存 `<path>/<project_slug>/<session-id>/`(适合统一审计)。

Phase 3 才有 `sshops log replay/search` 等回放能力,Phase 1b 只生成数据。

## 10 常见坑

- **首次连主机的 host key 提示**:已设 `StrictHostKeyChecking=accept-new`,自动接受
- **`.ppk` 私钥**:不支持,让用户用 `puttygen` 转 OpenSSH 格式
- **目标 shell 非 bash/zsh**:报 `exit 3`,fish/tcsh/网络设备 CLI 都不行
- **TUI 命令卡死**:超时 `exit 4`,让用户 `sshops close` 后自己上 SecureCRT
- **跳板机**:Phase 1b MVP **不递归解析跳板机**,只警告;若目标必须经跳板,Phase 2 才完整支持。临时方案:用户自己用 SecureCRT 连
- **路径中含中文 / 空格**:已全程 quote,SecureCRT 路径如 `/Users/u/Work/安全工具/SecureCRT/Config` 也能正常解析

## 11 一句话决策

**用户给主机标识(IP / 关键词 / `@路径`)→ `sshops run <那个标识> "<cmd>"`**。skill 自动从 SecureCRT 拉参数。失败再按提示加 `--ask-password` 或 SecureCRT 对应改 key。
