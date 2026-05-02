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

### 5.1 三种密码来源(skill 自动决策,Claude 不需要选)

**Claude 默认行为永远是**:`sshops run <selector> "<cmd>"`,不主动加 `--password` / `--ask-password`。skill 内部根据上下文自动选下面三种之一:

#### A. 用户在对话中直接给出密码

例如:"连 10.88.220.201,密码是 Qwe123!@#"。这是**用户显式授权 + 显式提供**,Claude 此时**直接用 `--password '<密码>'` 调用**:

```bash
sshops run 10.88.220.201 --password 'Qwe123!@#' "uptime"
```

skill 会用 sshpass 自动喂密码,登录无感。

**告知用户一次但不阻塞**:密码进入进程 args(`ps` 可见,短暂)、不进 shell history、不写入 commit/log。**生产密码建议用 keychain / pass 后端**(Phase 2 的 password_refs)。

#### B. 用户没给密码 + 在自己终端跑(有 tty)

skill 从 `/dev/tty` 自动弹 prompt,用户输完密码就连(等同隐式 `--ask-password`):

```bash
$ sshops run 10.88.220.201 "uptime"
密码 (该主机 10.88.220.201 在 SecureCRT 是密码登录): ******
{"exit": 0, ...}
```

#### C. 用户没给密码 + Claude 子进程调用(无 tty)— 自动 spawn + 等手输

**这是关键场景**:Claude 在子进程里调 `sshops`,没 tty,但**不会报错**。skill 行为:

1. spawn pane,跑 `ssh` 不带 sshpass(让 ssh 自己 prompt)
2. WezTerm pane 显示 `[user@host]$ password:`(ssh 在等密码)
3. **用户在 WezTerm pane 里手输密码**(密码不进 ps、不进 history、不进 log,只在 pane 的 PTY 里)
4. skill 后台轮询 pane 内容,检测到密码 prompt 消失 + shell 启动
5. 自动发命令,切片输出,返回 JSON

**Claude 看到此场景,只需告诉用户**:"已经 spawn 好 pane,你去 WezTerm 输密码,完事我直接给你结果"。然后等 sshops 的 JSON 返回(可能 5-30 秒,看用户输密码速度)。**不要重试,不要二次询问密码**。

#### 总结决策表

| 场景 | Claude 调的命令 | skill 内部行为 |
|---|---|---|
| 用户对话给密码 | `--password 'XXX'` | sshpass 喂入 |
| 用户在自己终端跑 sshops 且 .ini 密码登录 | (无 password 参数) | tty 自动 prompt |
| Claude 子进程,密码登录主机 | (无 password 参数) | spawn pane → 用户手输 → 自动接管 |
| key 登录主机 | (无 password 参数) | 直接 ssh -i key |

**Claude 不需要根据场景区别对待 — 永远调 `sshops run <selector> "<cmd>"`,skill 自己决策**。仅在用户对话里明确给密码时,加 `--password 'XXX'`。

### Claude 看到 "需要密码,但当前进程无 tty" 错误时怎么做

1. **不要再次重试同一命令** — 没有 tty,再调一次还是失败
2. **把 stderr 的方案文本原样转给用户**,问他选哪个
3. 推荐顺序:**A > C > B > D**
   - A:用户在自己终端 zsh 里跑同样命令 → 自动 prompt,密码不留 history
   - C:Phase 2 password_refs 配 keychain / pass → 一次配置,以后 Claude 直接调无需交互(**长期最优**)
   - B:用户在终端 + `--ask-password`(等同 A,显式)
   - D:SecureCRT 改 key 登录(治本,但要用户改配置)

### 主机有 key 但 .ini 仍标密码登录?

如果 `Identity Filename V2` 字段被 SSH2.ini 全局回退给了 key,但 `Password V2` 仍非空,**仍当密码登录处理**(用户在 SecureCRT 里选了密码 auth 是强信号)。回退的 key 留在 ssh `-i` 参数里,如果远端恰好接受这个 key,ssh 会自动用 key 登录,密码不会被发到目标机(零额外审计噪音)。

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
| 抓取 pane 当前可见文本(用户在 pane 手敲命令后,Claude 直接读画面) | `sshops peek <selector>` |
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
