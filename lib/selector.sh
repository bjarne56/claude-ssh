#!/usr/bin/env bash
# lib/selector.sh
# 三入口选择器归一。
#
# 入口:
#   1. @<相对路径>   → SecureCRT 精确(@aws/edge)
#   2. <关键词>      → SecureCRT 模糊匹配(文件名 / Hostname 子串)
#   3. 临时参数      → --host --user [--port] [--key]
#
# 解析后填全局:
#   SEL_HOST SEL_USER SEL_PORT SEL_KEY SEL_DISPLAY SEL_SOURCE SEL_INI_PATH
#   SEL_PASSWORD_PRESENT  : 1 = .ini 原本是密码登录(但 SecureCRT Password V2 不解,
#                           需用户传 --ask-password 或在 config 配 password_ref)
# SEL_SOURCE: "crt" 或 "tmp"
# SEL_DISPLAY: 用于日志 / 录像 selector 字段(@aws/edge / user@host:port)

if [[ -n "${_SSHOPS_SELECTOR_SOURCED:-}" ]]; then return 0; fi
_SSHOPS_SELECTOR_SOURCED=1

_lib_dir="$(dirname "${BASH_SOURCE[0]}")"
# shellcheck disable=SC1091
source "$_lib_dir/common.sh"
# shellcheck disable=SC1091
source "$_lib_dir/crt.sh"
unset _lib_dir

# selector_resolve_crt <input>
# input 可能是 "@a/b" 或裸关键词(IP / 主机名 / 标签)
selector_resolve_crt() {
    local input="$1"
    local ini=""

    SEL_HOST=""; SEL_USER=""; SEL_PORT=""; SEL_KEY=""
    SEL_DISPLAY=""; SEL_SOURCE=""; SEL_INI_PATH=""; SEL_PASSWORD_PRESENT=0

    if [[ "$input" =~ ^@ ]]; then
        ini="$(crt_resolve_at_path "$input")" || return 2
        SEL_DISPLAY="$input"
    else
        local matches; matches="$(crt_fuzzy_search "$input")"
        # 计数(空字符串也会算 1 行,过滤掉)
        local n=0
        if [[ -n "$matches" ]]; then
            n="$(printf '%s\n' "$matches" | grep -cv '^[[:space:]]*$' || true)"
        fi
        if [[ "$n" == "0" ]]; then
            log_error "SecureCRT 没找到匹配主机: '$input'"
            return 2
        fi
        if [[ "$n" -gt 1 ]]; then
            log_error "关键词 '$input' 多个候选,请用 @<路径> 精确指定:" >&2
            local ml; ml="$(printf '%s\n' "$matches" | head -10)"
            local f
            while IFS= read -r f; do
                [[ -z "$f" ]] && continue
                local rel; rel="$(crt_relative_path "$f")"
                printf '  @%s\n' "$rel" >&2
            done <<< "$ml"
            return 2
        fi
        ini="$matches"
        local rel; rel="$(crt_relative_path "$ini")"
        SEL_DISPLAY="@$rel"
    fi

    crt_parse "$ini" || return $?
    SEL_HOST="$CRT_HOST"
    SEL_USER="$CRT_USER"
    SEL_PORT="$CRT_PORT"
    SEL_KEY="$CRT_KEY"
    SEL_SOURCE="crt"
    SEL_INI_PATH="$ini"
    SEL_PASSWORD_PRESENT="${CRT_PASSWORD_PRESENT:-0}"

    log_info "selector resolved: $SEL_DISPLAY → ${SEL_USER}@${SEL_HOST}:${SEL_PORT} key=${SEL_KEY:-<none>} pwd_in_crt=$SEL_PASSWORD_PRESENT"
    return 0
}

# selector_set_tmp <user> <host> <port> <key>
# 临时参数模式
selector_set_tmp() {
    SEL_USER="$1"
    SEL_HOST="$2"
    SEL_PORT="${3:-22}"
    SEL_KEY="${4:-}"
    SEL_DISPLAY="${SEL_USER}@${SEL_HOST}:${SEL_PORT}"
    SEL_SOURCE="tmp"
    SEL_INI_PATH=""
}

# selector_is_prod:基于 SEL_DISPLAY 与 prod_keywords 判定
# 用法:if selector_is_prod; then ...
selector_is_prod() {
    [[ "$SEL_SOURCE" == "tmp" ]] && return 1   # 临时参数模式由 --prod 标志单独处理
    local kw
    while IFS= read -r kw; do
        [[ -z "$kw" ]] && continue
        if [[ "$SEL_DISPLAY" == *"$kw"* ]]; then return 0; fi
    done < <(config_get_array '.prod_keywords')
    return 1
}
