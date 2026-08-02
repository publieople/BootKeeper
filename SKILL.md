---
name: bootkeeper
description: Manage Windows startup items via the bootkeeper CLI. Use when the user asks to list, inspect, analyze, or manage autostart entries (registry Run keys, startup folders, scheduled tasks). Read-only in v1 — write operations arrive later.
---

# BootKeeper

Manage Windows autostart (startup) items from the `bootkeeper` CLI.

## When to use

The user wants to:
- see what starts automatically at login (startup items)
- inspect a specific autostart entry (command, signature, publisher)
- analyze which entries look suspicious
- view change history (snapshots)

## Commands (all output JSON)

### List startup items

```sh
bootkeeper list
bootkeeper list --category registry_run   # or startup_folder, scheduled_task
```

Each item: `id`, `category`, `name`, `command`, `location`, `signature`, `publisher`, `risk`, `enabled`.

### Inspect one item

```sh
bootkeeper get <id>
```

The `id` is `category:location:name` — copy it from `list` output.

### Analyze risk (rules decide, AI may explain)

```sh
bootkeeper analyze
bootkeeper analyze --category registry_run
```

Output per item: `id`, `name`, `command`, `signature`, `risk` (low/medium/high), `reasons`. Risk is decided by the software rule engine — unsigned + non-system path + non-Microsoft publisher is high. You may explain *why* but never change the `risk` value yourself.

### Snapshots (change history, retained 7 days)

```sh
bootkeeper snapshot list
bootkeeper snapshot show <id>
```

## Platform notes

- `bootkeeper` must run **on Windows** for real enumeration. On other OSes it returns `[]`.
- The CLI runs with the user's permissions: HKLM / system-level items are visible but writes (M2) will require elevation + user confirmation.
- `BOOTKEEPER_DATA` env var overrides where snapshots are stored (default `%APPDATA%\BootKeeper\snapshots`).

## Rules for the agent

1. **Never fabricate items.** If `bootkeeper list` returns `[]` or the platform is not Windows, say so — do not invent startup entries.
2. **Risk is authoritative.** Treat `risk` + `reasons` as the software's verdict. You can add analysis, but the verdict stands.
3. **Write operations are NOT available in v1.** If the user asks to disable/remove an item, explain that writes arrive in a later version (or check for a newer CLI).
4. Always prefer JSON fields over prose when quoting an item back to the user.
