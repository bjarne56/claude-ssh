---
name: sshops
description: SSH fjernhandlinger. Auto-løser værter fra SecureCRT; én IP/nøgleord/sti forbinder. Live-visning + optagelse + kommandoindeks.
---

# sshops Skill — Decision Manual

> Phase 1b. Phase 2/3/4 are planned in `docs/Implementation_Plan.md`.

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
sshops run 10.32.49.7 "df -h"
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

- **Read `recent_human_activity`** — the user may have typed commands in the pane between `sshops run` calls; always factor that in to avoid clobbering.
- For multi-step ops, use one `sshops run` per command (each gets recorded separately) instead of joining with `&&`.
- Long-running / TUI commands (`top`, `vim`) — Phase 1b can't do these; ask the user to drive interactively in the pane, then `sshops close` when done.
- Don't fight the safety gate — if blocked, surface the reason to the user and wait for explicit "I confirm".

## 9 Phase 1b limits

- No jumphost (-J) chain (warned but not enforced; coming Phase 2)
- No background command (`bg`); long tasks should use the pane directly
- No fan-out (`fan`); coming Phase 2
- No replay/search/annotate UI; cast-player handles replay

For full feature roadmap see `docs/Implementation_Plan.md`.
