import { h } from "./result.js";

const list = (text) => text.split(/[,\n]/).map((s) => s.trim()).filter(Boolean);
const lines = (text) => text.split("\n").map((s) => s.trim()).filter((s) => s && !s.startsWith("#"));

function pairs(text) {
  return lines(text).map((line) => {
    const index = line.indexOf("=");
    return index < 0 ? { name: line, value: "" } : { name: line.slice(0, index).trim(), value: line.slice(index + 1).trim() };
  });
}

const FORMATS = {
  text: { read: (v) => v ?? "", write: (v) => v },
  list: { read: (v) => (v || []).join(", "), write: list },
  number: { read: (v) => String(v ?? ""), write: (v) => Math.max(0, Number.parseInt(v, 10) || 0) },
  bool: { read: (v) => !!v, write: (v) => !!v },
  pairs: { read: (v) => (v || []).map((p) => `${p.name} = ${p.value}`).join("\n"), write: pairs },
  mailboxes: {
    read: (v) => (v || []).map((m) => [m.name, ...(m.specialUse || [])].join(" ")).join("\n"),
    write: (text) => lines(text).map((line) => {
      const parts = line.split(/\s+(?=\\)/);
      return { name: parts[0], specialUse: parts.slice(1) };
    }),
  },
  lists: {
    read: (v) => (v || []).map((l) => `${l.name} = ${l.values.join(", ")}`).join("\n"),
    write: (text) => pairs(text).map((p) => ({ name: p.name, values: list(p.value) })),
  },
  metadata: {
    read: (v) => (v || []).map((m) => `${m.mailbox ? `${m.mailbox} ` : ""}${m.annotation} = ${m.value}`).join("\n"),
    write: (text) => pairs(text).map((p) => {
      const slash = p.name.indexOf("/");
      return slash > 0 ? { mailbox: p.name.slice(0, slash).trim(), annotation: p.name.slice(slash).trim(), value: p.value } : { mailbox: "", annotation: p.name, value: p.value };
    }),
  },
  time: {
    read: (v) => {
      if (v === null || v === undefined) return "";
      const date = new Date(v * 1000);
      return new Date(date.getTime() - date.getTimezoneOffset() * 60000).toISOString().slice(0, 16);
    },
    write: (v) => (v ? Math.floor(new Date(v).getTime() / 1000) : null),
  },
};

const SPAM_LABELS = ["0: not scanned", "1: not spam", "2", "3", "4", "5: maybe spam", "6", "7", "8", "9", "10: definitely spam"];
const VIRUS_LABELS = ["0: not scanned", "1: clean", "2: virus replaced", "3: virus cured", "4: possibly infected", "5: infected"];

