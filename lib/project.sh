#!/usr/bin/env bash
# lib/project.sh
# 项目识别 + WezTerm 窗口/pane 生命周期 + state(panes.json)管理。
#
# panes.json 结构:
# {
#   "<project_id>": {
#     "wezterm_window_id": 17,
#     "started_at": "2026-05-02T10:00:00Z",
#     "panes": {
#       "<selector>": { "pane_id": 42, "session_id": "...", "started_at": "..." }
#     }
#   }
# }

if [[ -n "${_SSHOPS_PROJECT_SOURCED:-}" ]]; then return 0; fi
_SSHOPS_PROJECT_SOURCED=1

_lib_dir="$(dirname "${BASH_SOURCE[0]}")"
# shellcheck disable=SC1091
source "$_lib_dir/common.sh"
# shellcheck disable=SC1091
source "$_lib_dir/wezterm.sh"
# shellcheck disable=SC1091
source "$_lib_dir/recorder.sh"
# shellcheck disable=SC1091
source "$_lib_dir/safety.sh"
unset _lib_dir

PANES_STATE_FILE="${SSHOPS_STATE_DIR}/panes.json"

panes_state_init() {
    if [[ ! -f "$PANES_STATE_FILE" ]]; then
        echo '{}' > "$PANES_STATE_FILE"
    fi
}

# panes_state_read:打印 panes.json 内容到 stdout
panes_state_read() {
    panes_state_init
    cat "$PANES_STATE_FILE"
}

# panes_state_write_atomic <json_string>:原子写
panes_state_write_atomic() {
    local content="$1"
    local tmp; tmp="$(mktemp "${SSHOPS_TMP_DIR}/panes.XXXXXX")"
    printf '%s' "$content" > "$tmp"
    mv "$tmp" "$PANES_STATE_FILE"
}

# panes_get_window <project_id>:打印 window_id,无则空
panes_get_window() {
    local pid="$1"
    panes_state_read | jq -r --arg p "$pid" '.[$p].wezterm_window_id // empty'
}

# panes_get_pane <project_id> <selector>:打印 pane_id,无则空
panes_get_pane() {
    local pid="$1" sel="$2"
    panes_state_read | jq -r --arg p "$pid" --arg s "$sel" '.[$p].panes[$s].pane_id // empty'
}

# panes_get_session <project_id> <selector>:打印 session_id
panes_get_session() {
    local pid="$1" sel="$2"
    panes_state_read | jq -r --arg p "$pid" --arg s "$sel" '.[$p].panes[$s].session_id // empty'
}

# panes_list <project_id>:每行一个 selector
panes_list() {
    local pid="$1"
    panes_state_read | jq -r --arg p "$pid" '.[$p].panes // {} | keys[]'
}

# _panes_set_window_inner <project_id> <window_id>(锁内调用)
_panes_set_window_inner() {
    local pid="$1" win="$2"
    local cur; cur="$(panes_state_read)"
    local now; now="$(date -u '+%Y-%m-%dT%H:%M:%SZ')"
    local new
    new="$(printf '%s' "$cur" | jq \
        --arg p "$pid" --argjson w "$win" --arg now "$now" \
        '.[$p] = (.[$p] // {}) | .[$p].wezterm_window_id = $w | .[$p].started_at //= $now | .[$p].panes //= {}')"
    panes_state_write_atomic "$new"
}

panes_set_window() {
    state_with_lock 10 _panes_set_window_inner "$@"
}

# _panes_add_pane_inner <project_id> <selector> <pane_id> <session_id>
_panes_add_pane_inner() {
    local pid="$1" sel="$2" pane="$3" sid="$4"
    local cur; cur="$(panes_state_read)"
    local now; now="$(date -u '+%Y-%m-%dT%H:%M:%SZ')"
    local new
    new="$(printf '%s' "$cur" | jq \
        --arg p "$pid" --arg s "$sel" --argjson pane "$pane" --arg sid "$sid" --arg now "$now" \
        '.[$p].panes[$s] = { pane_id: $pane, session_id: $sid, started_at: $now }')"
    panes_state_write_atomic "$new"
}

panes_add_pane() {
    state_with_lock 10 _panes_add_pane_inner "$@"
}

# _panes_remove_pane_inner <project_id> <selector>
_panes_remove_pane_inner() {
    local pid="$1" sel="$2"
    local cur; cur="$(panes_state_read)"
    local new
    new="$(printf '%s' "$cur" | jq --arg p "$pid" --arg s "$sel" 'del(.[$p].panes[$s])')"
    panes_state_write_atomic "$new"
}

