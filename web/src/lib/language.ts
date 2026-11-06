// SPDX-License-Identifier: AGPL-3.0-or-later
// Highlighting for .vk files: keywords, types, strings (Lean formulas set apart), numbers, dates, comments – with
// delimiters or by indentation: after `lean`, `source` and `label`/`text`/`criteria` and a language, the rest of the
// line – or the lines indented deeper below – is text, as the lexer reads it (crates/verkodo-syntax/src/lexer.rs).
import { HighlightStyle, StreamLanguage, syntaxHighlighting, type StringStream } from "@codemirror/language";
import { EditorView } from "@codemirror/view";
import { tags } from "@lezer/highlight";

const KEYWORDS = new Set(
  (
    "module concept variant bridge theorem example remark path step label source params type lean statement proof " +
    "using about text norm valid from to replaces input judgement criteria constant rule obligation permission " +
    "prohibition default when unless then if else and or not"
  ).split(" "),
);
const TYPES = new Set(["bool", "money", "int", "decimal", "date"]);
const DECLARING = new Set([
  "concept", "variant", "theorem", "example", "remark", "path", "norm", "input", "judgement", "constant", "rule",
  "obligation", "permission", "prohibition",
]);

interface State {
  /** The word before: `lean` makes the next string a formula, a declaring keyword the next name a definition. */
  last: string;
  /** Inside a block of Lean code between fences, which may span lines. */
  block: boolean;
  /** Text without quotes follows: its style, and the indentation of the line that announced it. */
  pending: { style: string; indent: number } | null;
  /** Inside text without quotes on the lines below: its style and the indentation it must stay deeper than. */
  raw: { style: string; indent: number } | null;
  /** Tokens on the current line so far, and the line's indentation. */
  onLine: string[];
  indent: number;
}

const LEAN = "special(string)";

const FENCE = "```";

/** Reads Lean code up to the closing fence or the end of the line; leaves the block when it finds the fence. */
function block(stream: StringStream, state: State): string {
  const rest = stream.string.slice(stream.pos);
  const end = rest.indexOf(FENCE);
  if (end < 0) {
    stream.skipToEnd();
  } else {
    stream.pos += end + FENCE.length;
    state.block = false;
  }
  return "special(string)";
}

function string(stream: StringStream): void {
  let escaped = false;
  let c: string | void;
  while ((c = stream.next()) !== undefined) {
    if (c === '"' && !escaped) return;
    escaped = !escaped && c === "\\";
  }
}

export const verkodo = StreamLanguage.define<State>({
  name: "verkodo",
  startState: () => ({ last: "", block: false, pending: null, raw: null, onLine: [], indent: 0 }),
  token(stream, state) {
    if (state.block) return block(stream, state);
    if (stream.sol()) {
      state.onLine = [];
      state.indent = stream.indentation();
      // Text announced at the end of the line above: the lines indented deeper.
      if (state.pending) {
        state.raw = state.pending;
        state.pending = null;
      }
      if (state.raw && !stream.match(/^\s*$/, false) && stream.indentation() <= state.raw.indent) state.raw = null;
    }
    if (state.raw) {
      stream.skipToEnd();
      return state.raw.style;
    }
    if (stream.eatSpace()) return null;
    if (state.pending) {
      const style = state.pending.style;
      state.pending = null;
      if (stream.peek() !== '"' && !stream.match(FENCE, false) && !stream.match("//", false)) {
        stream.skipToEnd();
        return style;
      }
    }
    if (stream.match(FENCE)) {
      state.block = true;
      state.last = "";
      return block(stream, state);
    }
    if (stream.match("//")) {
      stream.skipToEnd();
      return "comment";
    }
    if (stream.peek() === '"') {
      stream.next();
      string(stream);
      const formula = state.last === "lean";
      state.last = "";
      return formula ? "special(string)" : "string";
    }
    if (stream.match(/^\d{4}-\d{2}-\d{2}/)) return "number";
    if (stream.match(/^\d[\d_]*(\.\d+)?/)) return "number";
    if (stream.match("<->") || stream.match(/^(<=|>=|==|!=|[<>=+\-*\/:,.])/)) return "operator";
    if (stream.match(/^[\p{L}_][\p{L}\p{N}_]*/u)) {
      const word = stream.current();
      const before = state.last;
      state.last = word;
      const first = state.onLine.length === 0;
      state.onLine.push(word);
      // What announces text without quotes.
      if (word === "lean") state.pending = { style: LEAN, indent: state.indent };
      else if (word === "source" && first) state.pending = { style: "string", indent: state.indent };
      else if (state.onLine.length === 2 && ["label", "text", "criteria"].includes(state.onLine[0]!))
        state.pending = { style: "string", indent: state.indent };
      if (KEYWORDS.has(word)) return "keyword";
      if (TYPES.has(word)) return "typeName";
      if (word === "true" || word === "false") return "bool";
      if (word === "EUR") return "unit";
      if (DECLARING.has(before)) return "definition(variableName)";
      return "variableName";
    }
    stream.next();
    return null;
  },
  languageData: { commentTokens: { line: "//" } },
});

// Colours from the app's variables (app.css), so that light and dark both read well.
export const theme = EditorView.theme({
  "&": { backgroundColor: "var(--bg)", color: "var(--fg)" },
  ".cm-content": { caretColor: "var(--fg)" },
  ".cm-cursor": { borderLeftColor: "var(--fg)" },
  ".cm-gutters": { backgroundColor: "var(--bg)", color: "var(--accent)", borderRight: "1px solid var(--border)" },
  ".cm-activeLine, .cm-activeLineGutter": { backgroundColor: "var(--vk-active)" },
  "&.cm-focused .cm-selectionBackground, .cm-selectionBackground, ::selection": {
    backgroundColor: "var(--vk-selection) !important",
  },
  ".cm-tooltip": { backgroundColor: "var(--bg)", color: "var(--fg)", border: "1px solid var(--border)" },
  ".cm-tooltip-lint": { padding: "0" },
});

export const highlighting = syntaxHighlighting(
  HighlightStyle.define([
    { tag: tags.keyword, color: "var(--vk-keyword)", fontWeight: "600" },
    { tag: tags.typeName, color: "var(--vk-type)" },
    { tag: tags.string, color: "var(--vk-string)" },
    { tag: tags.special(tags.string), color: "var(--vk-formula)" },
    { tag: [tags.number, tags.bool, tags.unit], color: "var(--vk-number)" },
    { tag: tags.comment, color: "var(--accent)", fontStyle: "italic" },
    { tag: tags.definition(tags.variableName), color: "var(--vk-definition)", fontWeight: "600" },
    { tag: tags.operator, color: "var(--accent)" },
  ]),
);
