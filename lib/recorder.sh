#!/usr/bin/env bash
# lib/recorder.sh
# asciinema 录制 + commands.jsonl 索引 + meta.json 维护。
#
# 录像目录布局:
#   <log_dir>/<project_slug>/<session_id>/
#     stream.cast        ← asciinema rec 写入(spawn 时启动)
#     commands.jsonl     ← 每条命令一行,marker 切片成功后追加
#     meta.json          ← session 元数据,start/end 时更新
#     annotations.jsonl  ← 用户标注(Phase 3)
#
# session_id = <host_slug>-<YYYYMMDD-HHMMSS>-<short_id>

if [[ -n "${_SSHOPS_RECORDER_SOURCED:-}" ]]; then return 0; fi
_SSHOPS_RECORDER_SOURCED=1

if [[ -z "${_SSHOPS_COMMON_SOURCED:-}" ]]; then
    # shellcheck disable=SC1091
    source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
fi

_record_log_dir() {
    expand_path "$(config_get '.log_dir' '~/.ssh-recordings')"
}

# host_slug <selector|host>:转文件名安全
_host_slug() {
    printf '%s' "$1" | tr -c '[:alnum:]_.-' '_' | head -c 32
}

# record_session_id <host_or_selector>
record_session_id() {
    local host="$1"
    local slug; slug="$(_host_slug "$host")"
    local stamp; stamp="$(date -u '+%Y%m%d-%H%M%S')"
    local short; short="$(head -c 3 /dev/urandom | xxd -p | tr -d '\n')"
    printf '%s-%s-%s' "$slug" "$stamp" "$short"
}

# record_session_dir <session_id>
record_session_dir() {
    local sid="$1"
    local proj; proj="$(project_slug)"
    printf '%s/%s/%s' "$(_record_log_dir)" "$proj" "$sid"
}

# record_init <session_id> <selector> <host> <user> <auth_type> [extra_meta_json]
# 创建录像目录、写 meta.json。返回 0=ok。
# 录像目录返回到 stdout。
record_init() {
    local sid="$1" selector="$2" host="$3" user="$4" auth_type="$5"
    local extra="${6:-{\}}"
    local dir; dir="$(record_session_dir "$sid")"
    mkdir -p "$dir"
    : > "$dir/commands.jsonl"
    : > "$dir/annotations.jsonl"
    local now; now="$(date -u '+%Y-%m-%dT%H:%M:%SZ')"
    local proj_id; proj_id="$(project_id)"
    local proj_slug; proj_slug="$(project_slug)"
    jq -n \
        --arg sid "$sid" \
        --arg proj "$proj_slug" \
        --arg proj_path "$proj_id" \
        --arg sel "$selector" \
        --arg host "$host" \
        --arg user "$user" \
        --arg auth "$auth_type" \
        --arg now "$now" \
        --argjson extra "$extra" \
        '{
            session_id: $sid,
            project: $proj,
            project_path: $proj_path,
            host_selector: $sel,
            host_resolved: $host,
            user: $user,
            auth_type: $auth,
            started_at: $now,
            ended_at: null,
            wezterm_pane_id: null,
            command_count: 0,
            ai_command_count: 0,
            human_command_count: 0,
            dangerous_count: 0,
            blocked_count: 0,
            tags: []
        } * $extra' > "$dir/meta.json"
    # 录制起始 unix 时间戳(用于算 cast_offset),写到 .start
    date -u '+%s.%N' > "$dir/.start_ts"
    printf '%s' "$dir"
}

# record_set_pane <session_id> <pane_id>
# spawn 后把 wezterm pane_id 写进 meta
record_set_pane() {
    local sid="$1" pane="$2"
    local dir; dir="$(record_session_dir "$sid")"
    [[ -f "$dir/meta.json" ]] || return 1
    local tmp; tmp="$(mktemp "${SSHOPS_TMP_DIR}/meta.XXXXXX")"
    jq --argjson p "$pane" '.wezterm_pane_id = $p' "$dir/meta.json" > "$tmp" && mv "$tmp" "$dir/meta.json"
}