panes_remove_pane() {
    state_with_lock 10 _panes_remove_pane_inner "$@"
}

# panes_cleanup_stale:清理 state 中已不存在的 pane
panes_cleanup_stale() {
    local pid; pid="$(project_id)"
    local sel pane
    while IFS= read -r sel; do
        [[ -z "$sel" ]] && continue
        pane="$(panes_get_pane "$pid" "$sel")"
        if [[ -n "$pane" ]] && ! wt_pane_alive "$pane"; then
            log_warn "清理失效 pane: $sel (pane_id=$pane)"
            panes_remove_pane "$pid" "$sel"
        fi
    done < <(panes_list "$pid")
}

# === pane 生命周期 ===

# _wait_for_pane_input_complete <pane_id> <timeout_s> <context_for_log>
# 智能轮询 pane 末尾,直到不再出现"等用户输入"特征(密码 / passphrase /
# yes/no host key / sudo password)。返回:
#   0 = 已完成(prompt 消失,可继续 marker 注入)
#   1 = 超时(用户没输入)
# 用于 ssh 登录后 + sudo 后等用户手输的两个场景。
_wait_for_pane_input_complete() {
    local pane="$1" timeout="$2" ctx="${3:-input}"
    local elapsed=0
    local notified=0
    while (( elapsed < timeout )); do
        local raw stripped tail_text
        raw="$(wt_get_text "$pane" 2>/dev/null || true)"
        stripped="$(printf '%s' "$raw" | strip_ansi)"
        tail_text="$(printf '%s' "$stripped" | tail -5)"
        if printf '%s' "$tail_text" | grep -qiE \
            '([Pp]assword|[Pp]assphrase|\[sudo\][[:space:]]+[Pp]assword|[Vv]erification code|[Cc]ode):[[:space:]]*$|\(yes/no(/\[fingerprint\])?\)\?[[:space:]]*$|[Aa]re you sure you want to'; then
            if (( notified == 0 )); then
                log_info "$ctx 在等用户输入,在 WezTerm pane $pane 完成输入,skill 自动接管 (timeout ${timeout}s)"
                notified=1
            fi
            sleep 2
            elapsed=$((elapsed + 2))
            continue
        fi
        break
    done
    (( elapsed >= timeout )) && return 1
    return 0
}

# pane_ensure_window:确保 WezTerm 运行,返回可用的窗口 id。
# 优先级:项目记录窗口 > 当前活跃窗口 > 新建窗口。
# 始终只存在一个窗口,不创建占位 pane。
pane_ensure_window() {
    wt_check
    local pid; pid="$(project_id)"

    # 1. 项目记录的窗口
    local win; win="$(panes_get_window "$pid")"
    if [[ -n "$win" ]]; then
        local exists
        exists="$(wt_list_json | jq -r --arg w "$win" '.[] | select((.window_id|tostring) == $w) | .window_id' | head -1)"
        if [[ -n "$exists" ]]; then
            printf '%s' "$win"
            return 0
        fi
        log_warn "窗口 $win 已不存在"
        local cur new
        cur="$(panes_state_read)"
        new="$(printf '%s' "$cur" | jq --arg p "$pid" 'del(.[$p])')"
        state_with_lock 10 panes_state_write_atomic "$new"
    fi

    # 2. 当前活跃窗口(复用用户已开的 WezTerm 窗口,不创建新窗口)
    local active; active="$(wt_get_active_window)"
    if [[ -n "$active" ]]; then
        panes_set_window "$pid" "$active"
        printf '%s' "$active"
        return 0
    fi

    # 3. WezTerm 在运行但没有任何窗口(不太可能,但兜底)
    local cwd; cwd="$(pwd -P)"
    local new_pane; new_pane="$(wt_spawn_new_window "$cwd" "${SHELL:-/bin/zsh}")"
    [[ -z "$new_pane" ]] && die 6 "wezterm spawn 失败"
    win="$(wt_window_of_pane "$new_pane")"
    panes_set_window "$pid" "$win"
    printf '%s' "$win"
}

# wt_get_active_window:返回当前聚焦窗口的 window_id
wt_get_active_window() {
    wt_list_json | jq -r '
        [.[] | select(.is_active == true)] | .[0].window_id // empty
    '
}

