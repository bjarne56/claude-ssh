#!/usr/bin/env bash
# lib/marker.sh
# 命令注入 + 输出切片。技术心脏,所有 sshops run 都走这里。
#
# 注入格式:
#   echo __SSHOPS_BEGIN_<nonce>__; <cmd>; __sshops_rc=$?; echo __SSHOPS_END_<nonce>__:$__sshops_rc
#
# 切片策略:
#   - pane 文本经 strip_ansi 后按行切
#   - 寻找精确等于 `__SSHOPS_BEGIN_<nonce>__` 的孤行(命令的回显行因为带 prompt 前缀和分号,不会精确等于)
#   - 寻找精确匹配 `__SSHOPS_END_<nonce>__:<digits>` 的孤行
#   - 中间行为 stdout/stderr 输出
#
# 输出(全局变量,调用方读取):
#   SSHOPS_MARKER_OUTPUT       命令输出文本(不含 BEGIN/END 行)
#   SSHOPS_MARKER_EXIT         远端 exit code
#   SSHOPS_MARKER_DURATION_MS  耗时(毫秒)
#   SSHOPS_MARKER_NONCE        nonce(供 commands.jsonl 记录)

if [[ -n "${_SSHOPS_MARKER_SOURCED:-}" ]]; then return 0; fi
_SSHOPS_MARKER_SOURCED=1

if [[ -z "${_SSHOPS_COMMON_SOURCED:-}" ]]; then
    # shellcheck disable=SC1091
    source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
fi
if [[ -z "${_SSHOPS_WEZTERM_SOURCED:-}" ]]; then
    # shellcheck disable=SC1091
    source "$(dirname "${BASH_SOURCE[0]}")/wezterm.sh"
fi

# marker_inject_and_capture <pane_id> <cmd> [<timeout_seconds>]
# 命令包装并发送、轮询、切片。返回:
#   0  成功(填充 SSHOPS_MARKER_*)
#   非0  超时或异常(已经 die)
marker_inject_and_capture() {
    local pane="$1"
    local cmd="$2"
    local timeout="${3:-}"
    [[ -z "$timeout" ]] && timeout="$(config_get '.marker_timeout_seconds' 30)"

    local nonce; nonce="$(gen_nonce)"
    SSHOPS_MARKER_NONCE="$nonce"
    SSHOPS_MARKER_OUTPUT=""
    SSHOPS_MARKER_EXIT=""
    SSHOPS_MARKER_DURATION_MS=""

    # 命令包装。cmd 直接拼,信任调用方(safety_gate 已处理)。
    # 用 __sshops_rc 避免污染常见变量名 rc。
    # printf + ANSI dim (\033[2m...\033[0m) 让 marker 字符串在 pane 显示为灰色,
    # 不刺眼。strip_ansi 在切片前已处理 ANSI 序列,marker awk 匹配的是纯文本,
    # 切片不受影响。
    local wrapped
    wrapped=$'printf \'\\033[2m__SSHOPS_BEGIN_'"${nonce}"$'__\\033[0m\\n\'; '"${cmd}"$'; __sshops_rc=$?; printf \'\\033[2m__SSHOPS_END_'"${nonce}"$'__:%d\\033[0m\\n\' "${__sshops_rc}"'

    local start_ms; start_ms="$(now_ms)"

    wt_send_text "$pane" "$wrapped"

    local end_marker="__SSHOPS_END_${nonce}__:"
    local raw=""
    local elapsed_ms=0
    local poll_ms=200
    local timeout_ms=$((timeout * 1000))

    while (( elapsed_ms < timeout_ms )); do
        sleep 0.2
        raw="$(wt_get_text "$pane" 2>/dev/null || true)"
        if printf '%s' "$raw" | grep -q "$end_marker"; then
            break
        fi
        elapsed_ms=$((elapsed_ms + poll_ms))
    done

    if ! printf '%s' "$raw" | grep -q "$end_marker"; then
        die 4 "命令注入超时 ${timeout}s,改用 sshops bg / marker timeout pane=$pane nonce=$nonce"
    fi

    # ANSI strip + 切片
    local stripped
    stripped="$(printf '%s' "$raw" | strip_ansi)"

    # awk 提取 BEGIN→END 之间的行,以及 END 行的 exit code。
    # 取最后一对 BEGIN/END(防 nonce 异常重复)。
    local parsed
    parsed="$(printf '%s\n' "$stripped" | awk -v nonce="$nonce" '
        BEGIN {
            begin_re = "^[[:space:]]*__SSHOPS_BEGIN_" nonce "__[[:space:]]*$"
            end_re   = "^[[:space:]]*__SSHOPS_END_"   nonce "__:[0-9]+[[:space:]]*$"
        }
        $0 ~ begin_re { in_block = 1; buf = ""; next }
        $0 ~ end_re {
            if (in_block) {
                # 截最后一对
                last_buf  = buf
                last_exit = $0
                sub(/.*:/,  "", last_exit)
                gsub(/[[:space:]]+/, "", last_exit)
                in_block = 0
            }
            next
        }
        in_block { buf = buf $0 "\n" }
        END {
            if (last_exit == "" && in_block == 1) {
                # 不应发生:有 BEGIN 没 END,但前面已经 grep 校验过
                exit 1
            }
            printf "__SSHOPS_EXIT__%s\n", last_exit
            printf "%s", last_buf
        }
    ')"

    if [[ -z "$parsed" ]]; then
        die 4 "切片失败:无法定位 BEGIN/END 标记 / marker slice failed nonce=$nonce"
    fi

    SSHOPS_MARKER_EXIT="$(printf '%s\n' "$parsed" | head -1 | sed 's/^__SSHOPS_EXIT__//')"
    SSHOPS_MARKER_OUTPUT="$(printf '%s\n' "$parsed" | tail -n +2)"
    # 去掉末尾多余换行
    SSHOPS_MARKER_OUTPUT="${SSHOPS_MARKER_OUTPUT%$'\n'}"

    local end_ms; end_ms="$(now_ms)"
    SSHOPS_MARKER_DURATION_MS=$((end_ms - start_ms))
    return 0
}

# marker_wait_for_text <pane_id> <substring> <timeout_s>
# 用于等待初始 prompt 出现等场景。返回 0=找到,4=超时。
marker_wait_for_text() {
    local pane="$1" needle="$2" timeout="${3:-15}"
    local elapsed_ms=0
    local timeout_ms=$((timeout * 1000))
    while (( elapsed_ms < timeout_ms )); do
        sleep 0.2
        if wt_get_text "$pane" 2>/dev/null | grep -q -F -- "$needle"; then
            return 0
        fi
        elapsed_ms=$((elapsed_ms + 200))
    done
    return 4
}
