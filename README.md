# claude-ssh

A Claude Code skill for SSH remote operations via **WezTerm-SSH** (a WezTerm fork).

- **One WezTerm-SSH window per Claude Code project**, **one pane per remote host**
- Each pane holds a real PTY — character-level live view that the user can watch
- Commands are injected via `WezTerm-SSH-cli send-text` and the output is sliced back to Claude using marker delimiters
- **Full asciinema recording** of every session, with a separate command-level index
- Does not write `~/.ssh/config`, does not cache host info — all UI lives inside WezTerm-SSH

> Status: **Phase 1a** (MVP main loop works in temporary-args mode). Phase 1b adds SecureCRT integration. Full roadmap in [`docs/Implementation_Plan.md`](docs/Implementation_Plan.md).

> **WezTerm vs WezTerm-SSH**: `wezterm-src/` is a fork of WezTerm. After build & deploy it produces three renamed binaries inside an isolated `.app` bundle, **fully namespace-isolated** from upstream WezTerm (separate socket / data dir / window class / bundle id), so it can coexist with the official WezTerm:
>
> | Component | WezTerm-SSH (this fork) | Upstream WezTerm |
> |---|---|---|
> | macOS bundle | `~/Applications/WezTerm-SSH.app` | `/Applications/WezTerm.app` |
> | Bundle id | `com.wezterm-ssh.gui` | `com.github.wez.wezterm` |
> | GUI binary | `WezTerm-SSH` | `wezterm-gui` |
> | CLI subcommand | `WezTerm-SSH-cli` | `wezterm` |
> | mux server | `WezTerm-SSH-mux` | `wezterm-mux-server` |
> | runtime dir | `~/.local/share/WezTerm-SSH/` | `~/.local/share/wezterm/` |
>
> The ssh-ops business CLI (`bin/sshops`) does not collide with the GUI bundle — the former is a shell/Rust binary on `PATH`, the latter lives in `~/Applications/WezTerm-SSH.app`.

## Install

### 1. System dependencies

```bash
# macOS — note: do NOT install the upstream wezterm cask, this project uses the wezterm-src/ fork
brew install asciinema jq
brew install hudochenkov/sshpass/sshpass   # optional, only required if you use --password
brew install pass                          # optional, password backend

# Rust toolchain (needed to build the wezterm-src fork locally)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Enable "Remote Login" so self-test can ssh localhost
# System Settings → General → Sharing → Remote Login
# Add your public key to ~/.ssh/authorized_keys
```

> bash: **3.2 / 4 / 5 all supported**. The macOS default `/bin/bash` 3.2 is fine; brew bash 5.x works too. Empty-array + `set -u` corner cases are explicitly guarded. Tested on 3.2.57 and 5.3.9.

### 2. One-line install (builds & deploys the wezterm-src fork)

```bash
git clone https://github.com/bjarne56/claude-ssh ~/Code/ssh-ops
cd ~/Code/ssh-ops
bash install.sh
```

`install.sh` is **idempotent** — running it repeatedly only skips already-deployed pieces. It will:

1. Verify system dependencies (asciinema/jq/ssh/cargo/...) and `brew install` missing ones automatically (macOS)
2. **(macOS)** Run `wezterm-src/install-local.sh` to build & deploy the WezTerm-SSH bundle:
   - `cargo build --release` for the three core crates (wezterm/wezterm-gui/wezterm-mux-server, ~2 min on first build)
   - Deploy to `~/Applications/WezTerm-SSH.app/Contents/MacOS/{WezTerm-SSH, WezTerm-SSH-cli, WezTerm-SSH-mux}`
   - ad-hoc codesign + `lsregister -f` to refresh LaunchServices
   - Install wrapper scripts under `~/.local/bin/{WezTerm-SSH-cli, WezTerm-SSH-mux}` (they `exec` into the .app so macOS can correctly associate the icon)
3. Deploy the skill to `~/.claude/skills/sshops/` (slash command is `/sshops`)
   - `SKILL.md` description is auto-translated based on system locale (31 languages, see `skill-locales/descriptions.json`)
   - Unknown locales fall back to English
   - Business files (`bin/`, `rust/`, ...) are symlinked to the source repo
4. Append PATH entries to `~/.zshenv` (`~/Code/ssh-ops/bin` and `~/.local/bin`, both idempotent)

After install:

```bash
source ~/.zshenv          # apply PATH immediately (or just open a new shell)
sshops setup              # interactive wizard to write config.json
open -a WezTerm-SSH       # launch the GUI (double-clicking the .app does the same)
```

