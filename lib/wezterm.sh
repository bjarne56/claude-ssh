#!/usr/bin/env bash
# lib/wezterm.sh
# WezTerm cli 封装。所有 WezTerm 操作走这里。

if [[ -n "${_SSHOPS_WEZTERM_SOURCED:-}" ]]; then return 0; fi
_SSHOPS_WEZTERM_SOURCED=1

if [[ -z "${_SSHOPS_COMMON_SOURCED:-}" ]]; then
    # shellcheck disable=SC1091
    source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
fi

_WT_CLI=""
_wt_cli() {
    if [[ -z "$_WT_CLI" ]]; then
        _WT_CLI="$(config_get '.wezterm.cli_path' wezterm)"
    fi
    printf '%s' "$_WT_CLI"
}

# wt_check:确认 wezterm cli 可用。WezTerm 没启动时尝试 `open -a WezTerm`。
wt_check() {
    local cli; cli="$(_wt_cli)"
    if "$cli" cli list >/dev/null 2>&1; then
        return 0
    fi
    log_warn "WezTerm cli 不可用,尝试启动 WezTerm…"
    if [[ "$(uname)" == "Darwin" ]]; then
        open -a WezTerm 2>/dev/null || true
    else
        "$cli" >/dev/null 2>&1 &
        disown 2>/dev/null || true
    fi
    # 等待最多 5 秒
    local i
    for ((i=0; i<25; i++)); do
        if "$cli" cli list >/dev/null 2>&1; then
            return 0
        fi
        sleep 0.2
    done
    die 6 "WezTerm 启动失败,请手动启动后重试 / WezTerm not available"
}

# wt_list_json:列出所有窗口/tab/pane(JSON)
wt_list_json() {
    "$(_wt_cli)" cli list --format json
}

# wt_spawn_new_window <cwd> <cmd...>
# 返回新 pane id 到 stdout
wt_spawn_new_window() {
    local cwd="$1"; shift
    "$(_wt_cli)" cli spawn --new-window --cwd "$cwd" -- "$@"
}

# wt_spawn_tab <cwd> <cmd...>
# 在当前窗口创建新 tab(spawn 默认行为),返回新 pane id
wt_spawn_tab() {
    local cwd="$1"; shift
    "$(_wt_cli)" cli spawn --cwd "$cwd" -- "$@"
}

# wt_split_pane <parent_pane_id> <direction> <percent> <cmd...>
# direction: right | bottom | left | top
wt_split_pane() {
    local parent="$1" dir="$2" percent="$3"; shift 3
    "$(_wt_cli)" cli split-pane --pane-id "$parent" "--$dir" --percent "$percent" -- "$@"
}

# wt_send_text <pane_id> <text>
# 自动加 \r 触发命令执行;调用方传裸命令文本即可。
wt_send_text() {
    local pane="$1"; shift
    local text="$*"
    # --no-paste:防止 bracketed paste 模式干扰
    "$(_wt_cli)" cli send-text --pane-id "$pane" --no-paste -- "${text}"$'\r'
}

# wt_send_raw <pane_id> <text>
# 不加 \r。用于发 EOT (^D)、Ctrl-C (^C) 等控制字符。
wt_send_raw() {
    local pane="$1"; shift
    "$(_wt_cli)" cli send-text --pane-id "$pane" --no-paste -- "$*"
}

# wt_get_text <pane_id> [scrollback_lines]
# 输出 pane 内容(默认含 50000 行 scrollback, 不只是当前屏幕)
# wezterm cli get-text --start-line 负数读 scrollback (0=当前屏幕第一行)
wt_get_text() {
    local pane="$1"
    local scrollback="${2:-${SSHOPS_SCROLLBACK_LINES:-50000}}"
    "$(_wt_cli)" cli get-text --pane-id "$pane" --start-line "-${scrollback}"
}

# wt_get_text_screen <pane_id>: 仅当前屏幕(用于 prompt 检测等场景, 速度快)
wt_get_text_screen() {
    local pane="$1"
    "$(_wt_cli)" cli get-text --pane-id "$pane"
}

# wt_kill_pane <pane_id>
wt_kill_pane() {
    local pane="$1"
    "$(_wt_cli)" cli kill-pane --pane-id "$pane" 2>/dev/null || true
}

# wt_set_tab_title <pane_id> <title>
# 通过 pane 找到对应 tab,设置标题
wt_set_tab_title() {
    local pane="$1" title="$2"
    local tab_id
    tab_id="$(wt_list_json | jq -r --arg p "$pane" '.[] | select((.pane_id|tostring) == $p) | .tab_id')"
    if [[ -n "$tab_id" ]]; then
        "$(_wt_cli)" cli set-tab-title --tab-id "$tab_id" "$title" 2>/dev/null || true
    fi
}

# wt_activate <pane_id>
wt_activate() {
    local pane="$1"
    "$(_wt_cli)" cli activate-pane --pane-id "$pane" 2>/dev/null || true
}

# wt_set_user_var <pane_id> <key> <value>
# 用于 Lua 端 hook(Phase 4 视觉)
wt_set_user_var() {
    local pane="$1" key="$2" val="$3"
    "$(_wt_cli)" cli set-user-var --pane-id "$pane" "$key" "$val" 2>/dev/null || true
}

# wt_pane_alive <pane_id>:0=alive,1=不存在
wt_pane_alive() {
    local pane="$1"
    wt_list_json | jq -e --arg p "$pane" 'any(.[]; (.pane_id|tostring) == $p)' >/dev/null 2>&1
}

# wt_window_of_pane <pane_id>:打印 window_id,失败返回 1
wt_window_of_pane() {
    local pane="$1"
    wt_list_json | jq -r --arg p "$pane" '
        .[] | select((.pane_id|tostring) == $p) | .window_id
    ' | head -1
}
