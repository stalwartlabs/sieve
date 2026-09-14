import { Engine } from "./engine.js";
import { EML_ID, defineThemes, registerEml } from "./eml-language.js";
import { ResultView, h } from "./result.js";
import { SettingsDrawer, normalizeSettings } from "./settings.js";
import { LONG_LINK, clearShareHash, decodeSharedWorkspace, encodeShareUrl, hasSharedWorkspace, portable, validatePortable } from "./share.js";
import { LANGUAGE_ID, markersFor, registerSieve } from "./sieve-language.js";
import { dismissWelcome, isWelcomeDismissed, loadPrefs, newId, openStore, savePrefs } from "./store.js";

const $ = (selector) => document.querySelector(selector);
const COMPILE_DELAY = 300;
const SAVE_DELAY = 400;
const NARROW = window.matchMedia("(max-width: 900px)");
const DARK = window.matchMedia("(prefers-color-scheme: dark)");
const IS_MAC = /Mac|iPhone|iPad/.test(navigator.platform);

const BLANK_SCRIPT = `require ["fileinto", "imap4flags"];

# Write your Sieve script here. Press Run to test it.
# Everything runs in your browser: nothing is sent to or stored on a server.
if header :contains "Subject" "hello" {
    addflag "\\\\Flagged";
    fileinto "INBOX";
}
`;

const BLANK_MESSAGE = `From: Sender <sender@example.com>
To: Jane Doe <jane@example.org>
Subject: Hello from Sievepad
Date: Mon, 14 Sep 2026 10:00:00 +0000
Message-ID: <hello-1@example.com>
Content-Type: text/plain; charset="utf-8"

Hello! This is a test message.
`;

const state = {
  monaco: null,
  engine: null,
  store: null,
  prefs: loadPrefs(),
  defaults: null,
  capabilities: [],
  samples: [],
  workspace: null,
  workspaces: [],
  compileSeq: 0,
  runSeq: 0,
  diagnostics: [],
  lastOutput: null,
  lastRunMessage: "",
  stale: true,
  contentVersion: 0,
};

let scriptEditor;
let messageEditor;
let result;
let settingsDrawer;
let saveTimer;
let compileTimer;

function toast(message) {
  const el = $("#toast");
  el.textContent = message;
  el.hidden = false;
  clearTimeout(toast.timer);
  toast.timer = setTimeout(() => (el.hidden = true), 2600);
}

document.addEventListener("toast", (event) => toast(event.detail));

window.addEventListener("unhandledrejection", (event) => {
  if (event.reason?.name === "Canceled" && event.reason?.message === "Canceled") event.preventDefault();
});

function setStatus(id, text, stateName) {
  const el = $(id);
  el.textContent = text;
  if (stateName) el.dataset.state = stateName;
  else delete el.dataset.state;
}

function theme() {
  return DARK.matches ? "sievepad-dark" : "sievepad-light";
}

function settings() {
  return state.workspace?.settings || state.defaults;
}

function editorOptions(extra) {
  return {
    automaticLayout: true,
    minimap: { enabled: false },
    fontSize: 13,
    lineNumbersMinChars: 3,
    scrollBeyondLastLine: false,
    tabSize: 4,
    insertSpaces: true,
    fixedOverflowWidgets: true,
    stickyScroll: { enabled: false },
    padding: { top: 8 },
    theme: theme(),
    ...extra,
  };
}

function modelFor(item, language) {
  if (!item.model || item.model.isDisposed()) {
    const model = state.monaco.editor.createModel(item.source, language);
    Object.defineProperty(item, "model", { value: model, writable: true, enumerable: false, configurable: true });
    Object.defineProperty(item, "source", {
      get: () => (model.isDisposed() ? "" : model.getValue()),
      set: (value) => model.setValue(value),
      enumerable: true,
      configurable: true,
    });
    model.onDidChangeContent(() => onContentChanged(language === LANGUAGE_ID));
  }
  return item.model;
}

function disposeModels(workspace) {
  if (!workspace) return;
  for (const item of [...workspace.scripts, ...workspace.messages]) item.model?.dispose();
}

function serializable(workspace) {
  return {
    id: workspace.id,
    name: workspace.name,
    created: workspace.created,
    updated: workspace.updated,
    scripts: workspace.scripts.map(({ name, source }) => ({ name, source })),
    messages: workspace.messages.map(({ name, source }) => ({ name, source })),
    activeScript: workspace.activeScript,
    activeMessage: workspace.activeMessage,
    settings: workspace.settings,
  };
}

