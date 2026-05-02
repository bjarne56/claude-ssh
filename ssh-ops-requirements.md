# ssh-ops Skill 需求规格

> 这是一份给 Claude Code 实现用的需求规格。目标:为 Claude Code 制作一个 skill,让它能够通过 WezTerm 进行 SSH 远程运维操作,同时用户在 WezTerm 窗口里看到字符级实时画面、全程录像、可回放可搜索。

## 1. 背景与目标

### 1.1 痛点

Claude Code 默认通过 `bash` 工具执行 `ssh user@host "cmd"`,每次都是一次性短连接,没法持续会话,没法处理交互,没法看实时画面,没法录像。每次操作都重新 SSH,慢且割裂。

### 1.2 解决思路

把 SSH 会话外置到 WezTerm 终端中:

- 每个 CC 项目(以 CC 工作目录区分)对应一个 WezTerm 窗口
- 每台被连接的主机对应该窗口里的一个 pane,自动 split 成网格
- WezTerm pane 持有真 PTY,字符级实时渲染,用户可以肉眼盯着看
- skill 通过 `wezterm cli send-text` 注入命令,通过 marker 切片机制抓回输出给 Claude
- 全程用 `script` 录制 PTY 流(asciinema 兼容格式),命令级索引另存
- 回放、搜索、控制全部在 WezTerm 内部用 Lua + 快捷键实现,不依赖外部播放器

### 1.3 不做的事

- **不写** `~/.ssh/config`,不污染用户原有 SSH 配置
- **不缓存** 主机信息,不维护 hosts.json/inventory
- **不 fork** WezTerm 源码,只用它的 cli 和 Lua 配置
- **不做** Web 界面 / 独立 GUI 工具,所有 UI 在 WezTerm 内
- **不自动转换** SecureCRT 配置,只实时读取
- **不导出** 主机库到任何持久格式

## 2. 总体架构

```
┌────────────────────────────────────────────────────────────────┐
│ Claude Code 窗口(项目 A: ~/work/55ai)                         │
│  Claude 调用 sshops run @aws/edge "uptime"                     │
└────────────────────────────────────┬───────────────────────────┘
                                     │ 通过 wezterm cli
                                     ▼
┌────────────────────────────────────────────────────────────────┐
│ WezTerm 窗口 #1 (项目 55ai)                                    │
│ ┌──────────────────────┬──────────────────────┐                │
│ │ pane: @aws/edge      │ pane: @aws/bedrock   │                │
│ │ user@edge $ uptime   │ user@bedrock $ ...   │                │
│ │ 14:23 up 47 days...  │                      │                │
│ │ user@edge $ _        │                      │                │
│ └──────────────────────┴──────────────────────┘                │
│ tab 标签: [AI] aws/edge   [HUMAN] aws/bedrock                  │
└────────────────────────────────────────────────────────────────┘
        ▲ 同时录制
        │
┌───────┴─────────────────────────────────────────────────────────┐
│ ~/.ssh-recordings/55ai/<session-id>/                            │
│   stream.cast        # asciinema v2,完整 PTY 流                │
│   commands.jsonl     # 命令级索引(actor / time / exit ...)    │
│   meta.json          # session 元数据                           │
│   annotations.jsonl  # 用户标注                                 │
└─────────────────────────────────────────────────────────────────┘

另一个 CC 项目(~/work/nutanix-ops)→ 独立的 WezTerm 窗口 #2
```

## 3. 主机来源(三种入口)

### 3.1 SecureCRT session 路径(主要)

格式:`@<相对路径>`,对应 SecureCRT Sessions 目录下的 `.ini` 文件。

例:`@prod/nutanix/cvm-01` → `<SessionsDir>/prod/nutanix/cvm-01.ini`

skill **实时**解析 `.ini`,提取这些字段:

| .ini 字段 | 用途 |
|---|---|
| `S:"Hostname"` | IP 或域名 |
| `S:"Username"` | 登录用户 |
| `D:"[SSH2] Port"` | 端口(8 位 hex 字符串,如 `00000016` = 22;**兼容**:解析失败时尝试十进制,再失败默认 22 并 warn) |
| `S:"Identity Filename V2"` 或 `S:"PublicKey Filename V2"` | key 路径 |
| `S:"Firewall Name"` | 跳板机引用,值如 `Session:bastion-kl`(详见下方);值为 `None`(字面字符串)或空时表示无跳板机 |
| `S:"Protocol Name"` | 协议名,**仅处理 `SSH2`**,其它(SSH1 / Telnet / Serial / Rlogin)直接报错拒绝 |

**禁止读取** `S:"Password V2"` 字段(私有加密格式,不可靠)。如果该主机 SecureCRT 里只配了密码没配 key,skill 报错并要求用户用 `--ask-password` 或在 SecureCRT 中补 key。

#### 3.1.1 全局默认值回退(SSH2.ini)