# record_cast_offset <session_id>:打印当前距离录制开始的秒数(浮点)
record_cast_offset() {
    local sid="$1"
    local dir; dir="$(record_session_dir "$sid")"
    [[ -f "$dir/.start_ts" ]] || { printf '0'; return 0; }
    local start; start="$(cat "$dir/.start_ts")"
    local now; now="$(date -u '+%s.%N')"
    awk -v a="$now" -v b="$start" 'BEGIN { printf "%.3f", a - b }'
}

# record_append_command <session_id> <actor> <host> <cmd> <exit> <duration_ms> <dangerous> <blocked> <nonce>
record_append_command() {
    local sid="$1" actor="$2" host="$3" cmd="$4" exit_code="$5"
    local dur="$6" dangerous="$7" blocked="$8" nonce="$9"
    local dir; dir="$(record_session_dir "$sid")"
    local now; now="$(date -u '+%Y-%m-%dT%H:%M:%S.%3NZ' 2>/dev/null || date -u '+%Y-%m-%dT%H:%M:%SZ')"
    local offset; offset="$(record_cast_offset "$sid")"
    jq -nc \
        --arg ts "$now" \
        --arg actor "$actor" \
        --arg host "$host" \
        --arg cmd "$cmd" \
        --argjson exit "$exit_code" \
        --argjson dur "$dur" \
        --argjson off "$offset" \
        --argjson dgr "$dangerous" \
        --argjson blk "$blocked" \
        --arg nonce "$nonce" \
        '{
            ts: $ts, actor: $actor, host: $host, cmd: $cmd,
            exit: $exit, duration_ms: $dur, cast_offset: $off,
            dangerous: ($dgr == 1), blocked: ($blk == 1), nonce: $nonce
        }' >> "$dir/commands.jsonl"

    # 更新 meta 计数
    local tmp; tmp="$(mktemp "${SSHOPS_TMP_DIR}/meta.XXXXXX")"
    jq \
        --arg actor "$actor" \
        --argjson dgr "$dangerous" \
        --argjson blk "$blocked" \
        '
        .command_count += 1
        | (if $actor == "ai" then .ai_command_count += 1 else . end)
        | (if $actor == "human" then .human_command_count += 1 else . end)
        | (if $dgr == 1 then .dangerous_count += 1 else . end)
        | (if $blk == 1 then .blocked_count += 1 else . end)
        ' "$dir/meta.json" > "$tmp" && mv "$tmp" "$dir/meta.json"
}

# record_finalize <session_id>:写 ended_at
record_finalize() {
    local sid="$1"
    local dir; dir="$(record_session_dir "$sid")"
    [[ -f "$dir/meta.json" ]] || return 0
    local now; now="$(date -u '+%Y-%m-%dT%H:%M:%SZ')"
    local tmp; tmp="$(mktemp "${SSHOPS_TMP_DIR}/meta.XXXXXX")"
    jq --arg now "$now" '.ended_at = $now' "$dir/meta.json" > "$tmp" && mv "$tmp" "$dir/meta.json"
}

# record_build_spawn_cmd <session_dir> <ssh_args_string>
# 返回完整的 spawn 命令(asciinema rec 包 ssh)
# 调用方:eval 或 -- 透传
# 注意 wezterm spawn 的参数是 -- <argv>,不是 shell string
# 这里返回字符串,调用方 split 成 argv
record_build_spawn_argv() {
    local cast_path="$1"; shift
    # 参数为 ssh_argv...
    # 输出格式:asciinema rec --quiet --stdin --command "<ssh ...>" "$cast_path"
    # 但 --command 接 shell string,我们需要 quote ssh argv
    local ssh_cmd
    printf -v ssh_cmd '%q ' "$@"
    # 去掉末尾空格
    ssh_cmd="${ssh_cmd% }"
    # 输出:逐项一行,调用方 readarray
    cat <<EOF
asciinema
rec
--quiet
--stdin
--command
${ssh_cmd}
${cast_path}
EOF
}