# pane_open <selector> <ssh_argv...>
# 在当前项目窗口内 split 一个 pane 跑 asciinema rec 包 ssh,等 prompt,设 PS1。
# 输出(全局):
#   SSHOPS_PANE_ID
#   SSHOPS_PANE_SESSION_ID
pane_open() {
    local selector="$1"; shift
    # 后续参数:user host port [key_path] (经 sshpass 注入密码时由调用方处理)
    local user="$1" host="$2" port="${3:-22}"; shift 3 || true
    local key_path="${1:-}"; shift || true
    local extra_ssh_args=( "$@" )

    local pid; pid="$(project_id)"

    pane_ensure_window >/dev/null

    # 已存在?
    local existing
    existing="$(panes_get_pane "$pid" "$selector")"
    if [[ -n "$existing" ]] && wt_pane_alive "$existing"; then
        SSHOPS_PANE_ID="$existing"
        SSHOPS_PANE_SESSION_ID="$(panes_get_session "$pid" "$selector")"
        return 0
    fi
    # 失效则清理
    if [[ -n "$existing" ]]; then
        panes_remove_pane "$pid" "$selector"
    fi

    # 录像准备
    local sid; sid="$(record_session_id "$host")"
    local rec_dir; rec_dir="$(record_init "$sid" "$selector" "$host" "$user" "${SSHOPS_AUTH_TYPE:-key}")"
    local cast_path="$rec_dir/stream.cast"

    # 组装 ssh 参数
    # 注:bash 3.2 + set -u 下,空数组用 "${arr[@]}" 会报 unbound variable
    # (4.4+ 才修),所以用 [[ ${#arr[@]} -gt 0 ]] 显式守护
    ssh_opts_for pane
    local ssh_argv=( ssh )
    if [[ ${#SSHOPS_SSH_OPTS[@]} -gt 0 ]]; then
        ssh_argv+=( "${SSHOPS_SSH_OPTS[@]}" )
    fi
    [[ -n "$port" && "$port" != "22" ]] && ssh_argv+=( -p "$port" )
    [[ -n "$key_path" ]] && ssh_argv+=( -i "$key_path" )
    if [[ -n "${SSHOPS_JUMP:-}" ]]; then
        ssh_argv+=( -J "$SSHOPS_JUMP" )
    fi
    if [[ ${#extra_ssh_args[@]} -gt 0 ]]; then
        ssh_argv+=( "${extra_ssh_args[@]}" )
    fi
    ssh_argv+=( "${user}@${host}" )

    # 密码模式:sshpass 包一层
    if [[ "${SSHOPS_AUTH_TYPE:-}" == "password" && -n "${SSHOPS_PASSWORD:-}" ]]; then
        if ! command -v sshpass >/dev/null 2>&1; then
            die 1 "sshpass 未安装,密码模式不可用 / sshpass missing"
        fi
        ssh_argv=( sshpass -p "$SSHOPS_PASSWORD" "${ssh_argv[@]}" )
    fi

    # asciinema rec 包 ssh
    # asciinema 的 --command 接 shell string,我们把 ssh argv 用 printf %q 转义
    local ssh_cmd
    printf -v ssh_cmd '%q ' "${ssh_argv[@]}"
    ssh_cmd="${ssh_cmd% }"

    local rec_argv=( asciinema rec --quiet --stdin --command "$ssh_cmd" "$cast_path" )

    # 策略:
    #   第一台主机 → 当前窗口新 tab(ssh 独占整屏)
    #   第二台及后续 → 在项目的第一个 tab 内 split 网格
    local win; win="$(pane_ensure_window)"
    local recorded_count
    recorded_count="$(panes_state_read | jq --arg p "$pid" '.[$p].panes | length')"
    local new_pane

    if (( recorded_count == 0 )); then
        # 第一台主机:新 tab,独占整屏
        new_pane="$(wt_spawn_tab "$(pwd -P)" "${rec_argv[@]}")"
        win="$(wt_window_of_pane "$new_pane")"
        panes_set_window "$pid" "$win"
    else
        # 后续主机:在项目窗口第一个 tab 上 split
        local tab_panes
        tab_panes="$(wt_list_json | jq --arg w "$win" '[.[] | select((.window_id|tostring) == $w)] | length')"
        if (( tab_panes == 1 )); then
            local parent
            parent="$(wt_list_json | jq -r --arg w "$win" '.[] | select((.window_id|tostring) == $w) | .pane_id')"
            new_pane="$(wt_split_pane "$parent" right 50 "${rec_argv[@]}")"
        elif (( tab_panes == 2 )); then
            local parent
            parent="$(wt_list_json | jq -r --arg w "$win" '.[] | select((.window_id|tostring) == $w) | sort_by(.pane_id) | .[0].pane_id')"
            new_pane="$(wt_split_pane "$parent" right 50 "${rec_argv[@]}")"
        else
            local second
            second="$(wt_list_json | jq -r --arg w "$win" '.[] | select((.window_id|tostring) == $w) | sort_by(.pane_id) | .[1].pane_id')"
            new_pane="$(wt_split_pane "$second" bottom 50 "${rec_argv[@]}")"
        fi
    fi

    [[ -z "$new_pane" ]] && die 6 "split-pane 失败 / split failed"
    record_set_pane "$sid" "$new_pane"

    # 等 ssh 连接 + shell 启动(给 3 秒初始余量)
    sleep 3

    # 智能等待 ssh 登录完成(用户在 pane 手输密码 / passphrase / host key 确认)
    local login_timeout="${SSHOPS_LOGIN_TIMEOUT:-120}"
    if ! _wait_for_pane_input_complete "$new_pane" "$login_timeout" "ssh 登录"; then
        wt_kill_pane "$new_pane"
        record_finalize "$sid"
        die 3 "ssh 登录超时 (${login_timeout}s),用户未完成输入 / ssh login timeout"
    fi

    # auto_sudo:登录用户非 root 时自动 sudo -i 切 root
    # 默认 true(代码层默认):新用户开箱即用;现有 config.json 没此字段也走 true
    local sudo_active=0
    local auto_sudo; auto_sudo="$(config_get '.auto_sudo' 'true')"
    if [[ "$auto_sudo" == "true" ]]; then
        local skip_user_match=0
        local skip_u
        while IFS= read -r skip_u; do
            [[ -z "$skip_u" ]] && continue
            if [[ "$user" == "$skip_u" ]]; then
                skip_user_match=1
                break
            fi
        done < <(config_get_array '.auto_sudo_skip_users')
        if (( skip_user_match == 0 )); then
            log_info "auto_sudo: $user → root via sudo -i"
            wt_send_text "$new_pane" "sudo -i"
            sleep 1
            if ! _wait_for_pane_input_complete "$new_pane" 60 "sudo -i"; then
                log_warn "sudo -i 等待超时,后续命令可能在原 user shell 跑"
            else
                sudo_active=1
            fi
        else
            log_info "auto_sudo: 跳过 ($user 在 auto_sudo_skip_users 列表)"
        fi
    fi

    # PS1 + REAL_USER 审计身份,直接发命令,不用 marker 包装
    # 默认按 bash 语法注入 PS1;若目标为 zsh,后续再补 zsh PROMPT 模板
    local real_user="claude"
    local login_label
    if (( sudo_active )); then
        login_label="root"
    else
        login_label="$user"
    fi
    if [[ "$login_label" == "$real_user" ]]; then
        wt_send_text "$new_pane" "export REAL_USER='$real_user'; export PS1='[\\u@\\h \\W]\\\$ '; clear"
    else
        wt_send_text "$new_pane" "export REAL_USER='$real_user'; export PS1='[\\u($login_label:\$REAL_USER)@\\h \\W]\\\$ '; clear"
    fi

    # 视觉标识(Phase 4 由 Lua 响应,Phase 1a 仅设 user var,无视觉效果)
    wt_set_user_var "$new_pane" "sshops_actor"      "ai"
    wt_set_user_var "$new_pane" "sshops_session_id" "$sid"

    panes_add_pane "$pid" "$selector" "$new_pane" "$sid"

    SSHOPS_PANE_ID="$new_pane"
    SSHOPS_PANE_SESSION_ID="$sid"
    return 0
}

# pane_close <selector>
pane_close() {
    local selector="$1"
    local pid; pid="$(project_id)"
    local pane; pane="$(panes_get_pane "$pid" "$selector")"
    local sid; sid="$(panes_get_session "$pid" "$selector")"
    if [[ -n "$pane" ]]; then
        wt_kill_pane "$pane"
    fi
    if [[ -n "$sid" ]]; then
        record_finalize "$sid"
    fi
    panes_remove_pane "$pid" "$selector"
}