const GROUPS = [
  {
    title: "Identity and envelope",
    sub: "who receives the message",
    open: true,
    fields: [
      { key: "userAddress", label: "User address", format: "text" },
      { key: "userFullName", label: "User full name", format: "text" },
      { row: [
        { key: "envelopeFrom", label: "Envelope from", format: "text", placeholder: "taken from the message" },
        { key: "envelopeTo", label: "Envelope to", format: "list", placeholder: "the user address" },
      ] },
      { row: [
        { key: "envelopeId", label: "Envelope id", format: "text" },
        { key: "localHostname", label: "Local hostname", format: "text" },
      ] },
      { key: "currentTime", label: "Current time", format: "time", type: "datetime-local", help: "Leave empty to use the real clock. Affects currentdate and Date headers." },
    ],
  },
  {
    title: "Mailboxes",
    sub: "for fileinto, mailboxexists and special-use",
    fields: [{ key: "mailboxes", label: "One mailbox per line, followed by its special-use attributes", format: "mailboxes", textarea: 7 }],
  },
  {
    title: "Spam and virus",
    sub: "spamtest and virustest",
    fields: [{ row: [
      { key: "spamScore", label: "Spam score", format: "number", options: SPAM_LABELS },
      { key: "virusScore", label: "Virus status", format: "number", options: VIRUS_LABELS },
    ] }],
  },
  {
    title: "Environment and variables",
    sub: "environment, global variables, metadata",
    fields: [
      { key: "environment", label: "Environment items (name = value)", format: "pairs", textarea: 6 },
      { key: "globalVariables", label: "Initial global variables (name = value)", format: "pairs", textarea: 3 },
      { key: "metadata", label: "Metadata: [mailbox] /annotation = value", format: "metadata", textarea: 3, help: "Without a mailbox the annotation is a server annotation." },
    ],
  },
  {
    title: "Lists and notifications",
    sub: "extlists, enotify, vacation",
    fields: [
      { key: "lists", label: "External lists (list = value, value)", format: "lists", textarea: 3 },
      { key: "validNotificationUris", label: "Allowed notification URI schemes", format: "list" },
      { row: [
        { key: "vacationDefaultSubject", label: "Default vacation subject", format: "text" },
        { key: "vacationSubjectPrefix", label: "Vacation subject prefix", format: "text" },
      ] },
      { row: [
        { key: "defaultVacationExpiry", label: "Default vacation period (s)", format: "number" },
        { key: "defaultDuplicateExpiry", label: "Default duplicate period (s)", format: "number" },
      ] },
      { key: "vacationUseOrigRcpt", label: "Vacation uses the original recipient", format: "bool" },
      { key: "protectedHeaders", label: "Protected headers (editheader cannot change them)", format: "list" },
    ],
  },
  { title: "Extensions", sub: "capabilities allowed at run time", capabilities: true },
  {
    title: "Runtime limits",
    sub: "per run",
    fields: [
      { row: [{ key: "cpuLimit", label: "Max instructions", format: "number" }, { key: "memoryLimit", label: "Memory limit (bytes)", format: "number" }] },
      { row: [{ key: "maxRedirects", label: "Max redirects", format: "number" }, { key: "maxOutMessages", label: "Max outgoing messages", format: "number" }] },
      { row: [{ key: "maxNestedIncludes", label: "Max nested includes", format: "number" }, { key: "maxVariableSize", label: "Max variable size", format: "number" }] },
      { row: [{ key: "maxHeaderSize", label: "Max header size", format: "number" }, { key: "maxReceivedHeaders", label: "Max Received headers", format: "number" }] },
    ],
  },
  {
    title: "Compiler limits",
    sub: "applied when the script is compiled",
    fields: [
      { row: [{ key: "maxScriptSize", label: "Max script size", format: "number" }, { key: "maxStringSize", label: "Max string size", format: "number" }] },
      { row: [{ key: "maxNestedBlocks", label: "Max nested blocks", format: "number" }, { key: "maxNestedTests", label: "Max nested tests", format: "number" }] },
      { row: [{ key: "maxNestedForeverypart", label: "Max nested foreverypart", format: "number" }, { key: "maxMatchVariables", label: "Max match variables", format: "number" }] },
      { row: [{ key: "maxLocalVariables", label: "Max local variables", format: "number" }, { key: "maxVariableNameSize", label: "Max variable name size", format: "number" }] },
      { row: [{ key: "maxIncludes", label: "Max includes", format: "number" }] },
    ],
  },
];

export class SettingsDrawer {
  constructor({ drawer, backdrop, body, resetButton, jsonButton, defaults, capabilities, get, set }) {
    Object.assign(this, { drawer, backdrop, body, resetButton, jsonButton, defaults, capabilities, get, set });
    this.jsonMode = false;
    this.openGroups = new Set(GROUPS.filter((g) => g.open).map((g) => g.title));
    for (const el of drawer.querySelectorAll("[data-close]")) el.addEventListener("click", () => this.close());
    backdrop.addEventListener("click", () => this.close());
    drawer.addEventListener("keydown", (event) => {
      if (event.key === "Escape") this.close();
    });
    resetButton.addEventListener("click", () => {
      if (!confirm("Reset all settings of this workspace to their defaults?")) return;
      this.set(structuredClone(this.defaults));
      this.render();
    });
    jsonButton.addEventListener("click", () => {
      this.jsonMode = !this.jsonMode;
      this.render();
    });
  }

  open() {
    this.jsonMode = false;
    this.render();
    this.drawer.hidden = false;
    this.backdrop.hidden = false;
    this.drawer.querySelector("input, textarea, select, button")?.focus();
  }

  close() {
    if (this.jsonMode && !this.applyJson()) return;
    this.drawer.hidden = true;
    this.backdrop.hidden = true;
  }

