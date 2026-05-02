#!/usr/bin/env bash
# lib/common.sh
# 配置加载、日志、nonce、ANSI strip、ssh 选项组装、文件锁、路径展开。
# 调用方负责 set -euo pipefail。

# 防多次 source
if [[ -n "${_SSHOPS_COMMON_SOURCED:-}" ]]; then return 0; fi
_SSHOPS_COMMON_SOURCED=1

# === 路径常量 ===
# 允许调用者覆盖 SSHOPS_HOME(用于安装到 ~/.claude/skills/ssh-ops 后保持源码可定位)
: "${SSHOPS_HOME:=}"
if [[ -z "$SSHOPS_HOME" ]]; then
    SSHOPS_HOME="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
fi
export SSHOPS_HOME

SSHOPS_STATE_DIR="${SSHOPS_HOME}/state"
SSHOPS_TMP_DIR="${SSHOPS_HOME}/tmp"
SSHOPS_CONFIG="${SSHOPS_CONFIG:-$SSHOPS_HOME/config.json}"
SSHOPS_LOCK_DIR="${SSHOPS_STATE_DIR}/.lock.d"

mkdir -p "$SSHOPS_STATE_DIR" "$SSHOPS_TMP_DIR"

# === 日志 ===
_ts() { date -u '+%Y-%m-%dT%H:%M:%SZ'; }
log_info()  { printf '[%s] INFO  %s\n' "$(_ts)" "$*" >&2; }
log_warn()  { printf '[%s] WARN  %s\n' "$(_ts)" "$*" >&2; }
log_error() { printf '[%s] ERROR %s\n' "$(_ts)" "$*" >&2; }
log_debug() {
    if [[ "${SSHOPS_DEBUG:-0}" == "1" ]]; then
        printf '[%s] DEBUG %s\n' "$(_ts)" "$*" >&2
    fi
}

# 退出辅助:die <exit_code> <message...>
die() {
    local code="${1:-1}"; shift || true
    log_error "$*"
    exit "$code"
}

# === 配置 ===

require_config() {
    if [[ ! -f "$SSHOPS_CONFIG" ]]; then
        die 1 "配置文件不存在: $SSHOPS_CONFIG (先跑 sshops setup) / config not found"
    fi
}

# config_get <jq_path> [default]
# jq_path 形如 .marker_timeout_seconds
config_get() {
    require_config
    local path="$1"
    local default="${2-}"
    local val
    val="$(jq -r "$path // empty" "$SSHOPS_CONFIG" 2>/dev/null || true)"
    if [[ -z "$val" || "$val" == "null" ]]; then
        printf '%s' "$default"
    else
        printf '%s' "$val"
    fi
}

# config_get_array <jq_path>:每项一行到 stdout
config_get_array() {
    require_config
    local path="$1"
    jq -r "(${path} // []) | .[]" "$SSHOPS_CONFIG"
}

# === ssh 选项组装 ===
# 用法:
#   ssh_opts_for pane
#   ssh "${SSHOPS_SSH_OPTS[@]}" user@host
# purpose: pane | transfer | health | forward | fan
ssh_opts_for() {
    local purpose="$1"
    SSHOPS_SSH_OPTS=()
    local line
    while IFS= read -r line; do
        [[ -n "$line" ]] && SSHOPS_SSH_OPTS+=("$line")
    done < <(config_get_array '.ssh_options_base')
    case "$purpose" in
        transfer|health)
            while IFS= read -r line; do
                [[ -n "$line" ]] && SSHOPS_SSH_OPTS+=("$line")
            done < <(config_get_array '.ssh_options_control_master')
            ;;
        pane|forward|fan|"")
            : # 仅 _base
            ;;
        *)
            log_warn "未知 ssh purpose: $purpose,仅用 _base"
            ;;
    esac
}

# === ANSI strip ===
# 从 stdin 读,strip 后到 stdout。处理 CSI / OSC / 字符集切换。
strip_ansi() {
    # 用 perl 因为 sed 在 macOS BSD 上对 \x1B 支持不一致
    perl -pe '
        s/\x1B\[[0-9;?]*[A-Za-z]//g;          # CSI
        s/\x1B\][^\x07\x1B]*(\x07|\x1B\\)//g; # OSC
        s/\x1B[()*+][A-Za-z0-9]//g;           # 字符集切换
        s/\r$//;                               # 行尾 CR
    '
}

# === nonce ===
# 16 字符 hex,用于 marker。
gen_nonce() {
    head -c 8 /dev/urandom | xxd -p | tr -d '\n'
}

# === 路径展开 ===
# 展开 ~ 与 ${VAR};不用 eval 防注入。
expand_path() {
    local p="$1"
    if [[ "$p" == "~" ]]; then
        p="$HOME"
    elif [[ "$p" =~ ^~/ ]]; then
        p="${HOME}/${p:2}"
    fi
    while [[ "$p" =~ \$\{([A-Za-z_][A-Za-z0-9_]*)\} ]]; do
        local var="${BASH_REMATCH[1]}"
        local val="${!var-}"
        p="${p//\$\{${var}\}/${val}}"
    done
    printf '%s' "$p"
}

# === 文件锁(无 flock 依赖,用 mkdir 原子性) ===
# state_with_lock <timeout_seconds> <function_or_command> [args...]
# 在锁内执行命令(直接 exec,不走 eval),返回命令的退出码。
# 超时则 die 1。
state_with_lock() {
    local timeout="${1:-10}"
    shift
    local lock="$SSHOPS_LOCK_DIR/state"
    mkdir -p "$SSHOPS_LOCK_DIR"
    local waited=0
    until mkdir "$lock" 2>/dev/null; do
        if (( waited >= timeout * 20 )); then
            die 1 "获取 state 锁超时 ($timeout s) / state lock timeout"
        fi
        sleep 0.05
        waited=$((waited + 1))
    done
    local rc=0
    "$@" || rc=$?
    rmdir "$lock" 2>/dev/null || true
    return $rc
}

# === 项目根路径 ===
# 项目 ID = realpath $PWD,可被 SSHOPS_PROJECT 覆盖。
project_id() {
    if [[ -n "${SSHOPS_PROJECT:-}" ]]; then
        printf '%s' "$SSHOPS_PROJECT"
    else
        local p
        p="$(pwd -P)"
        printf '%s' "$p"
    fi
}

# project_slug:把 project_id 转为文件名安全的短标识(用于录像目录名)
project_slug() {
    local id
    id="$(project_id)"
    basename "$id" | tr -c '[:alnum:]_.-' '_' | head -c 64
}

# === 杂项 ===

# realpath 的便携实现(macOS 12 之前 realpath 在 GNU coreutils 包里)
real_path() {
    local p="$1"
    if command -v realpath >/dev/null 2>&1; then
        realpath "$p"
    else
        # python3 fallback
        python3 -c "import os,sys; print(os.path.realpath(sys.argv[1]))" "$p"
    fi
}

# now_ms:当前 unix 毫秒时间戳。BSD date 不支持 %3N,用 perl/python3 fallback。
now_ms() {
    perl -MTime::HiRes -e 'printf "%.0f", Time::HiRes::time*1000' 2>/dev/null \
        || python3 -c 'import time; print(int(time.time()*1000))'
}