`<SecureCRT_Config>/SSH2.ini` 是 SecureCRT 的 **SSH2 协议全局默认配置**。Sessions/*.ini 中字段为空字符串时,按以下优先级回退:

1. `S:"Identity Filename V2"` 空 → 取 `SSH2.ini` 同名字段
2. `S:"PublicKey Filename V2"` 空 → 取 `SSH2.ini` 同名字段
3. `S:"SSH1 Identity Filename V2"` (仅 SSH1,本 skill 不处理)

`Port` / `Hostname` / `Username` / `Firewall Name` **不**做全局回退(逐机配置,语义上不应该走默认)。

`<SecureCRT_Config>/Global.ini` 仅含 GUI 配置,**不解析**。

#### 3.1.2 路径变量展开

SecureCRT 配置中出现的以下变量在解析后必须展开:

| 变量 | 展开为 |
|---|---|
| `${VDS_CONFIG_PATH}` | SecureCRT Config 根目录绝对路径(如 `/Users/u/Work/安全工具/SecureCRT/Config`) |

Windows 风格路径(以 `[A-Z]:\\` 开头或包含 `\\`)走 `config.json` 的 `path_mappings` 数组转换为本机路径。

#### 3.1.3 跳板机解析(`Firewall Name=Session:<rel-path>`)

- 值的 `<rel-path>` 是相对 Sessions 根目录的路径,**不带 `.ini` 扩展名**,可包含 `/`(如 `Session:infra/bastion-kl` → `<SessionsDir>/infra/bastion-kl.ini`)
- 跳板机自身按完整 .ini 解析(host/user/port/key/全局回退)
- **递归深度上限 3 层**(目标机 → 跳板A → 跳板B → 跳板C),超过报错(`exit 2`)
- **必须做循环检测**:维护已访问 session 路径集合,命中即报错(`exit 2`)
- 组装为 ssh `-J user1@host1:port1,user2@host2:port2,...` 形式

### 3.2 关键词模糊匹配

例:`sshops run nutanix-cvm-01 ...`

实现:递归扫 SecureCRT Sessions 目录所有 `.ini`,匹配文件名(不含扩展名)或 Hostname 字段。

- 唯一命中 → 自动使用
- 多个命中 → 报错并列出所有候选,要求用户用 `@路径` 精确指定
- 零命中 → 报错

### 3.3 临时参数

完全绕过 SecureCRT:

```
sshops run --host 10.1.2.3 --user root --key ~/.ssh/k.pem "uptime"
sshops run --host 10.1.2.3 --user root --password "xxx" "uptime"
sshops run --host 10.1.2.3 --user root --ask-password "uptime"
```

`--ask-password` 在用户的当前终端(非 Claude Code)prompt 一次,通过 sshpass 喂给 ssh,不落盘。

### 3.4 密码后端(三选混用)

`password_ref` 字段三种前缀,逐机选择:

- `pass:server/cvm01` → 调用 `pass show ...`(GPG 加密的 password store)
- `keychain:cvm01` → macOS `security find-generic-password ...`
- `plain:hunter2` → 明文(**仅允许非生产路径主机**,prod 标签主机禁止)

## 4. 项目与窗口管理

### 4.1 项目识别

**规则**:项目 ID = CC 当前工作目录(`$PWD`)的规范化路径。

- 同一目录的多个 CC 实例共享同一 WezTerm 窗口(看到对方的 panes)
- 不同目录自动开不同 WezTerm 窗口
- 用户可通过 `SSHOPS_PROJECT` 环境变量手动覆盖

state 文件 `~/.claude/skills/ssh-ops/state/panes.json`:

```json
{
  "/Users/u/work/55ai": {
    "wezterm_window_id": 17,
    "started_at": "2026-05-02T10:00:00Z",
    "panes": {
      "@aws/edge": { "pane_id": 42, "session_id": "55ai-...", "started_at": "..." },
      "@aws/bedrock": { "pane_id": 43, "session_id": "55ai-...", "started_at": "..." }
    },
    "jobs": {
      "deploy": { "pane_id": 42, "started_at": "...", "pid": 12345 }
    },
    "forwards": [
      { "id": "f1", "selector": "@aws/edge", "spec": "8080:127.0.0.1:80", "pane_id": 99 }
    ]
  }
}
```

### 4.2 窗口生命周期

- 第一次在某项目里调用 skill → 检测窗口是否存在,不存在则 `wezterm cli spawn --new-window` 创建
- 窗口存在但 pane id 已失效(用户手动关了)→ 清理 state 重建
- 项目对话结束 / CC 退出 → **不**自动关窗口(用户可能还要看历史)
- `sshops cleanup` 清理失效引用

### 4.3 布局策略

**默认 grid**:多台主机自动 split 成网格。

split 规则:

- 第 1 台 → 占满窗口
- 第 2 台 → 横向 split(50/50)
- 第 3 台 → 在第二个 pane 上纵向 split
- 第 4 台 → 在第一个 pane 上纵向 split
- 第 N 台(N>4)→ 找当前最大的 pane split,优先纵横交替保持宽高比

不强制限制 pane 数量上限,用户屏幕看不下是用户自己的事。提供:

- `sshops focus <selector>` — zoom 单 pane 全屏
- `sshops detach <selector>` — 拆出独立窗口
- `sshops layout grid|tabs|stack` — 切换布局模式

### 4.4 多 CC 并发

不同 CC 项目天然分窗口。同一项目多个 CC 实例共享窗口,通过命令的 marker nonce 区分,各自的命令切片不会串。

## 5. 命令注入与输出抓取(marker 机制)

### 5.1 问题

WezTerm pane 持有的是 PTY,流式输出没有"命令边界"概念。skill 必须自己注入边界标记。

### 5.2 实现

每条命令注入时包成:

```bash
echo __SSHOPS_BEGIN_<nonce>__; <用户命令>; echo __SSHOPS_END_<nonce>__:$?
```

`<nonce>` 是 8 字符随机串。

发送方式:`wezterm cli send-text --pane-id N --no-paste "..." `,末尾加 `\r`。

### 5.3 抓取

发送后,轮询 `wezterm cli get-text --pane-id N`(每 200ms),直到看到 `__SSHOPS_END_<nonce>__:<exitcode>`。

抓取的内容:

- 起点:`__SSHOPS_BEGIN_<nonce>__` 之后的下一行
- 终点:`__SSHOPS_END_<nonce>__:` 所在行之前
- 处理:strip ANSI 转义码、去掉 prompt 行(可选)

最大等待时间默认 30 秒,超时返回错误并提示用户改用 `sshops bg`。

### 5.4 等待初始 prompt

新 spawn 的 pane 先 ssh 在认证,要等 prompt 出现才能注入命令。策略:

1. spawn 完后**先发 `echo $SHELL\r`**,从 pane 抓回 shell 路径
   - 仅 `bash` / `zsh` 通过(Phase 1a/1b 限定)
   - 其他(`fish` / `tcsh` / `csh` / busybox `sh` / 网络设备 CLI / Nutanix `ncli` 等)直接 close pane 并报错:`目标 shell 不支持(检测到 $SHELL),本 skill 仅支持 bash/zsh`(`exit 3`)
2. 注入 `export PS1='SSHOPS_READY$ '; clear`(进入 shell 后第一条;bash/zsh 兼容此语法)
3. 轮询直到 pane 末尾出现 `SSHOPS_READY$ `
4. 之后所有命令都基于这个 prompt

**Phase 1 排除清单**(已知 marker 机制不工作,不支持):
- 华为 / 思科 / NetScaler / Juniper 等网络设备 CLI
- 容器 `kubectl exec` 进入的非交互 shell
- 不允许 `export` 的受限 shell

未来(Phase 5+ 扩展点,不实现):fish 可通过 `function fish_prompt; echo SSHOPS_READY; end` 支持。

如果 ssh 卡在密码输入(密码主机首连),通过 sshpass 提前喂入,不应该走到这里。

### 5.5 ANSI 处理

抓回的输出 strip 掉:`\x1b\[[0-9;]*[a-zA-Z]` 和 `\x1b\][^\x07]*\x07`(OSC)。但保留行内容本身。

## 6. WezTerm cli 调用规范

skill 必须通过 `wezterm cli` 完成所有 WezTerm 操作:

| 操作 | 命令 |
|---|---|
| 列出窗口/tab/pane | `wezterm cli list --format json` |
| 新窗口 spawn | `wezterm cli spawn --new-window --cwd ~ -- <cmd>` |
| 在指定 pane 内 split | `wezterm cli split-pane --pane-id N --right/--bottom --percent P -- <cmd>` |
| 注入文本 | `wezterm cli send-text --pane-id N --no-paste "<text>"` |
| 抓 pane 内容 | `wezterm cli get-text --pane-id N` |
| 关 pane | `wezterm cli kill-pane --pane-id N` |
| 移到新 tab/窗口 | `wezterm cli move-pane-to-new-tab --pane-id N [--window-id ID]` |
| 激活 pane | `wezterm cli activate-pane --pane-id N` |

skill 不应该假设 WezTerm 已经启动,如果 `wezterm cli list` 失败,先尝试 `open -a WezTerm`(macOS) 或 `wezterm` 启动,等待 1-2 秒重试。

## 7. 录像系统

### 7.1 录制方式

**唯一录像方案:asciinema**(macOS 与 Linux 一致)。

```
wezterm cli spawn --new-window -- \
  asciinema rec --quiet --stdin --command "ssh <参数> user@host" \
                "<rec_dir>/stream.cast"
```

理由:
- 跨平台一致,`stream.cast` 是 v2 标准 JSON-Lines 格式,带每条事件的相对时间戳,`cast_offset` 字段天然存在
- macOS 自带的 `script` 是 BSD 版,**参数语义与 GNU `script` 不兼容**(`-t` 在 BSD 中是"最大空闲秒数",不是输出 timing 文件),不实现 BSD `script` fallback
- Phase 3 的搜索 / 回放 / 导出 全部基于 `.cast` 解析,不维护双轨格式

`install.sh` 检测不到 `asciinema` **直接报错**,提示 `brew install asciinema`(macOS)或 `pip install asciinema`(Linux),**不允许降级**。

GNU `script` (Linux 自带) 的 fallback 在 Phase 5+ 重新评估,Phase 1-4 不实现。

### 7.2 录像目录结构

```
~/.ssh-recordings/<project-name>/<session-id>/
├── stream.cast            # asciinema v2 格式 (优先) 或 stream.typescript + stream.timing
├── commands.jsonl         # 命令级索引
├── meta.json              # session 元数据
└── annotations.jsonl      # 用户标注
```

`<session-id>` 格式:`<host-slug>-<YYYYMMDD-HHMMSS>-<short-id>`

例:`aws-edge-20260502-142301-a3f4b1`

### 7.3 commands.jsonl 格式

每行一条 JSON:

```json
{"ts":"2026-05-02T14:23:01.234Z","actor":"ai","host":"@aws/edge","cmd":"uptime","exit":0,"duration_ms":340,"cast_offset":12.4,"dangerous":false,"blocked":false,"nonce":"a3f4b1c8"}
```

字段:

- `ts`: ISO 8601 时间戳
- `actor`: `ai` / `human` / `system`(skill 自身注入的如 PS1)
- `host`: 主机选择器
- `cmd`: 完整命令文本
- `exit`: 退出码
- `duration_ms`: 命令耗时
- `cast_offset`: 在 stream.cast 里的时间偏移(秒,浮点)
- `dangerous`: 是否匹配危险模式
- `blocked`: 是否被安全栏杆拦截(此时 exit 为 -1,无实际执行)
- `nonce`: marker 用的随机 ID

### 7.4 meta.json 格式

```json
{
  "session_id": "aws-edge-20260502-142301-a3f4b1",
  "project": "55ai",
  "project_path": "/Users/u/work/55ai",
  "host_selector": "@aws/edge",
  "host_resolved": "10.1.2.3",
  "user": "ec2-user",
  "auth_type": "key",
  "key_fingerprint": "SHA256:...",
  "started_at": "2026-05-02T14:23:01Z",
  "ended_at": null,
  "wezterm_pane_id": 42,
  "command_count": 0,
  "ai_command_count": 0,
  "human_command_count": 0,
  "dangerous_count": 0,
  "blocked_count": 0,
  "tags": ["55ai", "prod", "aws"]
}
```

session 结束时(pane 关闭)更新 `ended_at` 和计数字段。

### 7.5 actor 识别

- skill 注入的命令带 marker → actor = `ai`
- 用户在 pane 里手动敲的命令 → 通过 `sshops sync` 抓回 pane 历史时识别。识别规则:抓 pane 全文,按 prompt 行切分,不带 marker 的命令算 `human`,加入 commands.jsonl

`sshops sync` 应该在 Claude 调用 `sshops run` 之前**自动触发一次**,这样 AI 能感知到用户的介入。

### 7.6 retention 策略

`config.json` 里:

```json
{
  "retention": {
    "raw_days": 7,
    "compressed_days": 90,
    "keep_index_forever": true,
    "exempt_patterns": ["prod/**"]
  }
}
```

`sshops log gc` 跑清理:

- 7 天前的 `stream.cast` 用 gzip 压缩
- 90 天前的 `stream.cast.gz` 删除,保留 `commands.jsonl` `meta.json` `annotations.jsonl`
- `exempt_patterns` 匹配的 session 永不删

不要做自动后台清理,只做命令式清理(用户可以加 cron)。

## 8. 回放系统(全部在 WezTerm 内)

### 8.1 启动回放

```
sshops log replay <session-id> [--at <time>]
```

行为:

1. 在当前项目窗口里 split 一个**新 pane**(标记为 replay pane)
2. 在该 pane 内执行 `asciinema play` 或 `scriptreplay`
3. 设置 WezTerm key table,激活回放快捷键

`--at <time>` 支持:

- 秒数:`--at 145`
- 时间戳:`--at 14:23:30`
- 命令序号:`--at cmd:7`

### 8.2 回放 pane 的视觉标识

WezTerm Lua 配置应该让 replay pane:

- tab 标题前缀:`[REPLAY <session-short> @<offset>/<total>]`
- 边框颜色:灰色(区分于实时 AI 红 / HUMAN 绿)
- 背景略暗(可选,通过 `pane:set_user_var` 配合 Lua hooks)

### 8.3 回放快捷键(WezTerm key_tables)

进入 replay pane 自动激活 `replay_mode` key table。**asciinema CLI 没有原生暂停/精确 seek/动态变速 API**,以下按实现可行性分级:

**完全可做(Phase 3 必交付)** — 基于"关掉 asciinema 重开 + `--start-at` + `--speed`":

| 键 | 动作 |
|---|---|
| `q` | 退出回放(关 pane) |
| `g` | 弹出输入框跳到时间(WezTerm prompt → 重开播放器) |
| `n` | 跳到下一条命令(从 `commands.jsonl` 取下一个 `cast_offset` → 重开) |
| `p` | 跳到上一条命令(同上) |
| `←` `→` | 后退 / 前进 5 秒(当前位置 ± 5,重开) |
| `Shift+←` `Shift+→` | 后退 / 前进 30 秒(同上) |
| `1`/`2`/`4`/`5` | 1x/2x/4x/5x 速(重开 `--speed N`) |
| `Shift+1..4` | 10x/15x/20x/30x 速(同上) |
| `0` | 回到 1x |
| `/` | 命令搜索(grep `commands.jsonl` 后定位 `cast_offset`,重开) |
| `a` | 在当前位置加标注(写 `annotations.jsonl`) |

**Best effort(Phase 3 可选)** — 实现可能但精度差:

| 键 | 动作 | 备注 |
|---|---|---|
| `Space` | 暂停 / 继续 | `kill -STOP / -CONT` asciinema 进程,暂停时计时不准;**Phase 3 实现等价为"重启到当前位置",视觉效果一致** |

**不做(Phase 3 砍掉)**:

- 真·暂停后字符级精确续播(SIGSTOP/CONT 不保证位置)
- 进度条实时显示当前位置(需要 WezTerm 状态栏 Lua hook,**Phase 4 再说**)

**实现要点**:

- "重开播放器"模式统一封装为 `replay_seek <session_id> <offset_seconds> <speed>`,所有跳转/变速都调它
- 当前位置由外部 wrapper 跟踪(asciinema 启动时间 + 当前耗时 + 暂停累计)
- `commands.jsonl` 的 `cast_offset` 字段是 n/p 的核心数据源,必须确保写入精度

### 8.4 文本回放(只看命令不看视频)

```
sshops log replay <session-id> --text
```

直接在当前 pane 输出格式化文本:

```
[14:23:01] AI    @aws/edge       $ uptime                          0  340ms
[14:23:08] HUMAN @aws/edge       $ vim /etc/nginx.conf             0  45.2s
[14:24:01] AI    @aws/edge       $ systemctl restart nginx         0  2.1s
```

颜色:AI 蓝、HUMAN 绿、危险/失败 红。

## 9. 命令搜索

```
sshops log search <query> [filters...]
```

支持的 filter:

| filter | 含义 |
|---|---|
| `--cmd <regex>` | 命令文本正则 |
| `--output <regex>` | 输出内容正则(grep stream.cast) |
| `--host <pattern>` | 主机匹配 |
| `--actor ai\|human` | 谁敲的 |
| `--exit <op>` | 退出码,如 `!=0`、`>0`、`=2` |
| `--duration <op>` | 耗时,如 `>5s` |
| `--since <duration>` | 时间窗,如 `24h`、`7d` |
| `--between <T1>..<T2>` | 时间区间 |
| `--dangerous` | 仅危险或被拦截 |
| `--project <name>` | 项目名 |

输出:每行一条命中,带 session-id 和 cast_offset,行内可点击(WezTerm OSC 8 hyperlink)跳转到回放。

实现:跨所有 session 遍历 `commands.jsonl`,jq + grep 过滤。`--output` 需要扫 `stream.cast`(慢一点)。

## 10. 安全栏杆

### 10.1 生产机判定

如果选择器解析后的 SecureCRT session 路径包含以下关键词之一(可在 config.json 自定义),判为生产机:

```
prod / production / 生产 / live
```

例:`@prod/nutanix/cvm-01` 是 prod,`@dev/test01` 不是。

### 10.2 危险命令模式

`config.json` 里 `dangerous_patterns` 数组,默认值:

```json
[
  "rm\\s+-rf\\s+/",
  "rm\\s+-rf\\s+~",
  "mkfs\\.",
  "dd\\s+.*of=/dev/[sh]d",
  "shutdown",
  "reboot",
  "halt",
  "ncli\\s+cluster\\s+destroy",
  "cluster\\s+destroy",
  ":\\(\\)\\s*\\{",
  "chmod\\s+-R\\s+777\\s+/",
  ">\\s*/dev/[sh]d",
  "wipefs"
]
```

### 10.3 拦截流程

`sshops run`/`bg`/`session`/`fan` 在执行前必须:

1. 解析目标主机,判断是否生产机
2. 用 `dangerous_patterns` 正则匹配命令文本
3. 命中 + 生产机:
   - **拒绝执行**,返回 exit=-1,stderr 写明拦截原因
   - 在 commands.jsonl 写一条 `blocked=true` 记录
   - WezTerm pane 边框闪红 1 秒(Lua hook)
4. 命中 + 非生产机:仅警告(stderr),正常执行,但记录 `dangerous=true`

### 10.4 强制放行

只有当传入 `--i-mean-it` 标志时才在生产机上执行危险命令:

```
sshops run @prod/cvm-01 "reboot" --i-mean-it
```

Claude 不应该自动加这个标志,必须用户在对话里明确说"我确认"才能加。

### 10.5 明文密码限制

启动时自检:任何 `password_ref` 是 `plain:` 前缀的临时主机参数,如果同时该主机被判为生产机(虽然临时参数没路径,但用户可以传 `--prod` 显式标),拒绝连接。

## 11. 命令面规格

### 11.1 选择器语法

所有命令的 `<selector>` 参数支持:

- `@<相对路径>` — SecureCRT 路径
- `<关键词>` — 模糊匹配
- `--host <H> --user <U> [--port <P>] (--key <K> | --password <P> | --ask-password) [--jump <selector>]` — 临时参数

`<pattern>` 用于批量,语法:

- `@<glob>` 例 `@prod/nutanix/**`
- `tag:<词>` 路径包含该词(等价于 `@**/<词>/**` 或文件名包含)

### 11.2 完整命令清单

```
sshops setup                          # 交互式初始化配置

# SecureCRT 查询
sshops list [<pattern>]
sshops show <selector>
sshops crt-find <keyword>

# 单机操作
sshops open <selector>                # spawn pane,不执行命令
sshops run <selector> "<cmd>" [--sudo-pass]
sshops session <selector> < script    # heredoc 批量
sshops bg <selector> <jobname> "<cmd>"
sshops tail <selector> <jobname> [--follow <N>] [--lines <M>]
sshops jobs [<selector>]              # 列后台任务

# 批量
sshops fan <pattern> "<cmd>" [--parallel <N>] [--stop-on-fail]
sshops health [<pattern>] [--check tcp,ssh,uptime,disk]

# 布局控制
sshops grid <pattern>                 # 一次性 spawn 所有匹配主机
sshops focus <selector>               # zoom 该 pane
sshops detach <selector>              # 拆成独立窗口
sshops close <selector>
sshops layout grid|tabs|stack
sshops sync <selector>                # 抓 pane 最近输出回灌(也用于检测 human 命令)

# 端口转发
sshops forward <selector> <L:H:R> [--reverse]
sshops forward list
sshops forward stop <id>

# 文件传输
sshops push <selector> <local> <remote>
sshops pull <selector> <remote> <local>

# 录像与回放
sshops log                            # 当前项目所有 session
sshops log <session-id>               # 该 session 命令索引
sshops log search <query> [filters]
sshops log replay <session-id> [--at T] [--text]
sshops log export <session-id> --format <cast|txt|md|gif|mp4>
sshops log dangerous [--since 24h]
sshops log annotate <session-id> --at T --note "..."
sshops log gc

# 维护
sshops cleanup                        # 清理失效 state
sshops version
sshops help [<command>]
```

### 11.3 输出格式

`sshops run` 的成功输出:

```
exit: 0
duration_ms: 340
session_id: aws-edge-20260502-142301-a3f4b1
cast_offset: 12.4
output:
14:23:01 up 47 days, load 0.5
```

失败:

```
exit: 1
duration_ms: 1200
session_id: ...
cast_offset: ...
output:
ssh: connect to host 10.1.2.3 port 22: Connection refused
error: ssh exited non-zero
```

被拦截:

```
exit: -1
blocked: true
reason: dangerous pattern matched (rm -rf /), prod host
output:
(not executed)
```

`sshops fan` 输出按主机聚合:

```
=== @prod/cvm-01 ===
exit: 0, 234ms
14:23:01 up 47 days...

=== @prod/cvm-02 ===
exit: 0, 198ms
14:23:01 up 32 days...

=== @prod/cvm-03 ===
exit: 1, 5012ms
ssh: timeout

summary: 2/3 succeeded, 1 failed
```

## 12. 文件结构

```
~/.claude/skills/ssh-ops/
├── SKILL.md                          # Claude 决策手册(中文)
├── README.md                         # 用户安装与使用文档
├── bin/
│   ├── sshops                        # 主入口,bash
│   └── sshops-setup                  # 初始化向导,bash
├── lib/
│   ├── common.sh                     # 工具函数、配置读取、密码后端
│   ├── crt.sh                        # SecureCRT .ini 解析
│   ├── selector.sh                   # 三种选择器归一
│   ├── wezterm.sh                    # WezTerm cli 封装
│   ├── marker.sh                     # 命令注入 + 输出切片
│   ├── layout.sh                     # 布局计算
│   ├── project.sh                    # 项目识别 + 窗口管理
│   ├── safety.sh                     # 危险命令拦截
│   ├── recorder.sh                   # 录像 / 索引 / 回放控制
│   ├── fan.sh                        # 并发 fan-out
│   ├── health.sh                     # 健康检查
│   ├── forward.sh                    # 端口转发
│   └── transfer.sh                   # scp 包装
├── lua/
│   ├── wezterm-ai-tag.lua            # AI/HUMAN/REPLAY pane 视觉标识
│   ├── wezterm-replay-keys.lua       # 回放快捷键 key_table
│   └── wezterm-integration.lua       # 总入口,用户 source 进自己的 wezterm.lua
├── config.example.json               # 配置模板
├── state/                            # 运行时状态(.gitignore)
│   ├── panes.json
│   └── jobs.json
├── install.sh                        # 一键安装 + 自检
└── tests/
    └── self-test.sh                  # marker 切片、录像、回放冒烟测试
```

## 13. config.json 完整规范

```json
{
  "securecrt_config_dir": "/Users/u/Work/安全工具/SecureCRT/Config",
  "securecrt_sessions_dir": "/Users/u/Work/安全工具/SecureCRT/Config/Sessions",
  "path_mappings": [
    { "from": "C:\\Users\\u\\keys", "to": "~/.ssh/keys" }
  ],
  "default_parallel": 8,
  "log_dir": "~/.ssh-recordings",
  "marker_timeout_seconds": 30,
  "prompt_marker": "SSHOPS_READY$ ",
  "recorder": {
    "tool": "asciinema"
  },
  "retention": {
    "raw_days": 7,
    "compressed_days": 90,
    "keep_index_forever": true,
    "exempt_patterns": ["prod/**"]
  },
  "prod_keywords": ["prod", "production", "生产", "live"],
  "dangerous_patterns": [
    "rm\\s+-rf\\s+/",
    "rm\\s+-rf\\s+~",
    "mkfs\\.",
    "dd\\s+.*of=/dev/[sh]d",
    "shutdown",
    "reboot",
    "halt",
    "ncli\\s+cluster\\s+destroy",
    "cluster\\s+destroy",
    ":\\(\\)\\s*\\{",
    "chmod\\s+-R\\s+777\\s+/",
    ">\\s*/dev/[sh]d",
    "wipefs"
  ],
  "ssh_options_base": [
    "-o ServerAliveInterval=30",
    "-o ServerAliveCountMax=3",
    "-o StrictHostKeyChecking=accept-new"
  ],
  "ssh_options_control_master": [
    "-o ControlMaster=auto",
    "-o ControlPath=~/.claude/skills/ssh-ops/state/cm-%r@%h:%p",
    "-o ControlPersist=10m"
  ],
  "wezterm": {
    "cli_path": "wezterm",
    "spawn_strategy": "split-in-project-window",
    "ai_pane_color": "#cc3333",
    "human_pane_color": "#33aa33",
    "replay_pane_color": "#888888"
  }
}
```

**`ssh_options_*` 拆分说明**:

由 `lib/common.sh` 的 `ssh_opts_for <purpose>` 函数按用途组合:

| purpose | 用途 | 选项 |
|---|---|---|
| `pane` | pane 内长 ssh(`open`/`run` 主路径) | `_base` 单独 |
| `transfer` | `push`/`pull`(scp) | `_base` + `_control_master` |
| `health` | `health` 健康检查 | `_base` + `_control_master` |
| `forward` | `forward`(本身长连) | `_base` 单独 |
| `fan` | `fan` 并发分发 | `_base` 单独(并发首连,Master 反而引入串行化) |

## 14. WezTerm Lua 集成

用户在 `~/.config/wezterm/wezterm.lua` 里添加一行 source:

```lua
local sshops = require('sshops-integration')
return sshops.apply(config)
```

`lua/wezterm-integration.lua` 的职责:

- 注册 user var hooks:`sshops_actor`(ai/human/replay)、`sshops_dangerous`、`sshops_session_id`
- 根据 user var 动态改 tab 标题、pane 边框色
- 注册 `replay_mode` key_table(第 8.3 节列的所有快捷键)
- 提供 `wezterm.action.EmitEvent` 钩子,让 skill 可以触发 Lua 端的动作(比如闪红边框)

skill 通过 `wezterm cli set-user-var` 给 pane 打 user var,Lua 端响应。

## 15. 安装与初始化

### 15.1 install.sh

跑 `bash install.sh` 时:

1. 检测依赖:`wezterm` `ssh` `sshpass` `jq` `script` `asciinema`(可选) `pass`(可选) `gzip`
2. 缺什么打什么,给 brew/apt/dnf 命令提示
3. 把 skill 目录复制 / 链接到 `~/.claude/skills/ssh-ops/`
4. 调用 `sshops setup`

### 15.2 sshops setup

交互式问:

1. SecureCRT Sessions 目录路径(默认 `~/Library/Application Support/VanDyke/SecureCRT/Config/Sessions`,验证存在且能读到 `.ini`)
2. 是否需要 Windows→本机 路径映射?如果是,问 from 和 to
3. 录像目录(默认 `~/.ssh-recordings`)
4. 录像工具(asciinema/script,自动检测优先)
5. 是否启用 `pass` / `keychain` 后端
6. 默认并发数

写 `config.json`,然后跑 self-test。

### 15.3 self-test

冒烟测试:

1. 列 SecureCRT 前 3 个 session 解析是否正常
2. spawn 一个临时 pane 连 localhost(`ssh localhost`,需要用户已配 key 到自己),发 `echo hello`,验证 marker 切片
3. 验证录像文件生成 + commands.jsonl 写入
4. 用 `sshops log replay --text` 回放刚才那条命令
5. 全部通过则报 OK

## 16. 错误处理

所有 skill 命令的错误处理原则:

- 失败必非零 exit(POSIX 进程 exit code 取值 0..255,不允许负数)
- stderr 写中文 + 英文双语错误信息(中文 for 用户,英文便于 grep)
- 关键错误码:
  - `1` 一般错误
  - `2` 选择器解析失败 / 跳板机递归超限或循环
  - `3` 主机不可达 / SSH 失败 / 目标 shell 不支持
  - `4` 命令注入超时(应改用 bg)
  - `5` 危险命令被拦截(进程层)
  - `6` WezTerm 不可用
  - `7` 录像文件损坏
  - `64` 命令行参数错误

**JSON 输出层的 `exit` 字段**(见 11.3)语义不同于进程 exit code:
- `exit: 0..255` 表示远端命令的真实 exit code
- `exit: -1` **专用于** `blocked: true` 的拦截记录,语义为「未实际执行,无远端 exit code」
- 进程 exit code 5 与 JSON `exit: -1` 同时存在不矛盾:**进程层告知调用者"被拦截",JSON 层告知"未执行"**

Claude 据此决策。

## 17. SKILL.md 内容大纲(给 Claude 看的)

详细写明:

1. 触发条件
2. 主机解析三种入口的优先级
3. 项目 = WezTerm 窗口的语义
4. 命令决策树:短命令用 run / 长命令用 bg / 多机用 fan / 等等
5. 危险命令处理流程(必须先 echo 等用户确认)
6. TUI 类命令避免 + 替代方案表
7. sudo / 首次连接 / .ppk / 跳板机等常见坑
8. 录像与回放命令速查
9. 不该用本 skill 的场景

## 18. 测试要求

`tests/self-test.sh` 必须覆盖:

- [ ] 空 SecureCRT 目录的优雅降级
- [ ] 多个候选时模糊匹配的报错
- [ ] 临时参数三种(key / password / ask-password)
- [ ] marker 切片在有 ANSI 颜色输出时正常
- [ ] marker 切片在命令本身包含 marker 字符串时不误判(用更长 nonce)
- [ ] 危险命令在 prod 路径被拦截
- [ ] 危险命令在非 prod 路径只警告
- [ ] 跳板机 ProxyJump 解析(模拟一个 firewall .ini)
- [ ] Windows path mapping 转换
- [ ] `.ppk` 检测报错
- [ ] 录像文件 cast_offset 与 commands.jsonl 时间一致
- [ ] 回放 --text 模式输出格式
- [ ] sync 抓回 human 命令并写入索引
- [ ] retention gc 不删 prod 录像
- [ ] 多 CC 项目 → 多 WezTerm 窗口
- [ ] WezTerm 未启动时自动拉起

## 19. 实现优先级

如果不能一次实现全部,按这个顺序分阶段:

**Phase 1a - MVP 主链(必做,优先)**

打通"通过 WezTerm 跑命令并录"的端到端,marker 切片是技术心脏,先在不依赖 SecureCRT 的路径上验证。

- [ ] 依赖检测 + 极简 setup
- [ ] **临时参数**选择器(`--host --user --key/--password/--ask-password [--port] [--prod]`)
- [ ] 项目识别(`$PWD` → 唯一 WezTerm 窗口,`flock` 写 state)
- [ ] WezTerm cli 封装(spawn / split / send-text / get-text / kill / 自动启动)
- [ ] 目标 shell 检测(仅 bash/zsh)
- [ ] marker 注入 + 切片 + ANSI strip + 30s 超时
- [ ] asciinema 录制 + commands.jsonl + meta.json
- [ ] 危险命令拦截(`--prod` 标志驱动,不依赖 SecureCRT 路径)
- [ ] `sshops open` `run` `close` `list-panes`
- [ ] 极简 SKILL.md(够 Claude 调用即可)
- [ ] self-test:`ssh localhost` + `echo hello`,验证 marker 切片与录像生成

**Phase 1b - SecureCRT 接入**

- [ ] SecureCRT .ini 解析(字段 + SSH2.ini 全局回退 + `${VDS_CONFIG_PATH}` 展开 + `path_mappings`)
- [ ] 跳板机递归(深度 ≤3 + 循环检测)
- [ ] 选择器三入口归一(`@路径` / 关键词 / 临时参数)
- [ ] 生产机判定接入(从 `@<path>` 提取 prod 关键词)
- [ ] `sshops list` `show` `crt-find`
- [ ] `.ppk` 检测报错(不自动转换)
- [ ] 完整 SKILL.md

**Phase 2 - 进阶**

- [ ] `bg` `tail` `jobs` 长任务
- [ ] `fan` 并发
- [ ] `health` 健康检查
- [ ] `sync` 抓 human 命令
- [ ] `grid` `focus` `detach` 布局
- [ ] `push` `pull` 文件传输
- [ ] `forward` 端口转发

**Phase 3 - 录像系统完善**

- [ ] `log search` 跨 session 搜索
- [ ] `log replay`(视频回放 + 快捷键)
- [ ] `log replay --text`
- [ ] `log export` 多格式
- [ ] `log annotate`
- [ ] `log gc` retention

**Phase 4 - 视觉与体验**

- [ ] WezTerm Lua 集成(tab 标识 + 边框色)
- [ ] 回放 key_table 完整快捷键
- [ ] 危险命令 pane 闪红
- [ ] OSC 8 hyperlink 在 search 输出里

## 20. 给实现者的提示

- 用 bash 写,不要引入 Python/Go 等运行时依赖
- jq 可以放心用,自检里加它
- 所有 shell 脚本加 `set -euo pipefail`
- 路径处理小心 macOS 的空格(`Application Support`)
- 测试时本机用 `ssh localhost` 起 sshd,不依赖外部主机
- WezTerm pane 的 PTY 大小要在 spawn 时考虑(默认 80x24,但 split 后会变,marker 切片不应该依赖 pane 几何)
- marker nonce 用 16 字符以上避免误匹配,且不要包含 shell 特殊字符
- 所有时间戳用 ISO 8601 + UTC,展示时再转本地时区
- 路径有空格的地方一律 quote
- jq 输出当数据用,不要管它的格式美化
- 用户的 `~/.config/wezterm/wezterm.lua` 不要自动改,只生成片段让用户自己 source

---

**这份文档目标是让另一个 Claude Code 实例(或人类工程师)直接拿去实现一个能用的 v0.1 版 ssh-ops skill,不需要再回头问需求。**

如有歧义,优先级:正确性 > 安全栏杆 > 用户体验 > 性能。
