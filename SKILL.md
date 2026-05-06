---
name: sshops
description: SSH remote ops. Auto-resolves hosts from SecureCRT; one IP/keyword/path connects. Live view + recording + command index.
---

# sshops Skill — Decision Manual

> Phase 1b. Phase 2/3/4 are planned in `docs/Implementation_Plan.md`.

## 0 Cardinal rule (READ FIRST)

**Every remote command MUST run through `sshops run` (or `sshops open` for interactive) directly. Never detach, redirect, or hide output.**

The whole point of this skill is **live PTY view + asciinema recording + audit trail**. Bypassing the pane defeats all three.

### ❌ NEVER do these on a remote host through sshops

```bash
# Backgrounding — pane shows nothing, cast records nothing
sshops run X "long-cmd &"
sshops run X "nohup long-cmd > /tmp/log 2>&1 &"
sshops run X "setsid long-cmd < /dev/null > /tmp/log 2>&1 &"
sshops run X "long-cmd & disown"

# Redirect to file then poll — same as above
sshops run X "cargo build > /tmp/build.log 2>&1 &"
sshops run X "tail /tmp/build.log"   # later, polling

# Silent mode — no audit visible
sshops run X "cmd > /dev/null 2>&1"

# Spawn a separate non-sshops ssh — invisible to the user, no recording
Bash(ssh user@host 'long-cmd')
```

### ✅ DO this instead

```bash
# Long-running task: bump --timeout, run in foreground in the pane
sshops run --timeout 600 X "cd /src && cargo build --release -p mycrate"

# Even longer (up to 1 hour)
sshops run --timeout 3600 X "make test"

# Need real-time progress while doing other work? Open the pane and let user watch
sshops open X
# (then send sub-commands with sshops run, all visible in the same pane)
```

### Why

| Sin | Consequence |
|---|---|
| `&` / `nohup` / `setsid` / `disown` | Command detaches from PTY → pane shows nothing, cast file empty, audit broken |
| `> /tmp/log` redirect | PTY sees nothing → pane silent, cast empty |
| `> /dev/null 2>&1` | Output suppressed everywhere; user can't observe, replay can't recover |
| Bash(ssh ...) parallel session | Operates outside the recorded pane; equivalent to running on a separate machine for audit purposes |

**If you think you need to detach because the timeout is too short**: bump `--timeout` (default 30 s, max 3600 s = 1 h). Long-running tasks (`cargo build`, `make`, `apt upgrade`) regularly take 5-30 minutes and `--timeout 1800` handles them fine. There is **no** legitimate reason in Phase 1b to background a remote command from within `sshops run`.

## 1 When to invoke

Invoke this skill (instead of plain `Bash(ssh ...)`) for any of:

- User wants to run commands on a remote host **and** see the live view, **or** needs replay / audit later
- User says "ssh to X and run Y" / "connect to X" / "check status of X"
- User gives a host identifier (IP / hostname / SecureCRT path keyword), even without saying "ssh"
- Remote ops, config changes, long-running task monitoring
- Multi-host parallel session management within a single project

## 2 Three host-resolution entry points (KEY!)

The user's host identifier comes in three forms — the skill auto-picks the right path, no extra questions needed:

### 2.1 `@<relative-path>` — SecureCRT exact match

User input starts with `@`, e.g. `@aws/edge` → `<SecureCRT.Sessions>/aws/edge.ini`.

```bash
sshops run @aws/edge "uptime"
sshops open @prod/db-master
```

### 2.2 `<keyword>` — SecureCRT fuzzy match

Plain identifier (IP / partial hostname / file fragment) — fuzzy matches both filename (without `.ini`) and `Hostname=` field. **Single match required**, multi-match errors out with the candidate list.

```bash
sshops run cvm-01 "uptime"
sshops run 10.0.0.5 "df -h"
```

### 2.3 `--host H --user U` — Temporary args

