#!/usr/bin/env bash
# install-local.sh — 本地构建并部署 ssh-ops fork (WezTerm-SSH 命名空间).
#
# 完整命名隔离:
#   bundle:        ~/Applications/WezTerm-SSH.app
#   bundle id:     com.wezterm-ssh.gui
#   GUI binary:    MacOS/WezTerm-SSH    (CFBundleExecutable, 双击 .app 走它)
#   CLI binary:    MacOS/WezTerm-SSH-cli (cli list / send-text / spawn 等子命令)
#   mux binary:    MacOS/WezTerm-SSH-mux (mux server)
#   runtime dir:   ~/.local/share/WezTerm-SSH/  (跟上游 wezterm 的 ~/.local/share/wezterm/ 隔离)
#
# Cargo crate 名仍是 wezterm/wezterm-gui/wezterm-mux-server (不动, 避免内部
# use 引用全改). 改名只发生在 cp 阶段.
#
# 用法:
#   bash wezterm-src/install-local.sh           # 全流程
#   bash wezterm-src/install-local.sh --no-build  # 只部署
#
# 幂等: 反复跑只会覆盖同名产物.

set -euo pipefail

WEZSRC="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
APP_DIR="$HOME/Applications/WezTerm-SSH.app"
APP_MACOS="$APP_DIR/Contents/MacOS"
LOCAL_BIN="$HOME/.local/bin"

ok()   { printf '\033[32m✓\033[0m %s\n' "$*"; }
warn() { printf '\033[33m!\033[0m %s\n' "$*"; }
err()  { printf '\033[31m✗\033[0m %s\n' "$*" >&2; }
inf()  { printf '\033[36m•\033[0m %s\n' "$*"; }

if [[ "$(uname)" != "Darwin" ]]; then
    err "本脚本仅支持 macOS (Linux 用 ci/deploy.sh)"
    exit 1
fi

# === 1. 编译 ===
if [[ "${1:-}" != "--no-build" ]]; then
    inf "==> cargo build --release (wezterm + wezterm-gui + wezterm-mux-server)"
    cd "$WEZSRC"
    cargo build --release -p wezterm -p wezterm-gui -p wezterm-mux-server
    ok "编译完成"
else
    inf "跳过编译 (--no-build)"
fi

REL="$WEZSRC/target/release"
for bin in wezterm wezterm-gui wezterm-mux-server ; do
    if [[ ! -x "$REL/$bin" ]]; then
        err "缺少 $REL/$bin (先去掉 --no-build, 或检查 cargo build 失败)"
        exit 1
    fi
done

# === 2. 清理旧 wezterm/sshops 部署 (如果存在) ===
inf "==> 清理旧部署"
for old_app in "$HOME/Applications/WezTerm.app" "$HOME/Applications/sshops.app"; do
    if [[ -d "$old_app" ]]; then
        rm -rf "$old_app"
        warn "  删除旧 bundle: $old_app"
    fi
done

# === 3. 部署 .app bundle ===
inf "==> 部署到 $APP_DIR"
if [[ ! -d "$APP_DIR" ]]; then
    inf "  bundle 不存在, 从源拷贝骨架"
    cp -R "$WEZSRC/assets/macos/WezTerm-SSH.app" "$APP_DIR"
fi
mkdir -p "$APP_MACOS"

# 二进制改名规则 (与 ci/deploy.sh 一致)
declare -a RENAME=(
    "wezterm:WezTerm-SSH-cli"
    "wezterm-gui:WezTerm-SSH"
    "wezterm-mux-server:WezTerm-SSH-mux"
)
for pair in "${RENAME[@]}"; do
    src="${pair%%:*}"; dst="${pair##*:}"
    cp "$REL/$src" "$APP_MACOS/$dst"
    chmod +x "$APP_MACOS/$dst"
    ok "  $src → $APP_MACOS/$dst"
done

# === 4. ad-hoc codesign ===
inf "==> ad-hoc codesign (绕过 Gatekeeper)"
for dst in WezTerm-SSH WezTerm-SSH-cli WezTerm-SSH-mux; do
    codesign --force --sign - "$APP_MACOS/$dst" 2>&1 | grep -v 'replacing existing signature' || true
done
ok "签名完成"

# === 5. PATH 符号链接 ===
inf "==> ~/.local/bin wrapper"
mkdir -p "$LOCAL_BIN"

# 清理上游 wezterm 与上次改名实验残留
for old in wezterm wezterm-gui wezterm-mux-server wezterm-gui.real-bin sshops-cli sshops-mux; do
    if [[ -e "$LOCAL_BIN/$old" || -L "$LOCAL_BIN/$old" ]]; then
        rm -f "$LOCAL_BIN/$old"
        warn "  清理旧命名: $LOCAL_BIN/$old"
    fi
done

# CLI / mux server 用 wrapper 脚本 exec 到 .app 内 binary, 让进程身处 bundle 内,
# macOS 才能正确关联 .app icon (直接 cp binary 到 ~/.local/bin 进程脱离 bundle).
# GUI binary 不暴露 PATH (用户走 open -a WezTerm-SSH 或双击 .app 启动).
for cmd in WezTerm-SSH-cli WezTerm-SSH-mux; do
    cat > "$LOCAL_BIN/$cmd" <<EOF
#!/bin/bash
exec "$APP_MACOS/$cmd" "\$@"
EOF
    chmod +x "$LOCAL_BIN/$cmd"
    ok "  $LOCAL_BIN/$cmd → wrapper(exec $APP_MACOS/$cmd)"
done

# === 6. LaunchServices 刷新 ===
inf "==> 刷新 LaunchServices"
LSREG=/System/Library/Frameworks/CoreServices.framework/Versions/A/Frameworks/LaunchServices.framework/Versions/A/Support/lsregister
if [[ -x "$LSREG" ]]; then
    "$LSREG" -f "$APP_DIR" >/dev/null 2>&1 || true
    ok "lsregister -f 完成"
else
    warn "lsregister 不存在, Dock 图标/名称可能要重启 Finder 才生效"
fi

# === 7. 自检 ===
inf "==> 自检"
echo
"$APP_MACOS/WezTerm-SSH-cli" --version || warn "WezTerm-SSH-cli --version 失败"
"$APP_MACOS/WezTerm-SSH-mux" --version || warn "WezTerm-SSH-mux --version 失败"
echo

ok "部署完成. 下一步:"
echo "    open -a WezTerm-SSH                   # 启动 GUI"
echo "    WezTerm-SSH-cli cli list              # 列 panes"
echo "    bash $HOME/Code/ssh-ops/install.sh    # 安装 ssh-ops skill"