function makeWorkspace({ name, scripts, messages, settings: overrides }) {
  const now = Date.now();
  return {
    id: newId(),
    name,
    created: now,
    updated: now,
    scripts: scripts.length ? scripts : [{ name: "main", source: BLANK_SCRIPT }],
    messages: messages.length ? messages : [{ name: "message.eml", source: BLANK_MESSAGE }],
    activeScript: 0,
    activeMessage: 0,
    settings: normalizeSettings({ ...state.defaults, ...(overrides || {}) }, state.defaults),
  };
}

function uniqueName(base) {
  const names = new Set(state.workspaces.map((ws) => ws.name));
  if (!names.has(base)) return base;
  let n = 2;
  while (names.has(`${base} ${n}`)) n++;
  return `${base} ${n}`;
}

async function saveNow() {
  clearTimeout(saveTimer);
  saveTimer = null;
  const ws = state.workspace;
  if (!ws) return;
  ws.updated = Date.now();
  const data = serializable(ws);
  const index = state.workspaces.findIndex((item) => item.id === ws.id);
  if (index >= 0) state.workspaces[index] = data;
  else state.workspaces.push(data);
  try {
    await state.store.put(data);
  } catch (err) {
    toast(`Could not save: ${err.message}`);
  }
}

function saveSoon() {
  clearTimeout(saveTimer);
  saveTimer = setTimeout(saveNow, SAVE_DELAY);
}

function onContentChanged(isScript) {
  state.contentVersion++;
  saveSoon();
  markStale();
  if (isScript) compileSoon();
}

function markStale() {
  state.stale = true;
  $("#run-button").classList.add("stale");
}

async function loadSamples() {
  try {
    const response = await fetch(new URL("./samples/index.json", import.meta.url));
    state.samples = await response.json();
  } catch (_) {
    state.samples = [];
  }
}

async function sampleWorkspace(sample) {
  const read = async (file) => {
    const response = await fetch(new URL(`./samples/${file}`, import.meta.url));
    if (!response.ok) throw new Error(`Could not load ${file}`);
    return response.text();
  };
  const scripts = await Promise.all(sample.scripts.map(async (s) => ({ name: s.name, source: await read(s.file) })));
  const messages = await Promise.all(sample.messages.map(async (m) => ({ name: m.name, source: await read(m.file) })));
  return makeWorkspace({ name: uniqueName(sample.title), scripts, messages, settings: sample.settings });
}

async function openWorkspace(data, { run = true } = {}) {
  if (state.workspace) await saveNow();
  disposeModels(state.workspace);
  const ws = {
    ...data,
    scripts: data.scripts.map((s) => ({ ...s })),
    messages: data.messages.map((m) => ({ ...m })),
    settings: normalizeSettings({ ...state.defaults, ...(data.settings || {}) }, state.defaults),
  };
  ws.activeScript = Math.min(ws.activeScript || 0, ws.scripts.length - 1);
  ws.activeMessage = Math.min(ws.activeMessage || 0, ws.messages.length - 1);
  state.workspace = ws;
  state.lastOutput = null;
  state.prefs.lastWorkspace = ws.id;
  savePrefs(state.prefs);

  scriptEditor.setModel(modelFor(ws.scripts[ws.activeScript], LANGUAGE_ID));
  messageEditor.setModel(modelFor(ws.messages[ws.activeMessage], EML_ID));
  renderScriptTabs();
  renderMessageTabs();
  renderWorkspaceMenu();
  renderEnvelope();
  result.renderEmpty();
  await saveNow();
  if (run) await runScript();
  else await compile();
}

function renderWorkspaceMenu() {
  $("#workspace-name").textContent = state.workspace?.name || "Workspace";
  const listEl = $("#workspace-list");
  const sorted = [...state.workspaces].sort((a, b) => b.updated - a.updated);
  listEl.replaceChildren(
    ...sorted.map((ws) =>
      h("button", { type: "button", role: "menuitem", class: ws.id === state.workspace?.id ? "current" : "", onclick: async () => { closeMenus(); if (ws.id !== state.workspace.id) await openWorkspace(ws); } },
        ws.name,
        h("span", { class: "item-sub" }, `${ws.scripts.length} script${ws.scripts.length === 1 ? "" : "s"} · edited ${relativeTime(ws.updated)}`),
      ),
    ),
  );
}

function renderExamplesMenu() {
  $("#examples-list").replaceChildren(
    ...state.samples.map((sample) =>
      h("button", { type: "button", role: "menuitem", onclick: async () => { closeMenus(); try { await openWorkspace(await sampleWorkspace(sample)); toast(`Opened "${sample.title}"`); } catch (err) { toast(err.message); } } },
        sample.title,
        h("span", { class: "item-sub" }, sample.description),
      ),
    ),
  );
}

function relativeTime(ms) {
  const minutes = Math.round((Date.now() - ms) / 60000);
  if (minutes < 1) return "just now";
  if (minutes < 60) return `${minutes} min ago`;
  const hours = Math.round(minutes / 60);
  if (hours < 24) return `${hours} h ago`;
  return new Date(ms).toLocaleDateString();
}

