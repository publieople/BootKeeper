---
name: bootkeeper
description: Manage Windows startup items via the bootkeeper CLI. Use when the user asks to list, inspect, analyze, enable, disable, remove, or restore autostart entries (registry Run keys, startup folders, scheduled tasks). Write operations require user confirmation via an elevated dialog.
---

# BootKeeper

Manage Windows autostart (startup) items from the `bootkeeper` CLI.

## When to use

The user wants to:
- see what starts automatically at login (startup items)
- inspect a specific autostart entry (command, signature, publisher, risk)
- analyze which entries look suspicious (software-ruled risk)
- enable / disable / remove a startup item
- restore an item from a snapshot (undo)

## Commands (all output JSON)

### Read

```sh
bootkeeper list [--category registry_run|startup_folder|scheduled_task]
bootkeeper get <id>
bootkeeper analyze [--category ...]
bootkeeper snapshot list
bootkeeper snapshot show <id>
```

Each list item: `id`, `category`, `name`, `command`, `location`, `signature`, `publisher`, `risk`, `enabled`.

### Write (elevated + user confirmation)

```sh
bootkeeper disable <id>          # rename Foo -> Foo.disabled
bootkeeper enable <id>           # rename back
bootkeeper remove <id>           # delete entry (snapshot created first)
bootkeeper restore <snapshot-id> <item-id>   # undo from snapshot
```

**Write behavior (hard confirmation):**
1. The CLI writes a request and launches `bootkeeper-helper.exe` elevated (UAC).
2. The helper re-verifies the item itself (registry lookup + WinVerifyTrust signature + rule engine) and shows a native confirmation dialog with machine-verified facts only.
3. Nothing is executed unless the user clicks **Yes**.
4. Every write creates a snapshot (kept 7 days) for undo.

**You cannot and must not bypass the dialog.** If the user hasn't confirmed, the operation fails with `cancelled by user`. That is by design.

## Snapshots

- Auto-created on every disable/enable/remove. Kept 7 days.
- `snapshot list` shows id/time/operation/entry count.
- `restore` needs the snapshot id and the item id (copy from the snapshot's `entries[].item_id`).

## Platform notes

- Must run **on Windows** for real enumeration; other OSes return `[]`.
- `BOOTKEEPER_DATA` overrides data dir (default `%APPDATA%\BootKeeper`).
- `BOOTKEEPER_HELPER` overrides the helper exe path (usually auto-discovered).

## Rules for the agent

1. **Never fabricate items.** If `list` returns `[]` or the platform isn't Windows, say so.
2. **Risk is authoritative.** `risk` + `reasons` come from the rule engine; you may explain but never change the verdict.
3. **Never claim a write succeeded without a result.** Check the JSON: `ok: true` + message. `ok: false` means cancelled or failed.
4. **Disabling is safer than removing.** Prefer `disable` (reversible via rename) over `remove` unless the user explicitly wants deletion.
5. **Always suggest the confirmation dialog consequence** before a write: "this will pop a UAC + confirmation dialog you must approve".