**Invoke from Claude Code:**

```
/sshops <your request>          # e.g. /sshops connect to 10.0.0.5 and run uname
```

The slash command name `sshops` matches the skill directory name and the `name` field in `SKILL.md` frontmatter. The description shown in the skill list is picked automatically from your system locale (macOS uses `defaults read -g AppleLocale`, Linux uses `$LANG`).

**Install options:**

```bash
bash install.sh --no-build-wezterm        # skip wezterm build (already deployed, or running on Linux)
bash install.sh --link-only               # only relink the skill (skip dep check + wezterm build)
bash install.sh --locale en               # force English description (default: system locale)
bash install.sh --link-only --locale ja   # switch description to Japanese, no rebuild
bash install.sh --status                  # show current deployment state
bash install.sh --uninstall               # remove all installed parts (asks for confirmation)
```

**Multi-language description maintenance:**

All 31 description translations live in a single file: `skill-locales/descriptions.json` (English + Simplified/Traditional Chinese + Japanese, Korean, French, German, Spanish, Italian, Portuguese, Russian, Ukrainian, etc., 32 KV entries total). Adding a new language requires only one new line in the JSON. The body of `SKILL.md` is the English master — to update wording, edit it in one place.

### 3. Verify

```bash
bash tests/self-test.sh
```

Passing means marker slicing + recording + state writes are all working end-to-end.

## Usage

### Claude calls automatically

Describe the task in your Claude Code chat. Based on [`SKILL.md`](SKILL.md), Claude decides when to invoke this skill. For example:

> Connect to 10.1.2.3, run uptime to check the load, and record it.

Claude will run:

```bash
sshops run --host 10.1.2.3 --user ec2-user --key ~/.ssh/aws.pem "uptime"
```

### Direct CLI usage

```bash
# Run a short command
sshops run --host 10.1.2.3 --user root --key ~/.ssh/k.pem "uptime"

# Just spawn a pane, no command (you'll type interactively)
sshops open --host 10.1.2.3 --user root --key ~/.ssh/k.pem

# Close the pane
sshops close --host 10.1.2.3 --user root --port 22

# List all panes for the current project
sshops list-panes
```

JSON output:

```json
{
  "exit": 0,
  "duration_ms": 340,
  "cast_offset": 12.4,
  "session_id": "10.1.2.3-20260502-142301-a3f4b1",
  "dangerous": false,
  "blocked": false,
  "output": "14:23:01 up 47 days, load 0.5"
}
```

When blocked:

```json
{
  "exit": -1,
  "blocked": true,
  "dangerous": true,
  "reason": "...(pattern: rm\\s+-rf\\s+/)",
  "session_id": "...",
  "output": "(not executed)"
}
```

## Safety guards

Built-in dangerous-command interceptor (`rm -rf /`, `reboot`, `mkfs`, `dd of=/dev/`, `shutdown`, `:(){`, `chmod -R 777 /`, etc., customizable in `config.json`).

| Scenario | Behavior |
|---|---|
| Dangerous + `--prod` flag + no `--i-mean-it` | **Refused**, exit 5 |
| Dangerous + `--prod` + `--i-mean-it` | Warns but allows |
| Dangerous + non-prod | Warns but allows |

`--i-mean-it` is **never** added by Claude on its own — the user must explicitly confirm in chat.

## Project layout

```
claude-ssh/
├── SKILL.md               Claude decision manual (English master)
├── README.md              this file
├── LICENSE                MIT
├── bin/
│   ├── sshops             business CLI dispatcher (bash, forwards subcommands to Rust)
│   └── sshops-setup       interactive setup wizard
├── rust/                  business implementation (Rust + daemon, primary path)
│   ├── core/              shared logic (config / state / pane / wezterm_mux / safety / recorder ...)
│   ├── bin/               sshops-rs binary (short-lived CLI)
│   └── daemon/            sshops-daemon (persistent IPC)
├── wezterm-src/           WezTerm fork → WezTerm-SSH (subdirectory, merged into this repo)
│   └── install-local.sh   local build + deploy ~/Applications/WezTerm-SSH.app
├── cast-player/           Tauri-based recording replay GUI (32-locale i18n)
├── lua/                   keyword-highlight rules for WezTerm-SSH
├── skill-locales/
│   └── descriptions.json  31-language SKILL frontmatter descriptions
├── tools/                 SecureCRT .ini → wezterm rules converter
├── config.example.json    config template
├── install.sh             dependency check + wezterm-src build + skill deploy (idempotent)
├── tests/self-test.sh     localhost echo smoke test
└── docs/
    ├── PROJECT_OVERVIEW.md   architecture overview
    └── Implementation_Plan.md task-level status
```

