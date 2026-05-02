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

- [ ] `lib/crt.sh`:.ini 解析(Hostname / Username / Port hex+decimal 兼容 / Identity / PublicKey / Firewall Name / Protocol Name)
- [ ] SSH2.ini 全局 Identity / PublicKey 回退
- [ ] `${VDS_CONFIG_PATH}` 路径变量展开
- [ ] `path_mappings`(Windows → macOS)转换
- [ ] 协议过滤:仅 SSH2,Telnet/SSH1/Serial/Rlogin 拒绝
- [ ] 跳板机递归 ≤3 层 + 循环引用检测,组装 `-J` 参数
- [ ] `.ppk` 检测报错(不自动转换)
- [ ] `lib/selector.sh`:`@path` / 关键词 / 临时参数 归一(三入口优先级)
- [ ] 生产机判定接入(从 `@<path>` 提取 prod 关键词)
- [ ] `sshops list <pattern>`、`sshops show <selector>`、`sshops crt-find <kw>`
- [ ] SKILL.md 完整版(主机解析三入口、跳板机、prod 判定、`.ppk` 等坑)
- [ ] 测试用例:`requirements.md` 第 18.2 节全覆盖

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

## 变更日志

- 2026-05-02 创建,Phase 1a 启动
- 2026-05-02 Phase 1a 实施完成(commit `9cd0881`,17 文件 / 3054 行);文档完善:README.md 创建、PROJECT_OVERVIEW 关键不变量补全、requirements 第 18/20 节补全
