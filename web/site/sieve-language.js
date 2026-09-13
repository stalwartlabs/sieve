import { ACTIONS, CONTROL, DATE_PARTS, ENVIRONMENT_ITEMS, FUNCTIONS, TAGS, TESTS, lookup } from "./sieve-docs.js";

export const LANGUAGE_ID = "sieve";

const CAPABILITY_ERROR = /Undeclared capability '([^']+)'/;

function monarch() {
  return {
    ignoreCase: true,
    defaultToken: "",
    keywords: [...Object.keys(CONTROL), ...Object.keys(ACTIONS), "foreach"],
    tests: Object.keys(TESTS),
    tokenizer: {
      root: [
        [/#.*$/, "comment"],
        [/\/\*/, "comment", "@comment"],
        [/text:/, { token: "string.heredoc", next: "@heredoc" }],
        [/"/, { token: "string.quote", next: "@string" }],
        [/:[A-Za-z_][A-Za-z0-9_-]*/, "attribute.name"],
        [/\d+[KMGkmg]?\b/, "number"],
        [/[A-Za-z_][A-Za-z0-9_.-]*/, { cases: { "@keywords": "keyword", "@tests": "type", "@default": "identifier" } }],
        [/[{}()[\]]/, "@brackets"],
        [/[;,]/, "delimiter"],
        [/\s+/, "white"],
      ],
      comment: [
        [/[^/*]+/, "comment"],
        [/\*\//, "comment", "@pop"],
        [/[/*]/, "comment"],
      ],
      string: [
        [/\$\{[^}"]*\}/, "variable"],
        [/[^"\\$]+/, "string"],
        [/\\./, "string.escape"],
        [/\$/, "string"],
        [/"/, { token: "string.quote", next: "@pop" }],
      ],
      heredoc: [
        [/\.\s*$/, { token: "string.heredoc", next: "@pop" }],
        [/.*$/, "string"],
      ],
    },
  };
}

const configuration = {
  comments: { lineComment: "#", blockComment: ["/*", "*/"] },
  brackets: [["{", "}"], ["[", "]"], ["(", ")"]],
  autoClosingPairs: [
    { open: "{", close: "}" },
    { open: "[", close: "]" },
    { open: "(", close: ")" },
    { open: '"', close: '"', notIn: ["string", "comment"] },
    { open: "/*", close: " */", notIn: ["string"] },
  ],
  surroundingPairs: [
    { open: "{", close: "}" },
    { open: "[", close: "]" },
    { open: "(", close: ")" },
    { open: '"', close: '"' },
  ],
  indentationRules: {
    increaseIndentPattern: /\{[^}"]*$/,
    decreaseIndentPattern: /^\s*\}/,
  },
  folding: { markers: { start: /\{\s*$/, end: /^\s*\}/ } },
  wordPattern: /(-?\d*\.\d\w*)|([^\s`~!@#%^&*()=+[{\]}\\|;:'",<>/?]+)/g,
};

function docMarkdown(word, entry) {
  const parts = [`\`\`\`sieve\n${entry.sig}\n\`\`\``, entry.doc];
  const links = [];
  if (entry.capability) links.push(`Requires \`"${entry.capability}"\``);
  if (entry.rfc) links.push(`[Specification](${entry.rfc})`);
  if (links.length) parts.push(links.join(" · "));
  return parts.join("\n\n");
}

function textBefore(model, position, maxLines = 40) {
  const startLine = Math.max(1, position.lineNumber - maxLines);
  return model.getValueInRange({
    startLineNumber: startLine,
    startColumn: 1,
    endLineNumber: position.lineNumber,
    endColumn: position.column,
  });
}

function stripComments(text) {
  return text.replace(/\/\*[\s\S]*?\*\//g, "").replace(/#.*$/gm, "");
}

function context(model, position) {
  const before = stripComments(textBefore(model, position));
  const statement = before.slice(Math.max(before.lastIndexOf(";"), before.lastIndexOf("{"), before.lastIndexOf("}")) + 1);
  const quotes = (statement.match(/(?<!\\)"/g) || []).length;
  const inString = quotes % 2 === 1;
  const command = (statement.match(/^\s*([A-Za-z_]+)/) || [])[1]?.toLowerCase() || "";
  const line = model.getLineContent(position.lineNumber).slice(0, position.column - 1);
  return { statement, inString, command, line };
}

function replaceRange(model, position) {
  const word = model.getWordUntilPosition(position);
  return {
    startLineNumber: position.lineNumber,
    endLineNumber: position.lineNumber,
    startColumn: word.startColumn,
    endColumn: word.endColumn,
  };
}

export function registerSieve(monaco, getSettings) {
  monaco.languages.register({ id: LANGUAGE_ID, extensions: [".sieve", ".siv"], aliases: ["Sieve"] });
  monaco.languages.setMonarchTokensProvider(LANGUAGE_ID, monarch());
  monaco.languages.setLanguageConfiguration(LANGUAGE_ID, configuration);

  const { CompletionItemKind: Kind, CompletionItemInsertTextRule: Rule } = monaco.languages;

  monaco.languages.registerCompletionItemProvider(LANGUAGE_ID, {
    triggerCharacters: [":", '"', "$", "{"],
    provideCompletionItems(model, position) {
      const ctx = context(model, position);
      const range = replaceRange(model, position);
      const settings = getSettings();
      const items = [];

      if (ctx.inString) {
        if (ctx.command === "require" || /\bihave\b/.test(ctx.statement)) {
          for (const capability of settings.capabilities || []) {
            items.push({ label: capability, kind: Kind.Module, insertText: capability, range });
          }
        } else if (/^\s*(fileinto|mailboxexists)\b/.test(ctx.statement) && !/:(flags|specialuse|mailboxid)\s+"[^"]*$/.test(ctx.statement)) {
          for (const mailbox of settings.mailboxes || []) {
            items.push({ label: mailbox.name, kind: Kind.Folder, insertText: mailbox.name, detail: mailbox.specialUse?.join(" "), range });
          }
        } else if (/\benvironment\b/.test(ctx.statement)) {
          const names = new Set([...ENVIRONMENT_ITEMS, ...(settings.environment || []).map((item) => item.name)]);
          for (const name of names) items.push({ label: name, kind: Kind.Constant, insertText: name, range });
        } else if (/\b(currentdate|date)\b/.test(ctx.statement)) {
          for (const part of DATE_PARTS) items.push({ label: part, kind: Kind.EnumMember, insertText: part, range });
        } else if (/\b(let|eval|while)\b/.test(ctx.statement)) {
          for (const [name, arity] of Object.entries(FUNCTIONS)) {
            const args = Array.from({ length: arity }, (_, i) => `\${${i + 1}}`).join(", ");
            items.push({ label: name, kind: Kind.Function, detail: `${arity} argument${arity === 1 ? "" : "s"}`, insertText: `${name}(${args})`, insertTextRules: Rule.InsertAsSnippet, range });
          }
          for (const name of ["header.subject", "header.from.addr", "header.from.name", "header.to[*].addr[*]", "envelope.from", "envelope.to", "env.remote_ip"]) {
            items.push({ label: name, kind: Kind.Variable, insertText: name, range });
          }
        } else if (/\b(addflag|setflag|removeflag|hasflag)\b|:flags\s+/.test(ctx.statement)) {
          for (const flag of ["\\\\Seen", "\\\\Flagged", "\\\\Answered", "\\\\Deleted", "\\\\Draft", "$Junk", "$NotJunk"]) {
            items.push({ label: flag, kind: Kind.EnumMember, insertText: flag, range });
          }
        }
        return { suggestions: items };
      }

      if (/:[A-Za-z_-]*$/.test(ctx.line)) {
        for (const [tag, doc] of Object.entries(TAGS)) {
          items.push({ label: `:${tag}`, kind: Kind.Property, insertText: tag, documentation: { value: doc }, range });
        }
        return { suggestions: items };
      }

      const snippets = {
        if: "if ${1:header :contains \"Subject\" \"${2:text}\"} {\n\t$0\n}",
        require: "require [\"${1:fileinto}\"];",
        fileinto: "fileinto \"${1:INBOX}\";",
        redirect: "redirect \"${1:user@example.org}\";",
        vacation: "vacation :days ${1:7} :subject \"${2:Out of office}\" text:\n${3:I am away.}\n.\n;",
        foreverypart: "foreverypart {\n\t$0\n}",
        while: "while \"${1:i < 10}\" {\n\t$0\n}",
        set: "set \"${1:name}\" \"${2:value}\";",
        let: "let \"${1:name}\" \"${2:expression}\";",
      };
      for (const [group, kind] of [[CONTROL, Kind.Keyword], [ACTIONS, Kind.Function], [TESTS, Kind.Interface]]) {
        for (const [name, entry] of Object.entries(group)) {
          items.push({
            label: name,
            kind,
            detail: entry.sig,
            documentation: { value: docMarkdown(name, entry) },
            insertText: snippets[name] || name,
            insertTextRules: snippets[name] ? Rule.InsertAsSnippet : undefined,
            range,
          });
        }
      }
      return { suggestions: items };
    },
  });

  monaco.languages.registerHoverProvider(LANGUAGE_ID, {
    provideHover(model, position) {
      const word = model.getWordAtPosition(position);
      if (!word) return null;
      const line = model.getLineContent(position.lineNumber);
      const range = new monaco.Range(position.lineNumber, word.startColumn, position.lineNumber, word.endColumn);
      if (line[word.startColumn - 2] === ":") {
        const doc = TAGS[word.word.toLowerCase()];
        if (!doc) return null;
        return { range, contents: [{ value: `**:${word.word}**` }, { value: doc }] };
      }
      const token = monaco.editor.tokenize(line, LANGUAGE_ID)[0]?.filter((t) => t.offset <= word.startColumn - 1).pop();
      if (token && /string|comment/.test(token.type)) return null;
      const entry = lookup(word.word);
      if (!entry) return null;
      return { range, contents: [{ value: docMarkdown(word.word, entry) }] };
    },
  });

  monaco.languages.registerCodeActionProvider(LANGUAGE_ID, {
    provideCodeActions(model, _range, ctx) {
      const actions = [];
      for (const marker of ctx.markers) {
        const match = CAPABILITY_ERROR.exec(marker.message);
        if (!match) continue;
        const edit = requireEdit(monaco, model, match[1]);
        if (!edit) continue;
        actions.push({
          title: `Add "${match[1]}" to require`,
          kind: "quickfix",
          diagnostics: [marker],
          isPreferred: true,
          edit: { edits: [{ resource: model.uri, textEdit: edit, versionId: model.getVersionId() }] },
        });
      }
      return { actions, dispose() {} };
    },
  });

  monaco.languages.registerDocumentFormattingEditProvider(LANGUAGE_ID, {
    provideDocumentFormattingEdits(model, options) {
      const unit = options.insertSpaces ? " ".repeat(options.tabSize) : "\t";
      let depth = 0;
      let inHeredoc = false;
      let inComment = false;
      const lines = model.getLinesContent().map((raw) => {
        if (inHeredoc) {
          if (/^\.\s*$/.test(raw)) inHeredoc = false;
          return raw;
        }
        const trimmed = raw.trim();
        const code = stripComments(trimmed.replace(/"(?:[^"\\]|\\.)*"/g, '""'));
        if (inComment) {
          if (trimmed.includes("*/")) inComment = false;
          return raw;
        }
        const opens = (code.match(/\{/g) || []).length;
        const closes = (code.match(/\}/g) || []).length;
        const leadingCloses = (trimmed.match(/^\}+/) || [""])[0].length;
        const indent = Math.max(0, depth - leadingCloses);
        depth = Math.max(0, depth + opens - closes);
        if (/text:\s*(#.*)?$/.test(code)) inHeredoc = true;
        if (trimmed.startsWith("/*") && !trimmed.includes("*/")) inComment = true;
        return trimmed === "" ? "" : unit.repeat(indent) + trimmed;
      });
      return [{ range: model.getFullModelRange(), text: lines.join(model.getEOL()) }];
    },
  });
}

export function requireEdit(monaco, model, capability) {
  const text = model.getValue();
  const quoted = `"${capability}"`;
  const listMatch = /require\s*\[([^\]]*)\]\s*;/i.exec(text);
  if (listMatch) {
    const closeOffset = listMatch.index + listMatch[0].lastIndexOf("]");
    const inner = listMatch[1].trim();
    const pos = model.getPositionAt(closeOffset);
    return { range: new monaco.Range(pos.lineNumber, pos.column, pos.lineNumber, pos.column), text: inner ? `, ${quoted}` : quoted };
  }
  const singleMatch = /require\s*("[^"]*")\s*;/i.exec(text);
  if (singleMatch) {
    const start = model.getPositionAt(singleMatch.index);
    const end = model.getPositionAt(singleMatch.index + singleMatch[0].length);
    return { range: new monaco.Range(start.lineNumber, start.column, end.lineNumber, end.column), text: `require [${singleMatch[1]}, ${quoted}];` };
  }
  let line = 1;
  const lines = model.getLinesContent();
  while (line <= lines.length && /^\s*(#.*)?$/.test(lines[line - 1])) line++;
  return { range: new monaco.Range(line, 1, line, 1), text: `require [${quoted}];\n\n` };
}

export function markersFor(monaco, model, diagnostics) {
  return diagnostics.map((diagnostic) => {
    const known = diagnostic.line > 0 && diagnostic.line <= model.getLineCount();
    const line = known ? diagnostic.line : 1;
    const maxColumn = model.getLineMaxColumn(line);
    const startColumn = known ? Math.min(Math.max(diagnostic.column, 1), maxColumn) : 1;
    return {
      severity: diagnostic.severity === "warning" ? monaco.MarkerSeverity.Warning : monaco.MarkerSeverity.Error,
      message: known ? diagnostic.message : `${diagnostic.message} (position unknown)`,
      startLineNumber: line,
      endLineNumber: line,
      startColumn: startColumn === maxColumn ? Math.max(1, startColumn - 1) : startColumn,
      endColumn: maxColumn,
      source: "sieve",
    };
  });
}
