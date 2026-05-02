#!/usr/bin/env bash
# lib/crt.sh
# SecureCRT .ini 解析。
#
# 字段映射:
#   S:"Hostname"             → host
#   S:"Username"             → user
#   D:"[SSH2] Port"          → port (8 位 hex 优先,十进制兼容)
#   S:"Identity Filename V2" → key (空时回退到 SSH2.ini 全局)
#   S:"PublicKey Filename V2" → key (备选)
#   S:"Firewall Name"        → 跳板机引用 (Session:xxx 或 None / 空)
#   S:"Protocol Name"        → 仅 SSH2
#
# 调用 crt_parse 后,以下全局变量被设置:
#   CRT_HOST CRT_USER CRT_PORT CRT_KEY CRT_FIREWALL CRT_PROTOCOL
#   CRT_PASSWORD_PRESENT  : .ini 含非空 Password V2 字段时为 1(原本是密码登录,但 skill 不解码)

if [[ -n "${_SSHOPS_CRT_SOURCED:-}" ]]; then return 0; fi
_SSHOPS_CRT_SOURCED=1

if [[ -z "${_SSHOPS_COMMON_SOURCED:-}" ]]; then
    # shellcheck disable=SC1091
    source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
fi

crt_sessions_dir() {
    local d
    d="$(config_get '.securecrt_sessions_dir' '')"
    [[ -z "$d" ]] && return 1
    expand_path "$d"
}

crt_config_dir() {
    local d
    d="$(config_get '.securecrt_config_dir' '')"
    [[ -z "$d" ]] && return 1
    expand_path "$d"
}

# 解析 Port 字段:8 位 hex(SecureCRT 默认)→ 十进制兼容 → 默认 22
_crt_decode_port() {
    local v="$1"
    [[ -z "$v" ]] && { echo 22; return 0; }
    if [[ "$v" =~ ^[0-9a-fA-F]{8}$ ]]; then
        printf '%d\n' "$((16#$v))"
    elif [[ "$v" =~ ^[0-9]+$ ]]; then
        echo "$v"
    else
        log_warn "无法解析 Port 字段: '$v',回落 22"
        echo 22
    fi
}

# crt_get_field <ini> <field>
# 抽 S:"<field>"=<v> 或 D:"<field>"=<v>;S/D 不重要,Port 是 D 类,其余多为 S 类。
# **重要**: 字段不存在是合法情况(回返空字符串),不能让 grep exit 1 透过
# pipefail 把整个调用链拉死。函数末尾强制 return 0。
crt_get_field() {
    local ini="$1" field="$2"
    grep -F "\"$field\"=" "$ini" 2>/dev/null \
        | grep -E "^[SDB]:\"$(printf '%s' "$field" | sed 's/[][]/\\&/g')\"=" 2>/dev/null \
        | head -1 \
        | sed -E "s/^[SDB]:\"[^\"]*\"=//" || true
    return 0
}

# 从 SSH2.ini 取全局 Identity(Sessions 字段空时的回退)
_crt_global_identity() {
    local cfg_dir; cfg_dir="$(crt_config_dir 2>/dev/null)" || return 1
    local ssh2="$cfg_dir/SSH2.ini"
    [[ -f "$ssh2" ]] || return 1
    crt_get_field "$ssh2" "Identity Filename V2"
}

# 展开 SecureCRT 路径变量
_crt_expand_vars() {
    local p="$1"
    local cfg_dir; cfg_dir="$(crt_config_dir 2>/dev/null || echo '')"
    p="${p//\$\{VDS_CONFIG_PATH\}/$cfg_dir}"
    # path_mappings:Windows → 本机
    while IFS= read -r mapping_json; do
        [[ -z "$mapping_json" ]] && continue
        local from to
        from="$(printf '%s' "$mapping_json" | jq -r '.from')"
        to="$(printf '%s' "$mapping_json" | jq -r '.to')"
        [[ -z "$from" || "$from" == "null" ]] && continue
        # Windows 反斜杠正常化
        local from_norm="${from//\\/\\\\}"
        if [[ "$p" == "$from"* ]]; then
            p="${to}${p#$from}"
            p="${p//\\//}"
            p="$(expand_path "$p")"
            break
        fi
    done < <(config_get_array '.path_mappings | map(@json)' 2>/dev/null || true)
    expand_path "$p"
}

# crt_resolve_at_path <selector_with_at>
# @aws/edge → <Sessions>/aws/edge.ini
crt_resolve_at_path() {
    local rel="$1"
    rel="${rel#@}"
    local sess; sess="$(crt_sessions_dir)" || { log_error "未配置 securecrt_sessions_dir"; return 1; }
    local ini="$sess/$rel.ini"
    [[ -f "$ini" ]] || { log_error "SecureCRT session 不存在: $ini"; return 2; }
    echo "$ini"
}

