#!/usr/bin/env python3

from __future__ import annotations

import argparse
import json
from pathlib import Path

PROJECT_DIR = Path(__file__).resolve().parent.parent
HOOK_EVENTS = ("SessionStart", "UserPromptSubmit", "Stop", "SessionEnd")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="安装 lesson 3 所需的 Codex hooks。")
    parser.add_argument("--project-root", type=Path, required=True, help="第三课目录绝对路径")
    parser.add_argument(
        "--scope",
        choices=("repo", "user"),
        default="repo",
        help="安装到仓库级 .codex/hooks.json 还是用户级 ~/.codex/hooks.json",
    )
    parser.add_argument(
        "--home-dir",
        type=Path,
        default=Path.home(),
        help="用户级 hooks 的 home 目录，测试时可覆盖",
    )
    parser.add_argument(
        "--uninstall",
        action="store_true",
        help="只移除 lesson 3 注入的 hooks，保留其他现有 hooks",
    )
    return parser.parse_args()


def panel_hook_command(project_root: Path) -> str:
    script_path = project_root.resolve() / "scripts" / "codex_panel_hook.mjs"
    return f'/usr/bin/env CODEX_PANEL_PARENT_PID="$PPID" node "{script_path}"'


def panel_hook_command_without_pid(project_root: Path) -> str:
    script_path = project_root.resolve() / "scripts" / "codex_panel_hook.mjs"
    return f'/usr/bin/env node "{script_path}"'


def legacy_repo_hook_command(project_root: Path) -> str:
    lesson_dir = project_root.resolve().name
    return f'/usr/bin/env CODEX_PANEL_PARENT_PID="$PPID" node "$(git rev-parse --show-toplevel)/{lesson_dir}/scripts/codex_panel_hook.mjs"'


def legacy_repo_hook_command_without_pid(project_root: Path) -> str:
    lesson_dir = project_root.resolve().name
    return f'/usr/bin/env node "$(git rev-parse --show-toplevel)/{lesson_dir}/scripts/codex_panel_hook.mjs"'


def managed_hook_commands(project_root: Path) -> tuple[str, ...]:
    return (
        panel_hook_command(project_root),
        panel_hook_command_without_pid(project_root),
        legacy_repo_hook_command(project_root),
        legacy_repo_hook_command_without_pid(project_root),
    )


def hooks_path_for_scope(project_root: Path, scope: str, home_dir: Path) -> Path:
    if scope == "user":
        return home_dir / ".codex" / "hooks.json"

    return project_root.resolve().parent / ".codex" / "hooks.json"


def load_hooks_config(hooks_path: Path) -> dict:
    if not hooks_path.exists():
        return {"hooks": {}}

    try:
        payload = json.loads(hooks_path.read_text(encoding="utf-8"))
    except json.JSONDecodeError:
        return {"hooks": {}}

    if not isinstance(payload, dict):
        return {"hooks": {}}

    hooks = payload.get("hooks")
    if not isinstance(hooks, dict):
        payload["hooks"] = {}

    return payload


def save_hooks_config(hooks_path: Path, payload: dict) -> None:
    hooks_path.parent.mkdir(parents=True, exist_ok=True)
    hooks_path.write_text(
        json.dumps(payload, indent=2, ensure_ascii=False) + "\n",
        encoding="utf-8",
    )


def normalize_event_entries(payload: dict, event_name: str) -> list[dict]:
    hooks = payload.setdefault("hooks", {})
    event_entries = hooks.setdefault(event_name, [])
    if not event_entries:
        event_entries.append({"hooks": []})

    for entry in event_entries:
        if not isinstance(entry, dict):
            continue
        nested_hooks = entry.get("hooks")
        if not isinstance(nested_hooks, list):
            entry["hooks"] = []

    if not isinstance(event_entries[0], dict):
        event_entries[0] = {"hooks": []}

    return event_entries


def install_command_for_event(payload: dict, event_name: str, command: str) -> None:
    event_entries = normalize_event_entries(payload, event_name)

    for entry in event_entries:
        for hook in entry.get("hooks", []):
            if hook.get("type") == "command" and hook.get("command") == command:
                return

    event_entries[0]["hooks"].append({"type": "command", "command": command})


def remove_command_from_event(payload: dict, event_name: str, commands: set[str]) -> None:
    hooks = payload.get("hooks", {})
    event_entries = hooks.get(event_name)
    if not isinstance(event_entries, list):
        return

    next_entries: list[dict] = []
    for entry in event_entries:
        if not isinstance(entry, dict):
            continue

        nested_hooks = entry.get("hooks")
        if not isinstance(nested_hooks, list):
            continue

        kept_hooks = [
            hook
            for hook in nested_hooks
            if not (hook.get("type") == "command" and hook.get("command") in commands)
        ]

        if kept_hooks:
            next_entry = dict(entry)
            next_entry["hooks"] = kept_hooks
            next_entries.append(next_entry)

    if next_entries:
        hooks[event_name] = next_entries
    else:
        hooks.pop(event_name, None)


def install_hooks(project_root: Path, scope: str, home_dir: Path) -> Path:
    hooks_path = hooks_path_for_scope(project_root, scope, home_dir)
    payload = load_hooks_config(hooks_path)
    command = panel_hook_command(project_root)
    commands = set(managed_hook_commands(project_root))

    for event_name in HOOK_EVENTS:
        remove_command_from_event(payload, event_name, commands)
        install_command_for_event(payload, event_name, command)

    save_hooks_config(hooks_path, payload)
    return hooks_path


def uninstall_hooks(project_root: Path, scope: str, home_dir: Path) -> Path:
    hooks_path = hooks_path_for_scope(project_root, scope, home_dir)
    payload = load_hooks_config(hooks_path)
    commands = set(managed_hook_commands(project_root))

    for event_name in HOOK_EVENTS:
        remove_command_from_event(payload, event_name, commands)

    save_hooks_config(hooks_path, payload)
    return hooks_path


def main() -> int:
    args = parse_args()
    project_root = args.project_root.resolve()

    if args.uninstall:
        hooks_path = uninstall_hooks(project_root, args.scope, args.home_dir)
        print(f"已移除 lesson 3 hooks: {hooks_path}")
        return 0

    hooks_path = install_hooks(project_root, args.scope, args.home_dir)
    print(f"已写入 lesson 3 hooks: {hooks_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
