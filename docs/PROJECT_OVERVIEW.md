# ssh-ops 架构总览

> 跟随实现演进,记录架构层面变更。任务级状态去 `Implementation_Plan.md`。

## 1 · 系统定位

为 Claude Code 提供"通过 WezTerm 跑 SSH 远程运维"的 skill:

- 每个 CC 项目(`$PWD`)对应一个 WezTerm 窗口,每台主机对应一个 pane
- 命令通过 `wezterm cli send-text` 注入,通过 marker 切片回抓输出
- 全程 asciinema 录制,命令级索引另存

源 of truth: `ssh-ops-requirements.md`(在仓库根)。本文件只描述**实现层架构**。

## 2 · 模块拓扑

```
bin/sshops                ← CLI 主分发器
└── 子命令路由
    ├── open / run / close / list-panes      [Phase 1a]
    ├── list / show / crt-find               [Phase 1b]
    ├── bg / tail / jobs                     [Phase 2]
    ├── fan / health                         [Phase 2]
    ├── grid / focus / detach / sync         [Phase 2]
    ├── push / pull / forward                [Phase 2]
    └── log replay / search / annotate / gc  [Phase 3]

lib/
├── common.sh      ← 配置加载、日志、ssh_opts_for, ANSI strip, nonce
├── safety.sh      ← 危险命令模式 + prod 判定 + --i-mean-it
├── wezterm.sh     ← wezterm cli 封装 + 自动启动
├── marker.sh      ← BEGIN/END 注入 + 轮询 get-text + 切片
├── project.sh     ← $PWD → 项目 ID + flock state + pane 生命周期
├── recorder.sh    ← asciinema spawn + commands.jsonl + meta.json
├── crt.sh         ← SecureCRT .ini 解析 [Phase 1b]
├── selector.sh    ← @path / 关键词 / 临时参数 归一 [Phase 1b]
├── layout.sh      ← grid / tabs / stack 布局 [Phase 2]
├── fan.sh         ← 并发分发 [Phase 2]
├── health.sh      ← 健康检查 [Phase 2]
├── forward.sh     ← 端口转发 [Phase 2]
└── transfer.sh    ← scp 包装 [Phase 2]

lua/
├── wezterm-integration.lua   ← 用户 source 入口 [Phase 4]
├── wezterm-ai-tag.lua        ← AI/HUMAN/REPLAY 视觉 [Phase 4]
└── wezterm-replay-keys.lua   ← 回放 key_table [Phase 4]
```

## 3 · 关键不变量

1. **POSIX exit code 不允许负数**。进程 exit 5 = 拦截;JSON 字段 `exit: -1` 仅用于 `blocked: true` 记录,语义"未实际执行"
2. **状态文件并发安全**:所有 panes.json/jobs.json 读改写走 `flock -x state/.lock`
3. **命令边界**:每条命令注入 `__SSHOPS_BEGIN_<nonce16>__ ... __SSHOPS_END_<nonce16>__:<exit>`,nonce 16 字符 hex
4. **目标 shell 限定 bash/zsh**:spawn 后先 `echo $SHELL` 探测,不符合的 close pane 报错
5. **项目隔离**:项目 ID = `realpath $PWD`,不同目录开不同 WezTerm 窗口
6. **录像唯一方案 asciinema**,不实现 BSD/GNU script fallback

## 4 · 数据流

```
用户/Claude 调 sshops run @aws/edge "uptime"
   │
   ▼
selector 归一 → host/user/port/key/jump
   │
   ▼
project.sh 找/建窗口 + pane(若新 pane: spawn asciinema rec → ssh)
   │
   ├─ recorder.sh 启动录制(stream.cast / commands.jsonl)
   │
   ▼
marker.sh 注入 BEGIN+cmd+END → 轮询 get-text → 切片 + ANSI strip
   │
   ▼
safety.sh 拦截判定(若 prod + 危险 → blocked,不送出)
   │
   ▼
返回 JSON: {exit, duration_ms, output, session_id, cast_offset}
   │
   ▼
recorder.sh 追加 commands.jsonl
```

## 5 · 阶段路线

- **Phase 1a**(MVP):临时参数主链 + marker + 录像 + 安全栏杆
- **Phase 1b**:SecureCRT 接入 + 选择器三入口 + 跳板机
- **Phase 2**:bg/fan/health/grid/sync/transfer/forward
- **Phase 3**:回放 + 搜索 + 标注 + retention gc
- **Phase 4**:WezTerm Lua 集成 + 视觉

## 变更日志

- 2026-05-02 初始化骨架,Phase 1a 启动
