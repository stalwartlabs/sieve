import { EML_ID } from "./eml-language.js";

export function h(tag, attrs = {}, ...children) {
  const el = document.createElement(tag);
  for (const [key, value] of Object.entries(attrs || {})) {
    if (value === undefined || value === null || value === false) continue;
    if (key === "class") el.className = value;
    else if (key === "dataset") Object.assign(el.dataset, value);
    else if (key.startsWith("on")) el.addEventListener(key.slice(2), value);
    else el.setAttribute(key, value === true ? "" : value);
  }
  for (const child of children.flat()) {
    if (child === null || child === undefined || child === false) continue;
    el.append(child instanceof Node ? child : document.createTextNode(String(child)));
  }
  return el;
}

const ICONS = {
  keep: '<path d="M2 9l2-6h8l2 6v4H2zM2 9h4l1 2h2l1-2h4"/>',
  fileinto: '<path d="M2 4h4l1.5 1.5H14V13H2z"/>',
  discard: '<path d="M3 4h10M6 4V2.5h4V4M4.5 4l.7 9.5h5.6l.7-9.5"/>',
  reject: '<circle cx="8" cy="8" r="5.5"/><path d="M4.2 11.8l7.6-7.6"/>',
  redirect: '<path d="M2 11c1-4 4-6 9-6M9 2.5L11.5 5 9 7.5"/>',
  vacation: '<path d="M8 2v12M3 7a5 5 0 0110 0zM8 14h-3"/>',
  notification: '<path d="M4 11V7a4 4 0 018 0v4l1 1.5H3zM6.5 14h3"/>',
  notify: '<path d="M4 11V7a4 4 0 018 0v4l1 1.5H3zM6.5 14h3"/>',
  created: '<path d="M4 2h5l3 3v9H4zM9 2v3h3M6.5 9.5h3M8 8v3"/>',
  envelope: '<path d="M2 4h12v8H2zM2 4l6 5 6-5"/>',
  error: '<circle cx="8" cy="8" r="5.5"/><path d="M8 5v3.5M8 11h0"/>',
};

function icon(kind) {
  const span = h("span", { class: "event-icon", "aria-hidden": "true" });
  span.innerHTML = `<svg viewBox="0 0 16 16">${ICONS[kind] || ICONS.created}</svg>`;
  return span;
}

const ROLE_BY_KIND = {
  keep: "Delivered",
  fileinto: "Delivered",
  vacation: "Vacation reply",
  redirect: "Redirected",
  notification: "Notification",
};

export function messageRoles(output) {
  const roles = new Map();
  for (const event of output.events) {
    if (!event.messageId || event.kind === "created") continue;
    const role = ROLE_BY_KIND[event.kind];
    if (!role) continue;
    const set = roles.get(event.messageId) || new Set();
    set.add(role);
    roles.set(event.messageId, set);
  }
  return roles;
}

function roleLabel(roles, id) {
  const set = roles.get(id);
  if (!set || set.size === 0) return "Generated";
  return [...set].join(" · ");
}