function tabButton({ label, active, error, closable, kind, onSelect, onClose, onRename }) {
  const button = h("button", { type: "button", role: "tab", class: `tab${active ? " active" : ""}`, "aria-selected": active ? "true" : "false", title: onRename ? "Double-click to rename" : undefined, onclick: onSelect, ondblclick: onRename },
    error ? h("span", { class: "dot", title: "Has errors" }) : null,
    label,
    kind ? h("span", { class: "tab-kind" }, kind) : null,
  );
  if (closable) {
    const close = h("span", { class: "tab-close", role: "button", title: "Remove", onclick: (event) => { event.stopPropagation(); onClose(); } });
    close.innerHTML = '<svg viewBox="0 0 16 16"><path d="M4 4l8 8M12 4l-8 8"/></svg>';
    button.append(close);
  }
  return button;
}

function renderScriptTabs() {
  const ws = state.workspace;
  const errors = new Set(state.diagnostics.map((d) => d.script));
  $("#script-tabs").replaceChildren(
    ...ws.scripts.map((script, index) =>
      tabButton({
        label: script.name,
        kind: index === 0 ? "main" : "include",
        active: index === ws.activeScript,
        error: errors.has(index),
        closable: index > 0,
        onSelect: () => selectScript(index),
        onRename: index > 0 ? () => renameScript(index) : undefined,
        onClose: () => removeScript(index),
      }),
    ),
  );
}

function renderMessageTabs() {
  const ws = state.workspace;
  $("#message-tabs").replaceChildren(
    ...ws.messages.map((message, index) =>
      tabButton({
        label: message.name,
        active: index === ws.activeMessage,
        closable: ws.messages.length > 1,
        onSelect: () => selectMessage(index),
        onRename: () => renameMessage(index),
        onClose: () => removeMessage(index),
      }),
    ),
  );
}

function selectScript(index) {
  const ws = state.workspace;
  ws.activeScript = index;
  scriptEditor.setModel(modelFor(ws.scripts[index], LANGUAGE_ID));
  renderScriptTabs();
  renderProblems();
  saveSoon();
}

function selectMessage(index) {
  const ws = state.workspace;
  ws.activeMessage = index;
  state.contentVersion++;
  messageEditor.setModel(modelFor(ws.messages[index], EML_ID));
  renderMessageTabs();
  markStale();
  saveSoon();
}

async function addScript(name, source) {
  const ws = state.workspace;
  const chosen = name ?? (await promptText("New include script", "Scripts include it by this name, for example include \"common\".", nextName(ws.scripts.map((s) => s.name), "common")));
  if (!chosen) return;
  ws.scripts.push({ name: chosen, source: source ?? 'require ["variables"];\n\n' });
  selectScript(ws.scripts.length - 1);
  compileSoon();
}

async function addMessage(name, source) {
  const ws = state.workspace;
  ws.messages.push({ name: name || nextName(ws.messages.map((m) => m.name), "message.eml"), source: source ?? BLANK_MESSAGE });
  selectMessage(ws.messages.length - 1);
}

function nextName(existing, base) {
  if (!existing.includes(base)) return base;
  const dot = base.lastIndexOf(".");
  const stem = dot > 0 ? base.slice(0, dot) : base;
  const ext = dot > 0 ? base.slice(dot) : "";
  let n = 2;
  while (existing.includes(`${stem}-${n}${ext}`)) n++;
  return `${stem}-${n}${ext}`;
}

async function renameScript(index) {
  const ws = state.workspace;
  const name = await promptText("Rename include script", "Other scripts include it by this name.", ws.scripts[index].name);
  if (!name) return;
  ws.scripts[index].name = name;
  renderScriptTabs();
  saveSoon();
  compileSoon();
}

async function renameMessage(index) {
  const ws = state.workspace;
  const name = await promptText("Rename message", "", ws.messages[index].name);
  if (!name) return;
  ws.messages[index].name = name;
  renderMessageTabs();
  saveSoon();
}

function removeScript(index) {
  const ws = state.workspace;
  if (index === 0) return;
  if (ws.scripts[index].source.trim() && !confirm(`Remove the script "${ws.scripts[index].name}"?`)) return;
  ws.scripts[index].model?.dispose();
  ws.scripts.splice(index, 1);
  const active = ws.activeScript >= index ? ws.activeScript - 1 : ws.activeScript;
  selectScript(Math.min(Math.max(active, 0), ws.scripts.length - 1));
  compileSoon();
}

