#!/usr/bin/env bash
# tests/self-test.sh
# Phase 1a 冒烟测试。
# 前置:
#   - WezTerm 已安装并能 GUI 启动
#   - asciinema / jq / ssh / perl / xxd 已装
#   - ssh localhost 能用当前用户免密登录(key 已加进 ~/.ssh/authorized_keys)
#   - 已跑过 sshops setup 或 install.sh
#
# 验证项:
#   1. wezterm cli 可用
#   2. ssh localhost echo 直接跑通(基线)
#   3. sshops run --host localhost --user $USER "echo hello-from-sshops" 跑通
#   4. JSON 输出 exit=0,output 含 "hello-from-sshops"
#   5. 录像文件 stream.cast 生成,commands.jsonl 有一条记录
#   6. sshops close 清理

set -euo pipefail

SSHOPS_HOME="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
SSHOPS="$SSHOPS_HOME/bin/sshops"
USER_NAME="${USER:-$(id -un)}"

# 颜色
RED=$'\033[31m'; GRN=$'\033[32m'; YEL=$'\033[33m'; RST=$'\033[0m'
ok()  { printf '%s✓%s %s\n' "$GRN" "$RST" "$*"; }
ng()  { printf '%s✗%s %s\n' "$RED" "$RST" "$*" >&2; exit 1; }
inf() { printf '%s•%s %s\n' "$YEL" "$RST" "$*"; }

cleanup() {
    set +e
    "$SSHOPS" close --host localhost --user "$USER_NAME" --port 22 >/dev/null 2>&1
}
trap cleanup EXIT

inf "==> 1) 环境检查"
command -v wezterm >/dev/null   || ng "wezterm 未安装"
command -v asciinema >/dev/null || ng "asciinema 未安装"
command -v jq >/dev/null        || ng "jq 未安装"
ok "工具齐"

inf "==> 2) wezterm cli 可用"
wezterm cli list >/dev/null 2>&1 || ng "wezterm cli 不通(GUI 是否在跑?)"
ok "wezterm cli ok"

inf "==> 3) ssh localhost 直连(基线)"
out="$(ssh -o BatchMode=yes -o StrictHostKeyChecking=accept-new \
        -o ConnectTimeout=5 "$USER_NAME"@localhost 'echo baseline-ok' 2>&1)" \
    || ng "ssh localhost 失败,先确保本机 key 已 authorized_keys: $out"
[[ "$out" == *"baseline-ok"* ]] || ng "基线输出异常: $out"
ok "ssh localhost ok"

inf "==> 4) 配置文件就位"
[[ -f "$SSHOPS_HOME/config.json" ]] || ng "config.json 不存在,先跑 sshops setup"
ok "config.json ok"

inf "==> 5) sshops run echo"
result_json="$("$SSHOPS" run --host localhost --user "$USER_NAME" --port 22 \
    "echo hello-from-sshops" 2>&1)" || {
    echo "$result_json" >&2
    ng "sshops run 失败"
}
echo "$result_json" | jq . >/dev/null || ng "输出不是合法 JSON: $result_json"

exit_code="$(echo "$result_json" | jq -r '.exit')"
output="$(echo "$result_json" | jq -r '.output')"
sid="$(echo "$result_json" | jq -r '.session_id')"

[[ "$exit_code" == "0" ]] || ng "exit != 0: $exit_code"
[[ "$output" == *"hello-from-sshops"* ]] || ng "output 不含期望字符串: $output"
ok "marker 切片输出正确: '$output'"
ok "session_id: $sid"

inf "==> 6) 录像文件检查"
log_dir="$(jq -r '.log_dir' "$SSHOPS_HOME/config.json" \
    | sed "s|^~|$HOME|")"
proj_slug="$(basename "$(pwd -P)" | tr -c '[:alnum:]_.-' '_' | head -c 64)"
session_dir="$log_dir/$proj_slug/$sid"

[[ -d "$session_dir" ]] || ng "session 目录缺失: $session_dir"
[[ -f "$session_dir/stream.cast" ]] || ng "stream.cast 缺失"
[[ -s "$session_dir/stream.cast" ]] || ng "stream.cast 空"
[[ -f "$session_dir/commands.jsonl" ]] || ng "commands.jsonl 缺失"
[[ -s "$session_dir/commands.jsonl" ]] || ng "commands.jsonl 空"
[[ -f "$session_dir/meta.json" ]] || ng "meta.json 缺失"
ok "录像三件套就位: $session_dir"

# 验证 commands.jsonl 包含 echo
last_cmd_line="$(tail -1 "$session_dir/commands.jsonl")"
last_cmd="$(echo "$last_cmd_line" | jq -r '.cmd')"
[[ "$last_cmd" == *"echo hello-from-sshops"* ]] || ng "commands.jsonl 最后一条不是预期: $last_cmd"
ok "commands.jsonl 末尾: $last_cmd"

inf "==> 7) cast 文件 sanity"
head -1 "$session_dir/stream.cast" | jq . >/dev/null || ng "stream.cast 第一行不是 JSON header"
ok "asciinema v2 header ok"

echo
ok "Phase 1a self-test 全部通过"
echo "下一步:把 sshops 加进 PATH,在新对话里让 Claude 通过 SKILL 调用本工具。"