function summarize(output) {
  const lines = [];
  const events = output.events;
  const byKind = (kind) => events.filter((event) => event.kind === kind);
  const flags = (event) => event.detail.find((d) => d.label === "flags")?.value;
  const withFlags = (event) => (flags(event) ? [" with flags ", h("b", {}, flags(event))] : []);

  for (const event of byKind("fileinto")) {
    const folder = event.summary.replace(/^File into /, "");
    lines.push(["Filed into ", h("b", {}, folder), ...withFlags(event)]);
  }
  for (const event of byKind("keep")) {
    lines.push([event.afterError ? "Kept in " : "Delivered to ", h("b", {}, "INBOX"), ...withFlags(event), event.afterError ? " because the script failed" : ""]);
  }
  for (const event of byKind("discard")) lines.push([h("b", {}, "Discarded"), " without notifying anyone"]);
  for (const event of byKind("reject")) {
    const reason = event.detail.find((d) => d.label === "reason")?.value || "";
    lines.push([h("b", {}, "Rejected"), reason ? `: "${reason}"` : ""]);
  }
  const redirects = byKind("redirect").map((event) => event.summary.replace(/^Redirect to /, ""));
  if (redirects.length) lines.push(["Redirected to ", h("b", {}, redirects.join(", "))]);
  for (const event of byKind("vacation")) lines.push(["Vacation reply sent to ", h("b", {}, event.summary.replace(/^Send a vacation reply to /, ""))]);
  for (const event of byKind("notification")) lines.push(["Notification sent to ", h("b", {}, event.summary.replace(/^Send a notification to /, ""))]);
  for (const event of byKind("notify")) lines.push([event.summary]);
  if (output.messageChanged) lines.push(["The message was ", h("b", {}, "modified"), " by the script"]);
  if (lines.length === 0) lines.push(["No actions were taken"]);
  return lines;
}

export class ResultView {
  constructor({ monaco, root, onRevealLine, onRunAgain, onSelectView, getOriginal, getTheme }) {
    this.monaco = monaco;
    this.actionsView = root.querySelector("#actions-view");
    this.messagesView = root.querySelector("#messages-view");
    this.variablesView = root.querySelector("#variables-view");
    this.countEl = document.querySelector("#messages-count");
    this.onRevealLine = onRevealLine;
    this.onRunAgain = onRunAgain;
    this.onSelectView = onSelectView;
    this.getOriginal = getOriginal;
    this.getTheme = getTheme;
    this.output = null;
    this.selected = null;
    this.mode = "preview";
    this.sourceEditor = null;
    this.diffEditor = null;
    this.renderEmpty();
  }

  renderEmpty() {
    this.output = null;
    this.countEl.textContent = "";
    this.actionsView.replaceChildren(
      h("div", { class: "empty" }, h("div", {}, h("strong", {}, "Nothing has run yet"), "Press ", h("kbd", {}, "Run"), " or ", h("kbd", {}, navigator.platform.includes("Mac") ? "⌘ Enter" : "Ctrl Enter"), " to filter the message through the script.")),
    );
    this.messagesView.replaceChildren(h("div", { class: "empty" }, h("div", {}, h("strong", {}, "No messages yet"), "Modified messages, vacation replies and notifications appear here after a run.")));
    this.variablesView.replaceChildren(h("div", { class: "empty" }, h("div", {}, h("strong", {}, "No variables yet"), "Global variables and duplicate ids appear here after a run.")));
    this.disposeEditors();
  }

  renderFailure(message) {
    this.renderEmpty();
    this.actionsView.replaceChildren(h("div", { class: "banner" }, message));
  }

  renderCompileErrors(count, onShow) {
    this.renderEmpty();
    this.actionsView.replaceChildren(
      h("div", { class: "banner" }, `The script has ${count} error${count === 1 ? "" : "s"} and cannot run.`, h("button", { type: "button", onclick: onShow }, "Show")),
    );
  }

  render(output) {
    this.output = output;
    this.roles = messageRoles(output);
    this.countEl.textContent = output.messages.length ? String(output.messages.length) : "";
    this.renderActions();
    if (!output.messages.some((message) => message.id === this.selected)) {
      const delivered = output.messages.find((message) => roleLabel(this.roles, message.id).includes("Delivered"));
      this.selected = (delivered || output.messages[0])?.id ?? null;
    }
    this.renderMessages();
    this.renderVariables();
  }