function removeMessage(index) {
  const ws = state.workspace;
  if (ws.messages.length < 2) return;
  if (!confirm(`Remove the message "${ws.messages[index].name}"?`)) return;
  ws.messages[index].model?.dispose();
  ws.messages.splice(index, 1);
  const active = ws.activeMessage >= index ? ws.activeMessage - 1 : ws.activeMessage;
  selectMessage(Math.min(Math.max(active, 0), ws.messages.length - 1));
}

function renderEnvelope() {
  const s = settings();
  $("#envelope-from").value = s.envelopeFrom || "";
  $("#envelope-from").placeholder = "taken from the message";
  $("#envelope-to").value = (s.envelopeTo || []).join(", ");
  $("#envelope-to").placeholder = s.userAddress || "the user address";
  $("#envelope-summary").textContent = `${s.envelopeFrom || "sender from message"} → ${(s.envelopeTo || []).join(", ") || s.userAddress}`;
}

function updateSettings(next) {
  state.contentVersion++;
  state.workspace.settings = normalizeSettings(next, state.defaults);
  renderEnvelope();
  saveSoon();
  markStale();
  compileSoon();
}

function request(extra = {}) {
  const ws = state.workspace;
  return {
    scripts: ws.scripts.map(({ name, source }) => ({ name, source })),
    message: ws.messages[ws.activeMessage]?.source || "",
    settings: ws.settings,
    seenIds: [],
    now: Math.floor(Date.now() / 1000),
    ...extra,
  };
}

function compileSoon() {
  clearTimeout(compileTimer);
  compileTimer = setTimeout(compile, COMPILE_DELAY);
}

async function compile() {
  clearTimeout(compileTimer);
  const seq = ++state.compileSeq;
  const ws = state.workspace;
  let output;
  try {
    output = await state.engine.compile({ scripts: ws.scripts.map(({ name, source }) => ({ name, source })), settings: ws.settings });
  } catch (err) {
    if (seq === state.compileSeq) setStatus("#compile-status", `Compiler unavailable: ${err.message}`, "error");
    return;
  }
  if (seq !== state.compileSeq || ws !== state.workspace) return;
  applyDiagnostics(output.diagnostics);
}

function applyDiagnostics(diagnostics) {
  const ws = state.workspace;
  state.diagnostics = diagnostics;
  ws.scripts.forEach((script, index) => {
    const model = modelFor(script, LANGUAGE_ID);
    state.monaco.editor.setModelMarkers(model, "sieve", markersFor(state.monaco, model, diagnostics.filter((d) => d.script === index)));
  });
  renderScriptTabs();
  renderProblems();
  if (diagnostics.length) {
    setStatus("#compile-status", `${diagnostics.length} error${diagnostics.length === 1 ? "" : "s"}`, "error");
  } else {
    setStatus("#compile-status", "No problems", "ready");
  }
}

function renderProblems() {
  const ws = state.workspace;
  const el = $("#problems");
  const runtime = state.lastOutput?.error && state.lastOutput.error.line > 0 ? [state.lastOutput.error] : [];
  const items = [...state.diagnostics, ...runtime];
  el.dataset.state = items.length ? "active" : "idle";
  el.replaceChildren(
    ...items.map((d) => {
      const scriptName = ws.scripts[d.script]?.name || "main";
      const where = d.line > 0 ? `${scriptName}:${d.line}:${Math.max(d.column, 1)}` : `${scriptName}`;
      const capability = /Undeclared capability '([^']+)'/.exec(d.message);
      return h("button", { type: "button", class: `problem ${d.severity}`, onclick: () => revealLine(d.script, d.line, d.column) },
        h("span", { class: "sev" }),
        h("span", { class: "where" }, where),
        h("span", {}, d.message, d.line > 0 ? "" : " (position unknown)"),
        capability ? h("span", { class: "fix" }, "Quick fix available") : null,
      );
    }),
  );
}

function revealLine(scriptIndex, line, column) {
  if (scriptIndex !== state.workspace.activeScript) selectScript(scriptIndex);
  showPanel("script");
  const lineNumber = Math.max(1, line || 1);
  scriptEditor.revealLineInCenter(lineNumber);
  scriptEditor.setPosition({ lineNumber, column: Math.max(1, column || 1) });
  scriptEditor.focus();
}