  update(key, value) {
    this.set({ ...this.get(), [key]: value });
  }

  render() {
    this.jsonButton.textContent = this.jsonMode ? "Back to form" : "Edit as JSON";
    if (this.jsonMode) {
      const area = h("textarea", { class: "json-editor", spellcheck: "false", rows: 30, style: "min-height:60vh;margin-top:12px" });
      area.value = JSON.stringify(this.get(), null, 2);
      const error = h("p", { class: "dialog-warning", hidden: true });
      area.addEventListener("change", () => this.applyJson());
      this.jsonArea = area;
      this.jsonError = error;
      this.body.replaceChildren(area, error);
      return;
    }
    const settings = this.get();
    this.body.replaceChildren(
      ...GROUPS.map((group) => {
        const details = h("details", { class: "settings-group", open: this.openGroups.has(group.title) },
          h("summary", {}, group.title, h("span", { class: "item-sub" }, group.sub)),
          h("div", { class: "settings-fields" }, group.capabilities ? this.capabilitiesField(settings) : group.fields.map((field) => this.field(field, settings))),
        );
        details.addEventListener("toggle", () => {
          if (details.open) this.openGroups.add(group.title);
          else this.openGroups.delete(group.title);
        });
        return details;
      }),
    );
  }

  applyJson() {
    try {
      const parsed = JSON.parse(this.jsonArea.value);
      if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) throw new Error("Settings must be a JSON object");
      this.set({ ...this.defaults, ...parsed });
      this.jsonError.hidden = true;
      return true;
    } catch (err) {
      this.jsonError.textContent = `Invalid JSON: ${err.message}`;
      this.jsonError.hidden = false;
      return false;
    }
  }

  field(spec, settings) {
    if (spec.row) return h("div", { class: "field-row" }, spec.row.map((item) => this.field(item, settings)));
    const format = FORMATS[spec.format];
    const value = format.read(settings[spec.key]);
    const id = `setting-${spec.key}`;
    if (spec.format === "bool") {
      const input = h("input", { type: "checkbox", id });
      input.checked = value;
      input.addEventListener("change", () => this.update(spec.key, input.checked));
      return h("label", { class: "check" }, input, spec.label);
    }
    let input;
    if (spec.options) {
      input = h("select", { id }, spec.options.map((label, index) => h("option", { value: index }, label)));
      input.value = value || "0";
    } else if (spec.textarea) {
      input = h("textarea", { id, rows: spec.textarea, spellcheck: "false" });
      input.value = value;
    } else {
      input = h("input", { id, type: spec.type || (spec.format === "number" ? "number" : "text"), spellcheck: "false", autocomplete: "off", placeholder: spec.placeholder, min: spec.format === "number" ? 0 : undefined });
      input.value = value;
    }
    input.addEventListener("change", () => this.update(spec.key, format.write(input.value)));
    return h("label", { class: "field", for: id }, h("span", {}, spec.label), input, spec.help ? h("small", {}, spec.help) : null);
  }

  capabilitiesField(settings) {
    const enabled = new Set(settings.capabilities || []);
    const boxes = this.capabilities.map((name) => {
      const input = h("input", { type: "checkbox", value: name });
      input.checked = enabled.has(name);
      input.addEventListener("change", () => {
        const next = new Set(this.get().capabilities || []);
        if (input.checked) next.add(name);
        else next.delete(name);
        this.update("capabilities", this.capabilities.filter((c) => next.has(c)));
      });
      return h("label", { class: "check" }, input, name);
    });
    const setAll = (on) => {
      this.update("capabilities", on ? [...this.capabilities] : []);
      this.render();
    };
    const noCheck = h("input", { type: "checkbox" });
    noCheck.checked = !!settings.noCapabilityCheck;
    noCheck.addEventListener("change", () => this.update("noCapabilityCheck", noCheck.checked));
    return [
      h("div", { class: "caps-actions" },
        h("button", { type: "button", onclick: () => setAll(true) }, "Enable all"),
        h("button", { type: "button", onclick: () => setAll(false) }, "Disable all"),
      ),
      h("div", { class: "caps" }, boxes),
      h("label", { class: "check" }, noCheck, "Allow commands without a matching require (not portable)"),
    ];
  }
}
