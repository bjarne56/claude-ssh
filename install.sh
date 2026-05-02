#!/usr/bin/env bash
# install.sh — 依赖检测 + 链接到 ~/.claude/skills/ssh-ops/
# 用法: bash install.sh [--link-only]
set -euo pipefail

SSHOPS_SRC="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SSHOPS_DEST="$HOME/.claude/skills/ssh-ops"

ok()   { printf '\033[32m✓\033[0m %s\n' "$*"; }
warn() { printf '\033[33m!\033[0m %s\n' "$*"; }
err()  { printf '\033[31m✗\033[0m %s\n' "$*"; }

check_cmd() {
    # check_cmd <name> <required> <hint>
    local cmd="$1" required="$2" hint="$3"
    if command -v "$cmd" >/dev/null 2>&1; then
        ok "$cmd: $(command -v "$cmd")"
        return 0
    fi
    if [[ "$required" == "1" ]]; then
        err "$cmd 未安装(必需)。安装提示: $hint"
        return 1
    else
        warn "$cmd 未安装(可选)。安装提示: $hint"
        return 0
    fi
}

echo "==> 检查依赖"
fail=0
check_cmd wezterm    1 "macOS: brew install --cask wezterm  /  Linux: 见 https://wezfurlong.org/wezterm/" || fail=1
check_cmd asciinema  1 "macOS: brew install asciinema  /  Linux: pip install asciinema 或 apt install asciinema" || fail=1
check_cmd jq         1 "brew install jq" || fail=1
check_cmd ssh        1 "(自带,通常无需安装)" || fail=1
check_cmd sshpass    0 "macOS: brew install hudochenkov/sshpass/sshpass  /  Linux: apt install sshpass" || true
check_cmd perl       1 "(自带,通常无需安装)" || fail=1
check_cmd xxd        1 "(自带,通常无需安装)" || fail=1
check_cmd pass       0 "macOS: brew install pass  /  Linux: apt install pass" || true

# bash 版本:本 skill 兼容 bash 3.2 / 4 / 5 全部主流版本
# - 3.2(macOS 系统自带 /bin/bash):空数组 + set -u 用 [[ ${#arr[@]} -gt 0 ]] 守护
# - 4.x / 5.x:自动兼容(向后兼容)
ok "bash: $BASH_VERSION (3.2 / 4 / 5 全部兼容)"

if [[ "$fail" == "1" ]]; then
    err "有必需依赖缺失,先安装上面提示的工具再重试"
    exit 1
fi

echo
echo "==> 安装到 $SSHOPS_DEST"
mkdir -p "$(dirname "$SSHOPS_DEST")"

# 已存在的处理:若是 symlink 指向相同源,跳过;否则警告。
if [[ -L "$SSHOPS_DEST" ]]; then
    cur="$(readlink "$SSHOPS_DEST")"
    if [[ "$cur" == "$SSHOPS_SRC" ]]; then
        ok "已链接: $SSHOPS_DEST -> $SSHOPS_SRC"
    else
        warn "已存在 symlink 指向其它路径: $cur"
        warn "如需重链,先 rm '$SSHOPS_DEST'"
    fi
elif [[ -e "$SSHOPS_DEST" ]]; then
    err "$SSHOPS_DEST 已存在且不是 symlink,请先备份或移除"
    exit 1
else
    ln -s "$SSHOPS_SRC" "$SSHOPS_DEST"
    ok "已链接: $SSHOPS_DEST -> $SSHOPS_SRC"
fi

# bin/ 加可执行
chmod +x "$SSHOPS_SRC/bin/sshops" 2>/dev/null || true
chmod +x "$SSHOPS_SRC/bin/sshops-setup" 2>/dev/null || true
chmod +x "$SSHOPS_SRC/install.sh" 2>/dev/null || true
chmod +x "$SSHOPS_SRC/tests/self-test.sh" 2>/dev/null || true

# PATH 提示
if ! echo ":$PATH:" | grep -q ":$SSHOPS_SRC/bin:"; then
    echo
    warn "建议把以下行加进 ~/.zshrc 或 ~/.bashrc 让 sshops 命令可用:"
    echo "    export PATH=\"$SSHOPS_DEST/bin:\$PATH\""
fi

echo
echo "==> 下一步"
echo "  1) 跑 sshops setup 完成配置(若已配置可跳过)"
echo "  2) 跑 bash $SSHOPS_SRC/tests/self-test.sh 做冒烟测试(需要 ssh localhost 可登录)"