async function runScript({ redeliver = false } = {}) {
  if (!state.workspace) return;
  const button = $("#run-button");
  button.disabled = true;
  await saveNow();
  const ws = state.workspace;
  const version = state.contentVersion;
  const seq = ++state.runSeq;
  setStatus("#engine-status", "Running…", "busy");
  const seenIds = redeliver && state.lastOutput ? state.lastOutput.duplicateIds : [];
  const req = request({ seenIds });
  const started = performance.now();
  let output;
  try {
    output = await state.engine.run(req);
  } catch (err) {
    if (seq !== state.runSeq) return;
    button.disabled = false;
    if (ws !== state.workspace) return;
    result.renderFailure(err.message);
    setEngineStatus();
    return;
  }
  if (seq !== state.runSeq) return;
  const elapsed = performance.now() - started;
  button.disabled = false;
  setEngineStatus();
  if (ws !== state.workspace) return;
  if (version === state.contentVersion) {
    button.classList.remove("stale");
    state.stale = false;
  }
  applyDiagnostics(output.diagnostics);

  ws.scripts.forEach((script, index) => {
    const runtimeMarkers = output.error && output.error.line > 0 && output.error.script === index ? markersFor(state.monaco, modelFor(script, LANGUAGE_ID), [output.error]) : [];
    state.monaco.editor.setModelMarkers(modelFor(script, LANGUAGE_ID), "sieve-runtime", runtimeMarkers);
  });

  if (output.diagnostics.length) {
    state.lastOutput = null;
    result.renderCompileErrors(output.diagnostics.length, () => revealLine(output.diagnostics[0].script, output.diagnostics[0].line, output.diagnostics[0].column));
    setStatus("#run-stats", "");
    return;
  }
  state.lastOutput = output;
  state.lastRunMessage = req.message;
  result.render(output);
  renderProblems();
  selectResultView("actions");
  setStatus("#run-stats", `${output.instructions.toLocaleString()} instructions · ${elapsed < 1 ? "<1" : Math.round(elapsed)} ms${redeliver ? " · redelivery" : ""}`);
  if (NARROW.matches) showPanel("result");
}

function selectResultView(view) {
  for (const tab of document.querySelectorAll("#result-tabs .tab")) tab.classList.toggle("active", tab.dataset.view === view);
  for (const pane of document.querySelectorAll(".result-view")) pane.classList.toggle("active", pane.dataset.view === view);
  if (view === "messages") requestAnimationFrame(() => result.onShown());
  if (NARROW.matches) showPanel("result");
}

function showPanel(name) {
  for (const button of document.querySelectorAll(".mobile-tabs button")) button.classList.toggle("active", button.dataset.panel === name);
  for (const panel of document.querySelectorAll(".panel")) panel.classList.toggle("active", panel.dataset.panel === name);
}

function closeMenus() {
  for (const menu of document.querySelectorAll(".menu")) {
    menu.querySelector(".menu-panel").hidden = true;
    menu.querySelector(".menu-trigger").setAttribute("aria-expanded", "false");
  }
}

function setupMenus() {
  for (const menu of document.querySelectorAll(".menu")) {
    const trigger = menu.querySelector(".menu-trigger");
    const panel = menu.querySelector(".menu-panel");
    trigger.addEventListener("click", (event) => {
      event.stopPropagation();
      const open = panel.hidden;
      closeMenus();
      if (open) {
        if (menu.id === "workspace-menu") renderWorkspaceMenu();
        panel.hidden = false;
        trigger.setAttribute("aria-expanded", "true");
        panel.querySelector("button")?.focus();
      }
    });
  }
  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape") closeMenus();
  });
  document.addEventListener(
    "pointerdown",
    (event) => {
      if (!event.target.closest(".menu")) closeMenus();
    },
    true,
  );
  document.addEventListener("click", async (event) => {
    const target = event.target.closest("[data-action]");
    if (!target) return;
    closeMenus();
    const action = ACTIONS[target.dataset.action];
    if (action) await action();
  });
}

const ACTIONS = {
  "new-workspace": async () => {
    const name = await promptText("New workspace", "A workspace holds scripts, test messages and settings.", uniqueName("Untitled"));
    if (!name) return;
    await openWorkspace(makeWorkspace({ name, scripts: [], messages: [] }));
  },
  "duplicate-workspace": async () => {
    const ws = state.workspace;
    await openWorkspace(makeWorkspace({ name: uniqueName(`${ws.name} copy`), scripts: serializable(ws).scripts, messages: serializable(ws).messages, settings: ws.settings }));
    toast("Workspace duplicated");
  },
  "rename-workspace": async () => {
    const name = await promptText("Rename workspace", "", state.workspace.name);
    if (!name) return;
    state.workspace.name = name;
    await saveNow();
    renderWorkspaceMenu();
  },
  "delete-workspace": async () => {
    const ws = state.workspace;
    if (!confirm(`Delete the workspace "${ws.name}"? This cannot be undone.`)) return;
    await state.store.delete(ws.id);
    state.workspaces = state.workspaces.filter((item) => item.id !== ws.id);
    disposeModels(ws);
    state.workspace = null;
    const next = [...state.workspaces].sort((a, b) => b.updated - a.updated)[0];
    if (next) await openWorkspace(next);
    else await openWorkspace(makeWorkspace({ name: uniqueName("Untitled"), scripts: [], messages: [] }));
    toast("Workspace deleted");
  },
  "export-workspace": async () => {
    $("#share-dialog").close();
    const data = portable(state.workspace, state.defaults);
    const blob = new Blob([JSON.stringify(data, null, 2)], { type: "application/json" });
    const link = h("a", { href: URL.createObjectURL(blob), download: `${state.workspace.name.replace(/[^\w.-]+/g, "-")}.sievepad.json` });
    document.body.append(link);
    link.click();
    link.remove();
    setTimeout(() => URL.revokeObjectURL(link.href), 1000);
  },
  "import-workspace": async () => {
    $("#file-input").value = "";
    $("#file-input").click();
  },
};

