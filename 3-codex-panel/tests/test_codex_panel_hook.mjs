import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

import {
  directoryLabel,
  handleSessionStart,
  handleStopLikeEvent,
  handleUserPromptSubmit,
  loadState,
  sanitizeLabel,
  saveState,
  stateFile,
  upsertDirectoryLabel,
} from "../scripts/codex_panel_hook.mjs";

test("sanitizeLabel keeps ascii and truncates", () => {
  const label = sanitizeLabel(`hello world ${"x".repeat(40)}`);
  assert.equal(label.startsWith("hello world"), true);
  assert.equal(label.endsWith("..."), true);
});

test("loadState returns empty object on broken json", () => {
  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), "codex-hook-"));
  const brokenPath = path.join(tempDir, "broken.json");
  fs.writeFileSync(brokenPath, "{", "utf8");
  assert.deepEqual(loadState(brokenPath), {});
});

test("saveState roundtrip works", () => {
  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), "codex-hook-"));
  const statePath = path.join(tempDir, "state.json");
  const payload = { session_id: "s1", active: true };
  saveState(statePath, payload);
  assert.deepEqual(JSON.parse(fs.readFileSync(statePath, "utf8")), payload);
});

test("upsertDirectoryLabel stores last two path segments", () => {
  const state = {};
  upsertDirectoryLabel(state, "/Users/langhuam/workspace/self/embedded-systems-lab/3-codex-panel");
  assert.equal(state.directory_label, "embedded-systems-lab/3-co...");
});

test("directoryLabel handles repo root style paths", () => {
  assert.equal(
    directoryLabel("/Users/langhuam/workspace/self/embedded-systems-lab"),
    "self/embedded-systems-lab",
  );
});

test("hook handlers persist parent pid when provided", () => {
  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), "codex-hook-"));
  process.env.STATE_DIR = tempDir;
  process.env.CODEX_PANEL_PARENT_PID = "12345";

  const payload = {
    session_id: "session-1",
    cwd: "/Users/langhuam/workspace/self/embedded-systems-lab/3-codex-panel",
    prompt: "test",
    hook_event_name: "UserPromptSubmit",
  };

  handleSessionStart(payload);
  handleUserPromptSubmit(payload);
  handleStopLikeEvent(payload);

  const state = JSON.parse(fs.readFileSync(stateFile("session-1"), "utf8"));
  assert.equal(state.parent_pid, 12345);
  assert.equal(state.active, false);

  delete process.env.STATE_DIR;
  delete process.env.CODEX_PANEL_PARENT_PID;
});
