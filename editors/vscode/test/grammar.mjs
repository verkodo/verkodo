// SPDX-License-Identifier: AGPL-3.0-or-later
// Tokenizes .vk text with the grammar as VS Code does (vscode-textmate, Oniguruma) and checks the scopes of a few
// tokens: `node test/grammar.mjs`.
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import vsctm from "vscode-textmate";
import oniguruma from "vscode-oniguruma";

const require = createRequire(import.meta.url);
const wasm = readFileSync(require.resolve("vscode-oniguruma/release/onig.wasm")).buffer;
await oniguruma.loadWASM(wasm);
const registry = new vsctm.Registry({
  onigLib: Promise.resolve({
    createOnigScanner: (p) => new oniguruma.OnigScanner(p),
    createOnigString: (s) => new oniguruma.OnigString(s),
  }),
  loadGrammar: async (scope) =>
    scope === "source.verkodo"
      ? vsctm.parseRawGrammar(readFileSync(new URL("../syntaxes/verkodo.tmLanguage.json", import.meta.url), "utf8"), "x.json")
      : null,
});
const grammar = await registry.loadGrammar("source.verkodo");

const text = `module de.ustg.p19 original de, en
// a comment
enum residence { value domestic }
norm n {
  label de Kleinunternehmer
  source "§ 19 UStG"
  input y : bool { label de "Umsatz" }
  valid from 2025-01-01
  input x : money
  rule r : decimal { by x up to 25_000 EUR: 19 % otherwise: 0 }
}
theorem t { statement lean "∀ x, x = x" proof lean \`\`\`
  by simp
\`\`\` }`;

const scopes = new Map();
let state = vsctm.INITIAL;
for (const line of text.split("\n")) {
  const r = grammar.tokenizeLine(line, state);
  for (const t of r.tokens) {
    const word = line.slice(t.startIndex, t.endIndex).trim();
    if (word && !scopes.has(word)) scopes.set(word, t.scopes.at(-1));
  }
  state = r.ruleStack;
}
const expect = {
  module: "keyword.declaration.verkodo",
  "de.ustg.p19": "entity.name.verkodo",
  "// a comment": "comment.line.double-slash.verkodo",
  residence: "entity.name.verkodo",
  Kleinunternehmer: "string.unquoted.verkodo",
  "2025-01-01": "constant.numeric.date.verkodo",
  money: "support.type.verkodo",
  "25_000": "constant.numeric.verkodo",
  otherwise: "keyword.other.verkodo",
  "by simp": "source.lean4.embedded.verkodo",
  "∀ x, x = x": "source.lean4.embedded.verkodo",
  proof: "keyword.other.verkodo",
  "§ 19 UStG": "string.quoted.double.verkodo",
  Umsatz: "string.quoted.double.verkodo",
};
let failed = 0;
for (const [word, scope] of Object.entries(expect)) {
  const got = scopes.get(word);
  if (got !== scope) {
    failed++;
    console.error(`${word}: expected ${scope}, got ${got}`);
  }
}
if (failed) process.exit(1);
console.log(`grammar: ${Object.keys(expect).length} tokens as expected`);