Recording data (produced at runtime, not committed) defaults to **`<project>/.ssh-ops/recordings/<session_id>/`** in the current project root. The skill auto-appends `.ssh-ops/` to the project's `.gitignore` on first write.

If you prefer **centralized storage** (all projects record to one location, useful for unified auditing), set `"log_dir": "~/.ssh-recordings"` in `config.json`. The directory structure becomes `<log_dir>/<project_slug>/<session_id>/`.

## Default audit scheme (no remote-host changes required)

The skill **does not require creating new accounts on the target host**. The default scheme uses your existing SecureCRT config + PS1 string audit + asciinema recording:

| Signal | Value |
|---|---|
| ssh login user | from SecureCRT .ini (e.g. `roy`) |
| auto_sudo to root | enabled by default; the target host needs NOPASSWD configured, or you type the sudo password manually in the pane |
| Remote PS1 prompt | `[root(roy:claude)@host ~]#` or `[roy(roy:claude)@host ~]$` |
| Recording | `<project>/.ssh-ops/recordings/<session-id>/` (stream.cast + commands.jsonl + meta.json) |

The `(roy:claude)` prompt segment means "**original ssh login is `roy`, current operator is `claude` (AI)**" — anyone watching the asciinema replay or sitting next to the pane can immediately tell AI from human. In production environments where you usually can't create new accounts on target hosts, this scheme (existing account + PS1 string audit + full recording) is the standard approach.

---

## Roadmap

- **Phase 1a** ✅ Temporary args + marker slicing + recording + safety guards
- **Phase 1b** SecureCRT integration (`@path` / keyword / jumphost / SSH2.ini global fallback)
- **Phase 2** `bg` `fan` `health` `sync` `grid` `push/pull` `forward`
- **Phase 3** Replay / search / annotate / retention gc
- **Phase 4** WezTerm Lua visuals (AI/HUMAN/REPLAY border colors + replay key bindings)

## FAQ

**Q: Claude calls sshops and gets "WezTerm-SSH-cli not reachable".**
A: Is the WezTerm-SSH GUI running? The skill auto-runs `open -a WezTerm-SSH` and retries for 5 seconds; if that fails, the fork isn't deployed — run `bash wezterm-src/install-local.sh` to redeploy (idempotent).

**Q: Command-injection timeout (`exit 4`).**
A: The command is a TUI (`vim`, `top`) or a long-running task. Phase 2 will support `sshops bg`; for now, `sshops close` and run it manually in your own terminal.

**Q: `exit 3` shell not supported.**
A: The login shell on the target host must be bash or zsh. fish / tcsh / network-device CLIs are not supported.

**Q: `exit 5` blocked.**
A: A dangerous command was blocked on a production host. To run it anyway, explicitly tell Claude in chat "I confirm running X on prod" — Claude will then add `--i-mean-it`.

**Q: bash version requirements?**
A: **3.2 / 4 / 5 all supported**. macOS default `/bin/bash 3.2` works; brew `bash 5.x` also works. The code uses `[[ ${#arr[@]} -gt 0 ]]` to explicitly guard the empty-array + `set -u` corner case (a known bug before bash 4.4). Tested on 3.2.57 (macOS Sonoma default) and 5.3.9 (Homebrew).

**Q: Build failure: `library 'git2' not found`.**
A: Fixed in v1.0.1 — the `git2` crate now uses `vendored-libgit2`, so wezterm-src no longer depends on the system Homebrew libgit2. If you're on v1.0.0 and hit this after `brew upgrade libgit2`, run:
```bash
cd wezterm-src
rm -rf target/release/build/libgit2-sys-* target/release/build/git2-*
git pull && bash install-local.sh
```

## Documentation

- [`SKILL.md`](SKILL.md) — Decision manual for Claude
- [`docs/PROJECT_OVERVIEW.md`](docs/PROJECT_OVERVIEW.md) — Architecture overview
- [`docs/Implementation_Plan.md`](docs/Implementation_Plan.md) — Task-level status
- [`CLAUDE.md`](CLAUDE.md) — Project coding constraints

## License

[MIT](LICENSE). The bundled WezTerm fork (`wezterm-src/`) inherits its own MIT license — see `wezterm-src/LICENSE.md`.