  renderActions() {
    const output = this.output;
    const nodes = [];

    if (output.error) {
      const where = output.error.line > 0 ? ` (line ${output.error.line})` : "";
      nodes.push(
        h("div", { class: "banner warn" }, h("span", {}, h("b", {}, "Runtime error: "), output.error.message, where),
          output.error.line > 0 ? h("button", { type: "button", onclick: () => this.onRevealLine(0, output.error.line, output.error.column) }, "Go to line") : null),
      );
    }

    nodes.push(
      h("div", { class: "outcome" },
        h("div", { class: "outcome-title" }, "Outcome"),
        h("div", { class: "outcome-lines" }, summarize(output).map((parts) => h("div", {}, ...parts))),
        h("div", { class: "outcome-actions" },
          output.messages.length ? h("button", { class: "btn", type: "button", onclick: () => this.onSelectView("messages") }, `View ${output.messages.length} message${output.messages.length === 1 ? "" : "s"}`) : null,
          h("button", { class: "btn", type: "button", title: "Run again with the duplicate ids and vacation history from this run", onclick: () => this.onRunAgain() }, "Run again as a redelivery"),
        ),
      ),
    );

    const list = h("ol", { class: "timeline" });
    for (const event of output.events) {
      const chips = event.detail.map((detail) =>
        h("span", { class: `chip${detail.label === "warning" ? " warn" : ""}` }, h("span", { class: "k" }, detail.label), detail.value),
      );
      if (event.isFinal) chips.unshift(h("span", { class: "chip tag" }, event.afterError ? "implicit keep" : "final action"));
      if (event.messageId > 0 && event.kind !== "created") {
        chips.push(h("button", { class: "chip chip-link", type: "button", onclick: () => this.showMessage(event.messageId) }, `message #${event.messageId}`));
      }
      if (event.kind === "created") {
        chips.push(h("button", { class: "chip chip-link", type: "button", onclick: () => this.showMessage(event.messageId) }, "view"));
      }
      list.append(
        h("li", { class: `event${event.isFinal ? " final" : ""}${event.afterError || event.kind === "created" ? " muted" : ""}`, dataset: { kind: event.kind } },
          icon(event.kind),
          h("div", { class: "event-card" }, h("div", { class: "event-summary" }, event.summary), chips.length ? h("div", { class: "event-meta" }, chips) : null),
        ),
      );
    }
    nodes.push(list);
    this.actionsView.replaceChildren(...nodes);
  }

  showMessage(id) {
    this.selected = id;
    this.renderMessages();
    this.onSelectView("messages");
  }

  renderMessages() {
    const output = this.output;
    this.disposeEditors();
    if (!output || output.messages.length === 0) {
      this.messagesView.replaceChildren(
        h("div", { class: "empty" }, h("div", {}, h("strong", {}, "No messages were generated"), "The original message was not modified and no replies or notifications were sent.")),
      );
      return;
    }

    const list = h("div", { class: "message-list" },
      output.messages.map((message) =>
        h("button", { type: "button", class: `message-pill${message.id === this.selected ? " active" : ""}`, onclick: () => { this.selected = message.id; this.renderMessages(); } },
          h("span", { class: "role" }, `#${message.id} ${roleLabel(this.roles, message.id)}`),
          h("span", { class: "subj" }, message.subject || "(no subject)"),
        ),
      ),
    );

    const message = output.messages.find((m) => m.id === this.selected) || output.messages[0];
    const canDiff = roleLabel(this.roles, message.id).includes("Delivered") || roleLabel(this.roles, message.id).includes("Redirected");
    if (this.mode === "diff" && !canDiff) this.mode = "preview";

    const setMode = (mode) => { this.mode = mode; this.renderMessages(); };
    const toolbar = h("div", { class: "message-toolbar" },
      h("div", { class: "segmented", role: "tablist" },
        h("button", { type: "button", class: this.mode === "preview" ? "active" : "", onclick: () => setMode("preview") }, "Preview"),
        h("button", { type: "button", class: this.mode === "source" ? "active" : "", onclick: () => setMode("source") }, "Source"),
        h("button", { type: "button", class: this.mode === "diff" ? "active" : "", disabled: !canDiff, title: canDiff ? "Compare with the input message" : "Only available for the delivered message", onclick: () => setMode("diff") }, "Changes"),
      ),
      h("span", { class: "toolbar-gap" }),
      h("span", { class: "item-sub" }, `${message.size.toLocaleString()} bytes`),
      h("button", { class: "btn ghost", type: "button", onclick: () => copyText(message.raw) }, "Copy"),
    );

    const frame = h("div", { class: `message-frame${this.mode === "preview" ? "" : " code"}` });
    this.messagesView.replaceChildren(list, toolbar, frame);

    if (this.mode === "preview") {
      frame.append(this.preview(message));
      this.pendingMount = null;
    } else {
      this.pendingMount = { frame, message, mode: this.mode };
      if (this.messagesView.classList.contains("active")) this.mountEditor();
    }
  }