# crt_fuzzy_search <keyword>
# 文件名(去 .ini)或 Hostname 字段包含关键词
# 输出每行一个 .ini 绝对路径(去重)
crt_fuzzy_search() {
    local kw="$1"
    local sess; sess="$(crt_sessions_dir)" || return 1
    [[ -d "$sess" ]] || return 1
    [[ -z "$kw" ]] && return 1

    # 用 find 而非 ls(避免 alias / 颜色干扰)
    while IFS= read -r f; do
        [[ -f "$f" ]] || continue
        local name="${f##*/}"
        name="${name%.ini}"
        if [[ "$name" == *"$kw"* ]]; then
            echo "$f"
            continue
        fi
        local h; h="$(crt_get_field "$f" "Hostname")"
        if [[ -n "$h" && "$h" == *"$kw"* ]]; then
            echo "$f"
        fi
    done < <(find "$sess" -name '*.ini' -type f 2>/dev/null) \
        | awk '!seen[$0]++'
}

# crt_relative_path <ini_abs_path>
# 把绝对 .ini 路径转为相对 Sessions 的路径(去 .ini 后缀)
crt_relative_path() {
    local ini="$1"
    local sess; sess="$(crt_sessions_dir)" || return 1
    local rel="${ini#$sess/}"
    echo "${rel%.ini}"
}

# crt_parse <ini_path>
# 解析并填全局 CRT_*。返回非零表示协议不支持 / .ppk / 必填字段缺失
crt_parse() {
    local ini="$1"
    [[ -f "$ini" ]] || { log_error "ini 不存在: $ini"; return 1; }

    CRT_HOST=""; CRT_USER=""; CRT_PORT=""; CRT_KEY=""
    CRT_FIREWALL=""; CRT_PROTOCOL=""; CRT_PASSWORD_PRESENT=0

    CRT_PROTOCOL="$(crt_get_field "$ini" "Protocol Name")"
    [[ -z "$CRT_PROTOCOL" ]] && CRT_PROTOCOL="SSH2"
    if [[ "$CRT_PROTOCOL" != "SSH2" ]]; then
        log_error "仅支持 SSH2 协议(当前: $CRT_PROTOCOL),ini=$ini"
        return 2
    fi

    CRT_HOST="$(crt_get_field "$ini" "Hostname")"
    CRT_USER="$(crt_get_field "$ini" "Username")"
    local port_raw; port_raw="$(crt_get_field "$ini" "[SSH2] Port")"
    CRT_PORT="$(_crt_decode_port "$port_raw")"

    # Identity:Sessions 空 → SSH2.ini 全局回退 → PublicKey 字段
    CRT_KEY="$(crt_get_field "$ini" "Identity Filename V2")"
    if [[ -z "$CRT_KEY" ]]; then
        CRT_KEY="$(_crt_global_identity)"
    fi
    if [[ -z "$CRT_KEY" ]]; then
        CRT_KEY="$(crt_get_field "$ini" "PublicKey Filename V2")"
    fi
    [[ -n "$CRT_KEY" ]] && CRT_KEY="$(_crt_expand_vars "$CRT_KEY")"

    # .ppk 检测
    if [[ "$CRT_KEY" == *.ppk ]]; then
        log_error ".ppk 私钥不支持,请用 puttygen 转 OpenSSH 格式: $CRT_KEY"
        return 3
    fi

    CRT_FIREWALL="$(crt_get_field "$ini" "Firewall Name")"
    [[ "$CRT_FIREWALL" == "None" ]] && CRT_FIREWALL=""
    # Phase 1b MVP 不递归跳板机,仅警告
    if [[ -n "$CRT_FIREWALL" ]]; then
        log_warn "目标配置了跳板机 (Firewall=$CRT_FIREWALL),Phase 1b MVP 暂不递归解析,尝试直连"
    fi

    [[ -z "$CRT_HOST" ]] && { log_error "Hostname 字段空: $ini"; return 1; }
    [[ -z "$CRT_USER" ]] && { log_error "Username 字段空: $ini"; return 1; }

    # 检测 Password V2 是否非空(用于"密码登录但无法解码"提示)
    local pwd_v2; pwd_v2="$(crt_get_field "$ini" "Password V2")"
    if [[ -n "$pwd_v2" ]]; then
        CRT_PASSWORD_PRESENT=1
    fi
    return 0
}