When the user explicitly gives full connection params, bypassing SecureCRT:

```bash
sshops run --host 10.1.2.3 --user root --key ~/.ssh/k.pem "uptime"
```

## 3 Command decision tree

| Intent | Command | Notes |
|---|---|---|
| Run a single command, get output | `sshops run <selector> "<cmd>"` | JSON: `{exit, output, duration_ms, cast_offset, ...}` |
| Open pane only (later interaction) | `sshops open <selector>` | Returns `pane_id` + `session_id` |
| Close pane | `sshops close <selector>` | Cleans state + finalizes recording |
| Read pane current screen | `sshops peek <selector>` | strip ANSI |
| List project's panes | `sshops list-panes` | JSON |

## 4 Dangerous-command handling

Built-in dangerous-pattern interceptor (`rm -rf /`, `reboot`, `mkfs`, `dd of=/dev/`, `shutdown`, `:(){`, `chmod -R 777 /`, etc., customizable in `config.json`).

| Scenario | Behavior |
|---|---|
| Dangerous + `--prod` flag + no `--i-mean-it` | **Reject**, exit 5, JSON `{blocked: true, dangerous: true, reason: ...}` |
| Dangerous + `--prod` + `--i-mean-it` | Warn, allow |
| Dangerous + non-prod | Warn, allow |

**`--i-mean-it` MUST NOT be added by Claude on its own** — only when the user explicitly says "I confirm running X on prod" in conversation.

## 5 SecureCRT password-login hosts

If the `.ini` has only `Password V2` (no key Identity), the user must supply password:

- `--ask-password` — skill prompts in user's terminal once
- `--password STR` — explicit (less secure, avoid in chat)
- Or fix the `.ini` in SecureCRT to use a key

## 6 `.ppk` hosts

PuTTY-format keys (`*.ppk`) are detected and rejected with a clear error. User must convert with `puttygen` first.

## 7 Output JSON contract (for `sshops run`)

Success:
```json
{
  "exit": 0,
  "duration_ms": 340,
  "cast_offset": 12.4,
  "session_id": "10.1.2.3-20260502-142301-a3f4b1",
  "selector": "@aws/edge",
  "dangerous": false,
  "blocked": false,
  "output": "...",
  "recent_human_activity": []
}
```

Blocked (exit 5):
```json
{
  "exit": -1,
  "blocked": true,
  "dangerous": true,
  "reason": "...(pattern: rm\\s+-rf\\s+/)",
  "selector": "...",
  "session_id": "...",
  "output": "(not executed)"
}
```

## 8 Tips for Claude

- **Cardinal rule (§0) is non-negotiable** — every remote command goes through `sshops run` in the foreground, no `&` / `nohup` / `setsid` / `> /tmp/log` redirects / `Bash(ssh …)` shortcuts. Bumping `--timeout` is always preferable to detaching.
- **Read `recent_human_activity`** — the user may have typed commands in the pane between `sshops run` calls; always factor that in to avoid clobbering.
- For multi-step ops, use one `sshops run` per command (each gets recorded separately) instead of joining with `&&`.
- Long-running / TUI commands (`top`, `vim`) — Phase 1b can't do these; ask the user to drive interactively in the pane, then `sshops close` when done.
- Long-running but non-TUI tasks (`cargo build`, `make`, `apt upgrade`, big `find`) — use `--timeout 600` to `--timeout 3600` and let it run in the foreground. The pane streams progress live and the cast captures everything.
- Don't fight the safety gate — if blocked, surface the reason to the user and wait for explicit "I confirm".

## 9 Phase 1b limits

- No jumphost (-J) chain (warned but not enforced; coming Phase 2)
- No background command (`bg`); long tasks should use the pane directly
- No fan-out (`fan`); coming Phase 2
- No replay/search/annotate UI; cast-player handles replay

For full feature roadmap see `docs/Implementation_Plan.md`.
