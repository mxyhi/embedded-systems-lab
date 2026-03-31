#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const PROJECT_DIR = path.resolve(__dirname, "..");
const STATE_DIR = process.env.STATE_DIR || path.join(PROJECT_DIR, "runtime", "codex-state");
const TITLE_MAX_CHARS = 28;

fs.mkdirSync(STATE_DIR, { recursive: true });

export function sanitizeLabel(text) {
  const collapsed = String(text).trim().split(/\s+/).join(" ");
  const asciiOnly = Array.from(collapsed, (char) => {
    if (char >= " " && char <= "~") {
      return char;
    }
    return "?";
  }).join("");

  if (asciiOnly.length <= TITLE_MAX_CHARS) {
    return asciiOnly;
  }

  return `${asciiOnly.slice(0, TITLE_MAX_CHARS - 3)}...`;
}

export function directoryLabel(cwd) {
  const resolved = path.resolve(String(cwd || "/"));
  const parts = resolved.split(path.sep).filter(Boolean);

  if (parts.length === 0) {
    return "/";
  }

  if (parts.length === 1) {
    return sanitizeLabel(parts[0]);
  }

  return sanitizeLabel(parts.slice(-2).join("/"));
}

export function stateFile(sessionId) {
  return path.join(STATE_DIR, `${sessionId}.json`);
}

export function loadState(filePath) {
  try {
    return JSON.parse(fs.readFileSync(filePath, "utf8"));
  } catch {
    return {};
  }
}

export function saveState(filePath, state) {
  const tempPath = `${filePath}.tmp`;
  fs.writeFileSync(tempPath, JSON.stringify(state), "utf8");
  fs.renameSync(tempPath, filePath);
}

export function upsertDirectoryLabel(state, cwd) {
  state.directory_label = directoryLabel(cwd);
}

export function upsertParentPid(state) {
  const rawPid = process.env.CODEX_PANEL_PARENT_PID || "";
  const pid = Number.parseInt(String(rawPid), 10);
  if (Number.isInteger(pid) && pid > 0) {
    state.parent_pid = pid;
  }
}

export function handleSessionStart(payload) {
  const sessionId = String(payload.session_id);
  const filePath = stateFile(sessionId);
  const state = loadState(filePath);
  const now = Date.now() / 1000;
  const cwd = String(payload.cwd || "");

  state.session_id = sessionId;
  state.cwd = cwd;
  state.hook_event = String(payload.hook_event_name || "");
  state.active = Boolean(state.active || false);
  state.created_at = state.created_at || now;
  state.updated_at = now;
  upsertDirectoryLabel(state, cwd);
  upsertParentPid(state);
  saveState(filePath, state);
}

export function handleUserPromptSubmit(payload) {
  const sessionId = String(payload.session_id);
  const filePath = stateFile(sessionId);
  const state = loadState(filePath);
  const now = Date.now() / 1000;
  const cwd = String(payload.cwd || "");

  state.session_id = sessionId;
  state.cwd = cwd;
  state.active = true;
  state.last_prompt = String(payload.prompt || "");
  state.turn_started_at = now;
  state.updated_at = now;
  upsertDirectoryLabel(state, cwd);
  upsertParentPid(state);
  saveState(filePath, state);
}

export function handleStopLikeEvent(payload) {
  const sessionId = String(payload.session_id);
  const filePath = stateFile(sessionId);
  const state = loadState(filePath);
  const now = Date.now() / 1000;
  const cwd = String(payload.cwd || "");

  state.session_id = sessionId;
  state.cwd = cwd;
  state.active = false;
  state.updated_at = now;
  state.last_assistant_message = String(payload.last_assistant_message || "");
  upsertDirectoryLabel(state, cwd);
  upsertParentPid(state);
  saveState(filePath, state);
  process.stdout.write(`${JSON.stringify({ continue: true })}\n`);
}

export function readPayload(stdinText) {
  return JSON.parse(stdinText);
}

export async function main() {
  const input = fs.readFileSync(0, "utf8");
  const payload = readPayload(input);
  const eventName = String(payload.hook_event_name || "");

  if (eventName === "SessionStart") {
    handleSessionStart(payload);
    return;
  }

  if (eventName === "UserPromptSubmit") {
    handleUserPromptSubmit(payload);
    return;
  }

  if (eventName === "Stop") {
    handleStopLikeEvent(payload);
    return;
  }

  if (eventName === "SessionEnd") {
    handleStopLikeEvent(payload);
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === __filename) {
  await main();
}
