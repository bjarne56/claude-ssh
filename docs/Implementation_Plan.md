# ssh-ops 实施计划

> 任务级状态。架构层变更去 `PROJECT_OVERVIEW.md`。

状态图例:`[ ]` 待办 / `[~]` 进行中 / `[x]` 完成 / `[!]` 阻塞 / `[-]` 砍掉。

---

## Phase 1a · MVP 主链

**目标**:打通"通过 WezTerm 跑命令并录"端到端,先在临时参数路径上验证 marker 切片这个技术心脏。

### 0. 骨架与文档

- [x] git init + .gitignore
- [x] docs/PROJECT_OVERVIEW.md
- [x] docs/Implementation_Plan.md
- [x] config.example.json
- [~] 同步 A/B/C/D 决策回 `ssh-ops-requirements.md`(关键 6 处已改,剩第 18/20 节)

### 1. 依赖与安装

- [ ] `install.sh` 检测 wezterm/asciinema/jq/sshpass/ssh,缺则报错给 brew 命令
- [ ] `bin/sshops-setup` 极简向导(写 `~/.claude/skills/ssh-ops/config.json`)
- [ ] 链接到 `~/.claude/skills/ssh-ops/`

### 2. 基础库

- [ ] `lib/common.sh`:配置加载、log_*、nonce 生成、ANSI strip、`ssh_opts_for <purpose>`
- [ ] `lib/safety.sh`:`is_prod_host`、`is_dangerous_cmd`、`gate_or_die`
- [ ] `lib/wezterm.sh`:`wt_list/spawn/split/send/get/kill`,自动启动 WezTerm
- [ ] `lib/marker.sh`:`marker_inject_and_capture`,30s 超时,ANSI strip,prompt 行剔除
- [ ] `lib/project.sh`:`project_id`、`panes_state_*`(flock)、`pane_open/close`
- [ ] `lib/recorder.sh`:`record_start`、`record_append_command`、meta 维护

### 3. 命令面

- [ ] `bin/sshops open`:临时参数 spawn + 等 prompt + shell 检测
- [ ] `bin/sshops run`:`open`(若需) + 注入 + 切片 + 输出 JSON
- [ ] `bin/sshops close`:kill pane + finalize 录像
- [ ] `bin/sshops list-panes`:从 state 列当前项目所有 pane

### 4. Claude 决策手册

- [ ] `SKILL.md` 极简版(触发条件 + 命令决策树 + Phase 1a 限制清单)

### 5. 自检

- [ ] `tests/self-test.sh`:连 `ssh localhost`,`echo hello`,验证 marker 切片 + 录像生成 + commands.jsonl 写入

### 6. 提交

- [ ] git commit Phase 1a

---

## Phase 1b · SecureCRT 接入

(详见 PROJECT_OVERVIEW 第 5 节)

- [ ] `lib/crt.sh`:.ini 解析 + SSH2.ini 全局回退 + `${VDS_CONFIG_PATH}` 展开 + path_mappings
- [ ] 跳板机递归(深度 ≤3 + 循环检测)
- [ ] `lib/selector.sh`:`@path` / 关键词 / 临时参数 归一
- [ ] 生产机判定接入(从 `@<path>` 关键词)
- [ ] `sshops list` `show` `crt-find`
- [ ] `.ppk` 检测报错
- [ ] SKILL.md 完整版

---

## Phase 2 · 进阶命令

略(详见 requirements 第 19 节)。

---

## Phase 3 · 录像系统完善

略。

---

## Phase 4 · WezTerm Lua 视觉

略。

---

## 变更日志

- 2026-05-02 创建,Phase 1a 启动