  onShown() {
    if (this.pendingMount) this.mountEditor();
    this.layout();
  }

  mountEditor() {
    const { frame, message, mode } = this.pendingMount;
    this.pendingMount = null;
    if (!frame.isConnected) return;
    const text = message.raw.replace(/\r\n/g, "\n");
    if (mode === "source") {
      if (!this.sourceHost) {
        this.sourceHost = h("div", { class: "viewer-host" });
        this.sourceEditor = this.monaco.editor.create(this.sourceHost, viewerOptions());
      }
      frame.append(this.sourceHost);
      this.swapModels(() => this.sourceEditor.setModel(this.monaco.editor.createModel(text, EML_ID)));
      this.sourceEditor.layout();
    } else {
      if (!this.diffHost) {
        this.diffHost = h("div", { class: "viewer-host" });
        this.diffEditor = this.monaco.editor.createDiffEditor(this.diffHost, { ...viewerOptions(), enableSplitViewResizing: false });
      }
      frame.append(this.diffHost);
      this.diffEditor.updateOptions({ renderSideBySide: frame.clientWidth > 700 });
      this.swapModels(() =>
        this.diffEditor.setModel({
          original: this.monaco.editor.createModel(this.getOriginal().replace(/\r\n/g, "\n"), EML_ID),
          modified: this.monaco.editor.createModel(text, EML_ID),
        }),
      );
      this.diffEditor.layout();
    }
  }

  swapModels(attach) {
    const previous = this.viewerModels || [];
    attach();
    const source = this.sourceEditor?.getModel();
    const diff = this.diffEditor?.getModel();
    this.viewerModels = [source, diff?.original, diff?.modified].filter(Boolean);
    const keep = new Set(this.viewerModels);
    for (const model of previous) if (!keep.has(model)) model.dispose();
  }

  preview(message) {
    const container = h("div", { class: "preview" });
    const original = parseHeaderNames(this.getOriginal());
    const main = ["From", "To", "Cc", "Subject", "Date"];
    let showAll = false;
    const table = h("table", { class: "headers-table" });
    const drawHeaders = () => {
      const rows = message.headers
        .filter((header) => showAll || main.some((name) => name.toLowerCase() === header.name.toLowerCase()))
        .map((header) => {
          const isNew = !original.has(`${header.name.toLowerCase()}:${header.value}`);
          return h("tr", { class: isNew && message.id > 0 && roleLabel(this.roles, message.id).includes("Delivered") ? "changed" : "" }, h("th", {}, header.name), h("td", {}, header.value));
        });
      table.replaceChildren(...rows);
      toggle.textContent = showAll ? "Show fewer headers" : `Show all ${message.headers.length} headers`;
    };
    const toggle = h("button", { type: "button", class: "headers-toggle", onclick: () => { showAll = !showAll; drawHeaders(); } });
    drawHeaders();
    container.append(table, toggle);

    if (message.html && !message.text) {
      container.append(htmlFrame(message.html));
    } else if (message.html) {
      let showHtml = false;
      const body = h("div", {});
      const switcher = h("div", { class: "segmented" });
      const draw = () => {
        switcher.replaceChildren(
          h("button", { type: "button", class: showHtml ? "" : "active", onclick: () => { showHtml = false; draw(); } }, "Text"),
          h("button", { type: "button", class: showHtml ? "active" : "", onclick: () => { showHtml = true; draw(); } }, "HTML"),
        );
        body.replaceChildren(showHtml ? htmlFrame(message.html) : h("pre", { class: "body-text" }, message.text));
      };
      draw();
      container.append(h("div", { style: "margin-bottom:8px" }, switcher), body);
    } else {
      container.append(h("pre", { class: "body-text" }, message.text || "(empty body)"));
    }

    if (message.attachments.length) {
      container.append(
        h("div", { class: "attachments" },
          message.attachments.map((a) => h("span", { class: "attachment" }, a.name, h("span", { class: "muted" }, a.contentType), h("span", { class: "muted" }, formatSize(a.size)))),
        ),
      );
    }
    return container;
  }

