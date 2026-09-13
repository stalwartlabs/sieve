export const EML_ID = "eml";

class EmlState {
  constructor(inHeaders, boundaries) {
    this.inHeaders = inHeaders;
    this.boundaries = boundaries;
  }

  clone() {
    return new EmlState(this.inHeaders, this.boundaries.slice());
  }

  equals(other) {
    return (
      other instanceof EmlState &&
      other.inHeaders === this.inHeaders &&
      other.boundaries.length === this.boundaries.length &&
      other.boundaries.every((b, i) => b === this.boundaries[i])
    );
  }
}

const HEADER = /^([A-Za-z0-9!#$%&'*+.^_`|~-]+)(\s*:)/;
const BOUNDARY_PARAM = /boundary\s*=\s*"?([^";\s]+)"?/i;

export function registerEml(monaco) {
  monaco.languages.register({ id: EML_ID, extensions: [".eml"], aliases: ["Email"] });
  monaco.languages.setTokensProvider(EML_ID, {
    getInitialState: () => new EmlState(true, []),
    tokenize(line, state) {
      const next = state.clone();
      const tokens = [];

      if (next.boundaries.length) {
        const trimmed = line.trimEnd();
        const hit = next.boundaries.find((b) => trimmed === `--${b}` || trimmed === `--${b}--`);
        if (hit) {
          tokens.push({ startIndex: 0, scopes: "keyword.boundary" });
          if (trimmed.endsWith("--") && trimmed === `--${hit}--`) {
            next.boundaries = next.boundaries.filter((b) => b !== hit);
            next.inHeaders = false;
          } else {
            next.inHeaders = true;
          }
          return { tokens, endState: next };
        }
      }

      if (next.inHeaders) {
        if (line.trim() === "") {
          next.inHeaders = false;
          return { tokens: [{ startIndex: 0, scopes: "" }], endState: next };
        }
        const boundary = BOUNDARY_PARAM.exec(line);
        if (boundary && !next.boundaries.includes(boundary[1])) next.boundaries.push(boundary[1]);
        const header = HEADER.exec(line);
        if (header) {
          tokens.push({ startIndex: 0, scopes: "attribute.name.header" });
          tokens.push({ startIndex: header[1].length, scopes: "delimiter" });
          tokens.push({ startIndex: header[0].length, scopes: "string.header" });
        } else if (/^\s/.test(line)) {
          tokens.push({ startIndex: 0, scopes: "string.header" });
        } else {
          tokens.push({ startIndex: 0, scopes: "invalid" });
        }
        return { tokens, endState: next };
      }

      tokens.push({ startIndex: 0, scopes: /^>/.test(line) ? "comment.quote" : "" });
      return { tokens, endState: next };
    },
  });
}

export function defineThemes(monaco) {
  const rules = (dark) => [
    { token: "keyword", foreground: dark ? "c792ea" : "7a3eb1", fontStyle: "bold" },
    { token: "type", foreground: dark ? "82aaff" : "1f5fbf" },
    { token: "attribute.name", foreground: dark ? "f78c6c" : "b35300" },
    { token: "variable", foreground: dark ? "ffcb6b" : "a0680a", fontStyle: "bold" },
    { token: "string", foreground: dark ? "c3e88d" : "2f7d32" },
    { token: "string.heredoc", foreground: dark ? "89ddff" : "0a7f94", fontStyle: "bold" },
    { token: "string.escape", foreground: dark ? "89ddff" : "0a7f94" },
    { token: "number", foreground: dark ? "f78c6c" : "b35300" },
    { token: "comment", foreground: dark ? "6a7390" : "7d8595", fontStyle: "italic" },
    { token: "attribute.name.header", foreground: dark ? "82aaff" : "1f5fbf", fontStyle: "bold" },
    { token: "string.header", foreground: dark ? "d7dae0" : "3a3f4b" },
    { token: "keyword.boundary", foreground: dark ? "c792ea" : "7a3eb1" },
    { token: "comment.quote", foreground: dark ? "6a7390" : "7d8595" },
    { token: "invalid", foreground: dark ? "ff7b82" : "c6313a" },
  ];
  monaco.editor.defineTheme("sievepad-light", {
    base: "vs",
    inherit: true,
    rules: rules(false),
    colors: { "editor.background": "#ffffff", "editorGutter.background": "#ffffff" },
  });
  monaco.editor.defineTheme("sievepad-dark", {
    base: "vs-dark",
    inherit: true,
    rules: rules(true),
    colors: { "editor.background": "#1b1d24", "editorGutter.background": "#1b1d24" },
  });
}