async function importFile(file) {
  const text = await file.text();
  const lower = file.name.toLowerCase();
  if (lower.endsWith(".json") || /^\s*\{/.test(text)) {
    try {
      const data = validatePortable(JSON.parse(text));
      await openWorkspace(makeWorkspace({ ...data, name: uniqueName(data.name) }));
      toast(`Imported "${data.name}"`);
    } catch (err) {
      toast(`Import failed: ${err.message}`);
    }
  } else if (lower.endsWith(".sieve") || lower.endsWith(".siv") || /^\s*(require|#|if\b)/.test(text)) {
    const name = file.name.replace(/\.(sieve|siv|txt)$/i, "");
    const ws = state.workspace;
    if (ws.scripts.length === 1 && ws.scripts[0].source === BLANK_SCRIPT) {
      ws.scripts[0].model.setValue(text);
    } else if (confirm(`Replace the main script with "${file.name}"? Choose Cancel to add it as an include script instead.`)) {
      selectScript(0);
      ws.scripts[0].model.setValue(text);
    } else {
      await addScript(name, text);
    }
    toast(`Loaded ${file.name}`);
  } else {
    await addMessage(file.name, text);
    toast(`Added ${file.name} as a test message`);
  }
}

function setupFiles() {
  $("#file-input").addEventListener("change", async (event) => {
    for (const file of event.target.files) await importFile(file);
  });
  let depth = 0;
  window.addEventListener("dragenter", (event) => {
    if (!event.dataTransfer?.types.includes("Files")) return;
    depth++;
    document.body.classList.add("dragging-file");
  });
  window.addEventListener("dragleave", () => {
    depth = Math.max(0, depth - 1);
    if (depth === 0) document.body.classList.remove("dragging-file");
  });
  window.addEventListener("dragover", (event) => {
    if (event.dataTransfer?.types.includes("Files")) event.preventDefault();
  });
  window.addEventListener("drop", async (event) => {
    if (!event.dataTransfer?.files.length) return;
    event.preventDefault();
    depth = 0;
    document.body.classList.remove("dragging-file");
    for (const file of event.dataTransfer.files) await importFile(file);
  });
}

function setEngineStatus() {
  if (state.engine.failed) setStatus("#engine-status", state.engine.failed, "error");
  else setStatus("#engine-status", "Engine ready", "ready");
}

function promptText(title, text, value) {
  const dialog = $("#prompt-dialog");
  $("#prompt-title").textContent = title;
  $("#prompt-text").textContent = text;
  $("#prompt-text").hidden = !text;
  const input = $("#prompt-input");
  input.value = value || "";
  return new Promise((resolve) => {
    dialog.onclose = () => {
      const name = input.value.trim();
      resolve(dialog.returnValue === "ok" && name ? name.slice(0, 80) : null);
    };
    input.onkeydown = (event) => {
      if (event.key === "Enter" && !event.isComposing) {
        event.preventDefault();
        dialog.close("ok");
      }
    };
    dialog.returnValue = "";
    dialog.showModal();
    input.select();
  });
}

function setupShare() {
  const dialog = $("#share-dialog");
  const includeMessages = $("#share-messages");
  const refresh = async () => {
    try {
      const url = await encodeShareUrl(portable(state.workspace, state.defaults, { includeMessages: includeMessages.checked }));
      $("#share-url").value = url;
      $("#share-privacy").hidden = !includeMessages.checked;
      $("#share-size").textContent = url.length > LONG_LINK
        ? `This link is ${(url.length / 1024).toFixed(1)} KB long. Some chat apps and email clients cut long links: consider sharing without messages or exporting to a file.`
        : `Link length: ${url.length.toLocaleString()} characters.`;
    } catch (err) {
      $("#share-url").value = "";
      $("#share-size").textContent = `Could not create a link: ${err.message}`;
    }
  };
  includeMessages.addEventListener("change", refresh);
  $("#share-button").addEventListener("click", async () => {
    await saveNow();
    await refresh();
    dialog.showModal();
    $("#share-url").select();
  });
  $("#share-copy").addEventListener("click", async () => {
    try {
      await navigator.clipboard.writeText($("#share-url").value);
      $("#share-copy").textContent = "Copied";
      setTimeout(() => ($("#share-copy").textContent = "Copy link"), 1500);
    } catch (_) {
      $("#share-url").select();
      toast("Press Ctrl+C to copy the selected link");
    }
  });
}

function setupGutters() {
  const workbench = $("#workbench");
  let columns = Array.isArray(state.prefs.columns) && state.prefs.columns.length === 3 ? state.prefs.columns : [1.15, 0.95, 1];
  const apply = () => {
    ["--col-a", "--col-b", "--col-c"].forEach((name, i) => workbench.style.setProperty(name, `minmax(220px, ${columns[i]}fr)`));
  };
  apply();
  for (const gutter of document.querySelectorAll(".gutter")) {
    const index = Number(gutter.dataset.gutter);
    const resize = (deltaFraction) => {
      const total = columns[index] + columns[index + 1];
      const left = Math.min(Math.max(columns[index] + deltaFraction, total * 0.15), total * 0.85);
      columns = columns.map((value, i) => (i === index ? left : i === index + 1 ? total - left : value));
      apply();
    };
    gutter.addEventListener("pointerdown", (event) => {
      event.preventDefault();
      gutter.setPointerCapture(event.pointerId);
      gutter.classList.add("dragging");
      const sum = columns.reduce((a, b) => a + b, 0);
      const width = workbench.clientWidth - 12;
      let last = event.clientX;
      const move = (e) => {
        resize(((e.clientX - last) / width) * sum);
        last = e.clientX;
      };
      const up = () => {
        gutter.classList.remove("dragging");
        gutter.removeEventListener("pointermove", move);
        gutter.removeEventListener("pointerup", up);
        state.prefs.columns = columns;
        savePrefs(state.prefs);
      };
      gutter.addEventListener("pointermove", move);
      gutter.addEventListener("pointerup", up);
    });
    gutter.addEventListener("keydown", (event) => {
      if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
      event.preventDefault();
      resize((event.key === "ArrowLeft" ? -0.05 : 0.05) * columns.reduce((a, b) => a + b, 0));
      state.prefs.columns = columns;
      savePrefs(state.prefs);
    });
    gutter.addEventListener("dblclick", () => {
      columns = [1.15, 0.95, 1];
      apply();
      state.prefs.columns = columns;
      savePrefs(state.prefs);
    });
  }
}

function setupKeys() {
  const runKey = state.monaco.KeyMod.CtrlCmd | state.monaco.KeyCode.Enter;
  const saveKey = state.monaco.KeyMod.CtrlCmd | state.monaco.KeyCode.KeyS;
  for (const editor of [scriptEditor, messageEditor]) {
    editor.addAction({ id: "sievepad.run", label: "Sievepad: Run script", keybindings: [runKey], run: () => runScript() });
    editor.addAction({ id: "sievepad.redeliver", label: "Sievepad: Run again as a redelivery", run: () => runScript({ redeliver: true }) });
    editor.addAction({ id: "sievepad.save", label: "Sievepad: Save workspace", keybindings: [saveKey], run: async () => { await saveNow(); toast("Saved in this browser"); } });
    editor.addAction({ id: "sievepad.share", label: "Sievepad: Share workspace", run: () => $("#share-button").click() });
    editor.addAction({ id: "sievepad.settings", label: "Sievepad: Open settings", run: () => settingsDrawer.open() });
  }
  scriptEditor.addAction({ id: "sievepad.format", label: "Sievepad: Format script", run: (editor) => editor.getAction("editor.action.formatDocument")?.run() });
  document.addEventListener("keydown", async (event) => {
    const mod = IS_MAC ? event.metaKey : event.ctrlKey;
    if (mod && event.key === "Enter") {
      event.preventDefault();
      await runScript();
    } else if (mod && event.key.toLowerCase() === "s") {
      event.preventDefault();
      await saveNow();
      toast("Saved in this browser");
    }
  });
  $("#run-button").title = `Run (${IS_MAC ? "⌘" : "Ctrl"}+Enter)`;
}

function setupEnvelope() {
  $("#envelope-from").addEventListener("change", (event) => updateSettings({ ...settings(), envelopeFrom: event.target.value.trim() }));
  $("#envelope-to").addEventListener("change", (event) =>
    updateSettings({ ...settings(), envelopeTo: event.target.value.split(",").map((s) => s.trim()).filter(Boolean) }),
  );
}

function setupWelcome() {
  const dialog = $("#welcome-dialog");
  $("#privacy-link").addEventListener("click", () => dialog.showModal());
  dialog.querySelector("button.primary").addEventListener("click", dismissWelcome);
  dialog.addEventListener("cancel", dismissWelcome);
  dialog.addEventListener("close", dismissWelcome);
  if (!isWelcomeDismissed()) dialog.showModal();
}

async function initialWorkspace() {
  if (hasSharedWorkspace()) {
    try {
      const data = await decodeSharedWorkspace();
      clearShareHash();
      const ws = makeWorkspace({ ...data, name: uniqueName(`${data.name} (shared)`) });
      toast("Opened a shared workspace. Your own workspaces are in the workspace menu.");
      return ws;
    } catch (err) {
      clearShareHash();
      toast(`The shared link is invalid: ${err.message}`);
    }
  }
  const last = state.workspaces.find((ws) => ws.id === state.prefs.lastWorkspace);
  if (last) return last;
  const recent = [...state.workspaces].sort((a, b) => b.updated - a.updated)[0];
  if (recent) return recent;
  const welcome = state.samples.find((sample) => sample.id === "welcome");
  if (welcome) {
    try {
      return await sampleWorkspace(welcome);
    } catch (_) {
      return makeWorkspace({ name: "Untitled", scripts: [], messages: [] });
    }
  }
  return makeWorkspace({ name: "Untitled", scripts: [], messages: [] });
}

function clearReloadFlag() {
  try {
    sessionStorage.removeItem("sievepad.reloaded");
  } catch (_) {
    return;
  }
}

async function boot() {
  clearReloadFlag();
  let monaco;
  try {
    monaco = await window.monacoReady;
  } catch (err) {
    setStatus("#engine-status", "Editor failed to load", "error");
    console.error(err);
    return;
  }
  state.monaco = monaco;
  defineThemes(monaco);
  registerEml(monaco);
  registerSieve(monaco, () => settings() || {});

  scriptEditor = monaco.editor.create($("#script-editor"), editorOptions({ language: LANGUAGE_ID, ariaLabel: "Sieve script", wordBasedSuggestions: "off", "semanticHighlighting.enabled": false }));
  messageEditor = monaco.editor.create($("#message-editor"), editorOptions({ language: EML_ID, wordWrap: "on", ariaLabel: "Test message" }));

  state.engine = new Engine();
  state.engine.onStatus((kind, detail) => {
    if (kind === "failed") setStatus("#engine-status", detail, "error");
    else if (kind === "restarting" && detail !== "timeout") setStatus("#engine-status", "Restarting engine…", "busy");
  });

  try {
    const [version, defaults, capabilities, store] = await Promise.all([
      state.engine.version(),
      state.engine.defaults(),
      state.engine.capabilities(),
      openStore(),
      loadSamples(),
    ]);
    state.defaults = defaults;
    state.capabilities = capabilities;
    state.store = store;
    $("#engine-version").textContent = `v${version}`;
    setStatus("#engine-status", "Engine ready", "ready");
  } catch (err) {
    setStatus("#engine-status", `Engine failed: ${err.message}`, "error");
    console.error(err);
    return;
  }

  state.workspaces = await state.store.list().catch(() => []);

  result = new ResultView({
    monaco,
    root: $("#result-body"),
    onRevealLine: revealLine,
    onRunAgain: () => runScript({ redeliver: true }),
    onSelectView: selectResultView,
    getOriginal: () => state.lastRunMessage,
    getTheme: theme,
  });

  settingsDrawer = new SettingsDrawer({
    drawer: $("#settings-drawer"),
    backdrop: $("#settings-backdrop"),
    body: $("#settings-body"),
    resetButton: $("#settings-reset"),
    jsonButton: $("#settings-json"),
    defaults: state.defaults,
    capabilities: state.capabilities,
    get: () => state.workspace.settings,
    set: updateSettings,
  });

  $("#run-button").addEventListener("click", () => runScript());
  $("#settings-button").addEventListener("click", () => settingsDrawer.open());
  $("#add-script").addEventListener("click", () => addScript());
  $("#add-message").addEventListener("click", () => addMessage());
  for (const tab of document.querySelectorAll("#result-tabs .tab")) tab.addEventListener("click", () => selectResultView(tab.dataset.view));
  for (const button of document.querySelectorAll(".mobile-tabs button")) button.addEventListener("click", () => showPanel(button.dataset.panel));
  DARK.addEventListener("change", () => monaco.editor.setTheme(theme()));
  window.addEventListener("hashchange", async () => {
    if (!hasSharedWorkspace()) return;
    await openWorkspace(await initialWorkspace());
  });
  document.addEventListener("visibilitychange", () => {
    if (document.visibilityState === "hidden" && saveTimer) saveNow();
  });
  window.addEventListener("pagehide", () => {
    if (saveTimer) saveNow();
  });

  setupMenus();
  setupShare();
  setupFiles();
  setupGutters();
  setupKeys();
  setupEnvelope();
  setupWelcome();
  renderExamplesMenu();

  await openWorkspace(await initialWorkspace());
}

boot();
