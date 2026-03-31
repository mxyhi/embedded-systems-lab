from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

import scripts.install_hooks as install_hooks


class InstallHooksTests(unittest.TestCase):
    def test_install_user_hooks_merges_existing_commands(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            home_dir = Path(temp_dir)
            hooks_path = home_dir / ".codex" / "hooks.json"
            hooks_path.parent.mkdir(parents=True, exist_ok=True)
            hooks_path.write_text(
                json.dumps(
                    {
                        "hooks": {
                            "SessionStart": [
                                {
                                    "hooks": [
                                        {
                                            "type": "command",
                                            "command": "/Users/langhuam/.superset/hooks/notify.sh",
                                        }
                                    ]
                                }
                            ]
                        }
                    }
                ),
                encoding="utf-8",
            )

            install_hooks.install_hooks(
                project_root=install_hooks.PROJECT_DIR,
                scope="user",
                home_dir=home_dir,
            )

            payload = json.loads(hooks_path.read_text(encoding="utf-8"))
            session_start = payload["hooks"]["SessionStart"][0]["hooks"]
            commands = [item["command"] for item in session_start]

            self.assertIn("/Users/langhuam/.superset/hooks/notify.sh", commands)
            self.assertIn(install_hooks.panel_hook_command(install_hooks.PROJECT_DIR), commands)
            self.assertIn("UserPromptSubmit", payload["hooks"])
            self.assertIn("Stop", payload["hooks"])
            self.assertIn("SessionEnd", payload["hooks"])

    def test_uninstall_user_hooks_keeps_other_commands(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            home_dir = Path(temp_dir)
            hooks_path = home_dir / ".codex" / "hooks.json"
            hooks_path.parent.mkdir(parents=True, exist_ok=True)
            panel_command = install_hooks.panel_hook_command(install_hooks.PROJECT_DIR)
            hooks_path.write_text(
                json.dumps(
                    {
                        "hooks": {
                            "SessionStart": [
                                {
                                    "hooks": [
                                        {
                                            "type": "command",
                                            "command": "/Users/langhuam/.superset/hooks/notify.sh",
                                        },
                                        {
                                            "type": "command",
                                            "command": panel_command,
                                        },
                                    ]
                                }
                            ],
                            "UserPromptSubmit": [
                                {
                                    "hooks": [
                                        {
                                            "type": "command",
                                            "command": panel_command,
                                        }
                                    ]
                                }
                            ],
                        }
                    }
                ),
                encoding="utf-8",
            )

            install_hooks.uninstall_hooks(
                project_root=install_hooks.PROJECT_DIR,
                scope="user",
                home_dir=home_dir,
            )

            payload = json.loads(hooks_path.read_text(encoding="utf-8"))
            session_start = payload["hooks"]["SessionStart"][0]["hooks"]
            commands = [item["command"] for item in session_start]

            self.assertEqual(commands, ["/Users/langhuam/.superset/hooks/notify.sh"])
            self.assertNotIn("UserPromptSubmit", payload["hooks"])

    def test_uninstall_repo_hooks_removes_legacy_repo_local_command(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            project_root = Path(temp_dir) / "3-codex-panel"
            repo_root = project_root.parent
            hooks_path = repo_root / ".codex" / "hooks.json"
            hooks_path.parent.mkdir(parents=True, exist_ok=True)
            project_root.mkdir(parents=True, exist_ok=True)
            hooks_path.write_text(
                json.dumps(
                    {
                        "hooks": {
                            "SessionStart": [
                                {
                                    "hooks": [
                                        {
                                            "type": "command",
                                            "command": "/usr/bin/env node \"$(git rev-parse --show-toplevel)/3-codex-panel/scripts/codex_panel_hook.mjs\"",
                                        }
                                    ]
                                }
                            ]
                        }
                    }
                ),
                encoding="utf-8",
            )

            install_hooks.uninstall_hooks(
                project_root=project_root,
                scope="repo",
                home_dir=repo_root,
            )

            payload = json.loads(hooks_path.read_text(encoding="utf-8"))
            self.assertEqual(payload["hooks"], {})


if __name__ == "__main__":
    unittest.main()
