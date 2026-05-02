#!/usr/bin/env bash
# tests/bash-compat.sh
# 验证所有脚本在 bash 3.2 / 4 / 5 下都能通过语法检查 + 关键 idiom 运行时 OK。
# 在 CI 或本地手工执行,作为 self-test 之外的兼容性 gate。
set -euo pipefail

cd "$(dirname "$0")/.."

RED=$'\033[31m'; GRN=$'\033[32m'; YEL=$'\033[33m'; RST=$'\033[0m'
ok()  { printf '%s✓%s %s\n' "$GRN" "$RST" "$*"; }
ng()  { printf '%s✗%s %s\n' "$RED" "$RST" "$*" >&2; }
inf() { printf '%s•%s %s\n' "$YEL" "$RST" "$*"; }

# 候选 bash 路径(macOS + Linux 常见)
BASHES=(
    /bin/bash
    /usr/local/bin/bash
    /opt/homebrew/bin/bash
)

# 待检查文件
FILES=(
    lib/common.sh
    lib/safety.sh
    lib/wezterm.sh
    lib/marker.sh
    lib/recorder.sh
    lib/project.sh
    bin/sshops
    bin/sshops-setup
    install.sh
    tests/self-test.sh
)

found_any=0
fail_total=0
for bin in "${BASHES[@]}"; do
    [[ -x "$bin" ]] || continue
    found_any=1
    ver="$("$bin" -c 'echo $BASH_VERSION')"
    inf "==> $bin ($ver)"

    # 语法检查
    fail=0
    for f in "${FILES[@]}"; do
        if "$bin" -n "$f" 2>&1; then
            :
        else
            ng "  syntax FAIL: $f"
            fail=$((fail + 1))
        fi
    done
    if [[ "$fail" == 0 ]]; then
        ok "  语法检查全过(${#FILES[@]} 个文件)"
    else
        ng "  $fail 个文件失败"
        fail_total=$((fail_total + fail))
    fi

    # idiom 运行时测试:空数组 + set -u
    if "$bin" -c '
        set -euo pipefail
        empty=()
        non=(a b)
        argv=( cmd )
        if [[ ${#empty[@]} -gt 0 ]]; then argv+=( "${empty[@]}" ); fi
        if [[ ${#non[@]}   -gt 0 ]]; then argv+=( "${non[@]}" ); fi
        argv+=( end )
        [[ "${argv[*]}" == "cmd a b end" ]]
    ' 2>&1; then
        ok "  空数组守护 idiom 运行时 OK"
    else
        ng "  idiom 运行时失败"
        fail_total=$((fail_total + 1))
    fi
done

if [[ "$found_any" == 0 ]]; then
    ng "未找到任何 bash 二进制(${BASHES[*]})"
    exit 1
fi

echo
if [[ "$fail_total" == 0 ]]; then
    ok "bash 兼容性测试全部通过"
    exit 0
else
    ng "总计 $fail_total 项失败"
    exit 1
fi
