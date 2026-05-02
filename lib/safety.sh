#!/usr/bin/env bash
# lib/safety.sh
# 危险命令模式匹配 + 生产机判定 + --i-mean-it 强制放行。

if [[ -n "${_SSHOPS_SAFETY_SOURCED:-}" ]]; then return 0; fi
_SSHOPS_SAFETY_SOURCED=1

# 依赖 lib/common.sh
if [[ -z "${_SSHOPS_COMMON_SOURCED:-}" ]]; then
    # shellcheck disable=SC1091
    source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
fi

# is_prod_host <selector>
# selector:
#   @<path>            → 路径包含 prod_keywords 任一即 prod
#   <临时参数 host>    → 调用方需额外传 --prod 标志,本函数仅看 selector
# 返回 0=prod,1=非 prod。
is_prod_host() {
    local selector="$1"
    # 临时参数模式由调用方判定(传入特殊标记)
    if [[ "$selector" == "__SSHOPS_TMP_PROD__" ]]; then return 0; fi
    if [[ "$selector" == "__SSHOPS_TMP_NONPROD__" ]]; then return 1; fi

    # @path 模式
    if [[ "$selector" =~ ^@ ]]; then
        local path="${selector:1}"
        local kw
        while IFS= read -r kw; do
            [[ -z "$kw" ]] && continue
            if [[ "$path" == *"$kw"* ]]; then return 0; fi
        done < <(config_get_array '.prod_keywords')
        return 1
    fi
    # 其他形式默认非 prod
    return 1
}

# is_dangerous_cmd <cmd>
# 返回 0=危险,1=安全;若危险,把命中的 pattern 写到 SSHOPS_DANGEROUS_REASON。
is_dangerous_cmd() {
    local cmd="$1"
    SSHOPS_DANGEROUS_REASON=""
    local pat
    while IFS= read -r pat; do
        [[ -z "$pat" ]] && continue
        if [[ "$cmd" =~ $pat ]]; then
            SSHOPS_DANGEROUS_REASON="$pat"
            return 0
        fi
    done < <(config_get_array '.dangerous_patterns')
    return 1
}

# safety_gate <selector> <cmd> [<i_mean_it_flag>]
# 拦截策略:
#   危险 + prod + 无 --i-mean-it → 拒绝执行,exit 5,JSON 写 blocked=true
#   危险 + prod + --i-mean-it    → 警告但放行,记 dangerous=true
#   危险 + 非 prod               → 警告放行,记 dangerous=true
#   非危险                       → 静默放行
# 返回:
#   0  放行
#   5  拦截(应转为进程 exit 5,JSON exit=-1, blocked=true)
# 副作用变量:
#   SSHOPS_GATE_DANGEROUS  : 0/1
#   SSHOPS_GATE_BLOCKED    : 0/1
#   SSHOPS_GATE_REASON     : 描述(中英)
safety_gate() {
    local selector="$1"
    local cmd="$2"
    local i_mean_it="${3:-0}"

    SSHOPS_GATE_DANGEROUS=0
    SSHOPS_GATE_BLOCKED=0
    SSHOPS_GATE_REASON=""

    if ! is_dangerous_cmd "$cmd"; then
        return 0
    fi

    SSHOPS_GATE_DANGEROUS=1
    local pat="$SSHOPS_DANGEROUS_REASON"

    if is_prod_host "$selector"; then
        if [[ "$i_mean_it" == "1" ]]; then
            log_warn "危险命令在生产机上执行(已用 --i-mean-it 强制放行): pattern=$pat host=$selector"
            SSHOPS_GATE_REASON="dangerous on prod, forced by --i-mean-it (pattern: $pat)"
            return 0
        fi
        SSHOPS_GATE_BLOCKED=1
        SSHOPS_GATE_REASON="危险命令在生产机被拦截 / dangerous on prod blocked (pattern: $pat). 必须加 --i-mean-it 才能执行 / require --i-mean-it"
        log_error "$SSHOPS_GATE_REASON host=$selector cmd=$cmd"
        return 5
    fi

    # 非 prod:警告放行
    log_warn "危险命令(非 prod): pattern=$pat host=$selector cmd=$cmd"
    SSHOPS_GATE_REASON="dangerous on non-prod, allowed (pattern: $pat)"
    return 0
}