  renderVariables() {
    const output = this.output;
    const nodes = [];
    if (output.globalVariables.length) {
      nodes.push(
        h("table", { class: "vars" },
          h("thead", {}, h("tr", {}, h("th", {}, "Global variable"), h("th", {}, "Value"))),
          h("tbody", {}, output.globalVariables.map((v) => h("tr", {}, h("td", {}, v.name), h("td", {}, v.value)))),
        ),
      );
    } else {
      nodes.push(h("div", { class: "vars-note" }, "No global variables were set. Declare a variable with ", h("code", {}, 'global ["name"];'), " (requires \"include\") to inspect its final value here."));
    }
    nodes.push(
      h("table", { class: "vars" },
        h("thead", {}, h("tr", {}, h("th", {}, "Duplicate and vacation ids seen"))),
        h("tbody", {}, output.duplicateIds.length ? output.duplicateIds.map((id) => h("tr", {}, h("td", {}, id))) : h("tr", {}, h("td", {}, "None"))),
      ),
      h("div", { class: "vars-note" }, `${output.instructions.toLocaleString()} instructions executed.`),
    );
    this.variablesView.replaceChildren(...nodes);
  }

  setTheme(theme) {
    this.monaco.editor.setTheme(theme);
  }

  layout() {
    this.sourceEditor?.layout();
    this.diffEditor?.layout();
  }

  disposeEditors() {
    this.pendingMount = null;
    this.sourceHost?.remove();
    this.diffHost?.remove();
  }
}

function viewerOptions() {
  return {
    readOnly: true,
    automaticLayout: true,
    minimap: { enabled: false },
    wordWrap: "on",
    scrollBeyondLastLine: false,
    lineNumbersMinChars: 3,
    fontSize: 12.5,
    renderLineHighlight: "none",
  };
}

function htmlFrame(html) {
  const csp = '<meta http-equiv="Content-Security-Policy" content="default-src \'none\'; img-src data:; style-src \'unsafe-inline\'">';
  return h("iframe", { class: "body-html", sandbox: "", referrerpolicy: "no-referrer", srcdoc: `${csp}${html}`, title: "HTML body" });
}

function parseHeaderNames(raw) {
  const set = new Set();
  const headerBlock = raw.replace(/\r\n/g, "\n").split(/\n\n/)[0];
  const unfolded = headerBlock.replace(/\n[ \t]+/g, " ");
  for (const line of unfolded.split("\n")) {
    const index = line.indexOf(":");
    if (index > 0) set.add(`${line.slice(0, index).trim().toLowerCase()}:${line.slice(index + 1).trim().split(/\s+/).join(" ")}`);
  }
  return set;
}

function formatSize(bytes) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

export async function copyText(text) {
  try {
    await navigator.clipboard.writeText(text);
    document.dispatchEvent(new CustomEvent("toast", { detail: "Copied to the clipboard" }));
  } catch (_) {
    document.dispatchEvent(new CustomEvent("toast", { detail: "Copy failed: clipboard access was blocked" }));
  }
}
