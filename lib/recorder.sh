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

# 录像存储根目录:
# - 默认 = 当前项目根/.ssh-ops/recordings(跟项目绑定,clone/move 时一起带走)
# - 若 config.log_dir 显式设置非空字符串,则用配置值(高级覆盖,适合统一审计场景)
_record_log_dir() {
    local cfg
    cfg="$(config_get '.log_dir' '')"
    if [[ -n "$cfg" ]]; then
        expand_path "$cfg"
    else
        printf '%s/.ssh-ops/recordings' "$(project_id)"
    fi
}

# 首次写录像时,给项目根的 .gitignore 追加 .ssh-ops/ 排除项,避免误 commit。
# 仅当 .gitignore 已存在且不含该条时追加;不存在则不动(尊重用户偏好,不主动创建)。
_ensure_project_gitignore() {
    # 仅在录像走项目内默认路径时才动 .gitignore
    local cfg; cfg="$(config_get '.log_dir' '')"
    [[ -n "$cfg" ]] && return 0

    local proj; proj="$(project_id)"
    local gi="$proj/.gitignore"
    [[ -f "$gi" ]] || return 0

    if ! grep -qE '^\.ssh-ops/?$' "$gi" 2>/dev/null; then
        # 追加,带说明
        {
            printf '\n'
            printf '# ssh-ops skill 录像与状态(自动维护)\n'
            printf '.ssh-ops/\n'
        } >> "$gi"
        log_info "已自动追加 .ssh-ops/ 到 $gi(可手动调整)"
    fi
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
# 全局模式:<log_dir>/<project_slug>/<session_id>(按项目分子目录,避免不同项目 sid 冲突)
# 项目内模式:<project>/.ssh-ops/recordings/<session_id>(已经在项目下,不再叠 project_slug)
record_session_dir() {
    local sid="$1"
    local cfg; cfg="$(config_get '.log_dir' '')"
    local root; root="$(_record_log_dir)"
    if [[ -n "$cfg" ]]; then
        local proj; proj="$(project_slug)"
        printf '%s/%s/%s' "$root" "$proj" "$sid"
    else
        printf '%s/%s' "$root" "$sid"
    fi
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
    # 项目内模式时,自动维护项目 .gitignore
    _ensure_project_gitignore
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

# record_cast_offset <session_id>:打印当前距离 cast 真正开始的秒数(浮点)
# 优先用 cast 文件 header 的 timestamp (asciinema 真实开始时刻),
# fallback .start_ts (record_init 时刻, 早于 cast)
record_cast_offset() {
    local sid="$1"
    local dir; dir="$(record_session_dir "$sid")"
    local cast="$dir/stream.cast"
    local base=""
    if [[ -f "$cast" ]]; then
        # 第一行 header 的 timestamp 字段 (cast v3 unix 秒)
        base="$(head -1 "$cast" 2>/dev/null | jq -r '.timestamp // empty' 2>/dev/null)"
    fi
    if [[ -z "$base" ]] && [[ -f "$dir/.start_ts" ]]; then
        base="$(cat "$dir/.start_ts")"
    fi
    [[ -z "$base" ]] && { printf '0'; return 0; }
    local now; now="$(date -u '+%s.%N')"
    awk -v a="$now" -v b="$base" 'BEGIN { printf "%.3f", a - b }'
}

# record_wait_prompt_in_cast <session_id> <start_byte> <timeout_ms>
# 从 cast 字节偏移开始监听, 直到看到 shell prompt (# 或 $ 末尾) 或超时
# 不依赖 wezterm get-text — cast-recorder 的 immediate flush 让 cast 实时可读
# 返回: 0=见到 prompt, 1=超时
record_wait_prompt_in_cast() {
    local sid="$1" start_byte="$2" timeout_ms="${3:-30000}"
    local dir; dir="$(record_session_dir "$sid")"
    local cast="$dir/stream.cast"
    [[ -f "$cast" ]] || return 1

    local poll_ms=100
    local max_iters=$((timeout_ms / poll_ms))
    local i prompt_seen=0
    for ((i=0; i<max_iters; i++)); do
        local tail_text
        tail_text="$(tail -c "+$((start_byte + 1))" "$cast" 2>/dev/null | jq -rs '
            reduce .[] as $e (""; if ($e[1] == "o") then . + ($e[2] // "") else . end)
        ' 2>/dev/null | tail -c 300)"
        if printf '%s' "$tail_text" | strip_ansi | grep -qE '[#$] $'; then
            prompt_seen=$((prompt_seen + 1))
            if (( prompt_seen >= 2 )); then return 0; fi
        else
            prompt_seen=0
        fi
        sleep 0.1
    done
    return 1
}

# record_extract_human_commands <session_id> <start_byte> <end_byte>
# 扫 cast 字节区间提取用户 *手敲* 命令 (逐字符输入 = chunk_size ≤ 3)
# 整块发送 (chunk_size > 3) = AI/paste/ssh-ops 注入, 跳过
# 输出: JSON array [{cmd, cast_offset, ts_unix}, ...]
record_extract_human_commands() {
    local sid="$1" start_byte="$2" end_byte="${3:-0}"
    local dir; dir="$(record_session_dir "$sid")"
    local cast="$dir/stream.cast"
    [[ -f "$cast" ]] || { echo "[]"; return 0; }

    local cast_ts; cast_ts="$(head -1 "$cast" 2>/dev/null | jq -r '.timestamp // 0')"
    local size; size="$(wc -c < "$cast" | tr -d ' ')"
    local actual_end="$end_byte"
    if (( actual_end == 0 || actual_end > size )); then actual_end=$size; fi
    if (( start_byte >= actual_end )); then echo "[]"; return 0; fi

    # python (随系统) 做事件聚合, 比复杂 jq 更可靠
    tail -c "+$((start_byte + 1))" "$cast" 2>/dev/null \
      | head -c $((actual_end - start_byte)) \
      | CAST_TS="$cast_ts" python3 -c '
import sys, json, os
cast_ts = int(os.environ.get("CAST_TS", "0"))
elapsed = 0.0
buf = ""
buf_start = None
max_chunk = 0
groups = []

for line in sys.stdin:
    line = line.strip()
    if not line: continue
    try: e = json.loads(line)
    except: continue
    if not isinstance(e, list) or len(e) < 3: continue
    elapsed += float(e[0])
    if e[1] != "i": continue
    data = e[2] or ""
    visible = sum(1 for c in data if c.isprintable() or c in "\r\n")
    if visible > max_chunk: max_chunk = visible
    if buf_start is None: buf_start = elapsed
    for ch in data:
        if ch in ("\r", "\n"):
            cmd = buf.strip()
            if cmd:
                groups.append({"cmd": cmd, "cast_offset": round(buf_start, 3),
                              "ts_unix": cast_ts + int(buf_start), "max_chunk": max_chunk})
            buf = ""; buf_start = None; max_chunk = 0
        elif ch in ("\x7f", "\x08"):
            buf = buf[:-1]
        elif ord(ch) < 32:
            pass
        else:
            buf += ch

# 只保留 human: 逐字符 (max_chunk ≤ 3) 且长度 > 1
human = [{"cmd": g["cmd"], "cast_offset": g["cast_offset"], "ts_unix": g["ts_unix"]}
         for g in groups if g["max_chunk"] <= 3 and len(g["cmd"]) > 1]
print(json.dumps(human))
' 2>/dev/null || echo "[]"
}

# record_cast_size <session_id>: 输出 cast 文件当前字节大小 (用于切片起点)
record_cast_size() {
    local sid="$1"
    local dir; dir="$(record_session_dir "$sid")"
    local cast="$dir/stream.cast"
    [[ -f "$cast" ]] && wc -c < "$cast" | tr -d ' ' || printf '0'
}

# record_extract_output_from_byte <session_id> <start_byte>
# 从 cast 字节偏移开始读到末尾, 等 cast flush 稳定后, 提取所有 'o' 事件 data
# 不依赖时间偏移 (asciinema header.timestamp 不可靠), 只看实际写入字节
record_extract_output_from_byte() {
    local sid="$1" start_byte="$2"
    local dir; dir="$(record_session_dir "$sid")"
    local cast="$dir/stream.cast"
    [[ -f "$cast" ]] || return 1

    # 等 cast 文件大小稳定: 连续 3 次相同大小 (asciinema flush 完毕)
    # 最多等 3 秒, 100ms 粒度
    local prev=-1 same=0
    local i
    for ((i=0; i<30; i++)); do
        local cur; cur="$(wc -c < "$cast" 2>/dev/null | tr -d ' ')"
        if [[ "$cur" == "$prev" ]] && (( cur > start_byte )); then
            same=$((same + 1))
            if (( same >= 3 )); then break; fi
        else
            same=0
        fi
        prev="$cur"
        sleep 0.1
    done

    # 从 start_byte+1 读到末尾, jq 解析所有 'o' 事件
    tail -c "+$((start_byte + 1))" "$cast" 2>/dev/null | jq -rs '
        reduce .[] as $e (
            "";
            if ($e[1] == "o") then . + ($e[2] // "") else . end
        )
    ' 2>/dev/null
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
    # 用 cast-recorder (本地 fork 的 asciinema, 加了 immediate flush)
    # 解决 asciinema 默认 buffered IO 导致 cast 文件滞后 1-3s 的问题
    local recorder; recorder="$(dirname "$(readlink -f "${BASH_SOURCE[0]}" 2>/dev/null || echo "${BASH_SOURCE[0]}")")/../bin/cast-recorder"
    if [[ ! -x "$recorder" ]]; then
        # fallback: 系统 asciinema (兼容未编译 cast-recorder 的环境)
        recorder="asciinema"
    fi

    local ssh_cmd
    printf -v ssh_cmd '%q ' "$@"
    ssh_cmd="${ssh_cmd% }"
    cat <<EOF
${recorder}
rec
--quiet
--stdin
--command
${ssh_cmd}
${cast_path}
EOF
}
