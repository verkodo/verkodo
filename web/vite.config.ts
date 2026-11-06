// SPDX-License-Identifier: AGPL-3.0-or-later
// The web app is built into crates/verkodo-web/web/, which the verkodo binary embeds – inside the crate, so that it is
// part of the crate on crates.io too. In development, `pnpm dev` serves it and passes
// /api on to a `verkodo serve` running on the usual port. The editor bundles the examples from ../examples and the
// WebAssembly module from `pnpm wasm` (src/lib/verkodo.wasm).
//
// `pnpm build:pages` builds it for a static host instead (GitHub Pages, below /verkodo/editor/; another path with
// `pnpm build:pages --base /path/`) into dist-pages/: no server, so no API and no Lean, and the Content-Security-Policy comes as a
// <meta> element. Without a server there is no nonce: scripts stay as strict as with `verkodo serve` – the preview's
// script is a fixed text of verkodo-render, hashed here – but styles are allowed inline, since CodeMirror adds its
// own <style> elements. Styles cannot run code. (frame-ancestors cannot be set by a <meta> element.)
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { defineConfig, type Plugin } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

/** The hash for the policy of one of the fixed texts in verkodo-render's assets.rs. */
function assetHash(name: "JS"): string {
  const assets = readFileSync(new URL("../crates/verkodo-render/src/assets.rs", import.meta.url), "utf8");
  const text = new RegExp(`pub const ${name}: &str = r#"([\\s\\S]*?)"#;`).exec(assets)?.[1];
  if (text === undefined) throw new Error(`${name} not found in verkodo-render's assets.rs`);
  return `'sha256-${createHash("sha256").update(text).digest("base64")}'`;
}

/** The policy as a <meta> element, and no nonce, for a static host. */
function staticPolicy(): Plugin {
  const policy =
    `default-src 'self'; script-src 'self' 'wasm-unsafe-eval' ${assetHash("JS")}; ` +
    `style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self'; connect-src 'self'; ` +
    `base-uri 'none'; form-action 'self'`;
  return {
    name: "verkodo-static-policy",
    transformIndexHtml: (html) =>
      html.replace(
        `<meta name="csp-nonce" content="verkodo-nonce" />`,
        `<meta http-equiv="Content-Security-Policy" content="${policy}" />\n    <meta name="csp-nonce" content="" />`,
      ),
  };
}

export default defineConfig(({ mode }) =>
  mode === "pages"
    ? {
        plugins: [svelte(), staticPolicy()],
        base: "/verkodo/editor/",
        build: { outDir: "dist-pages", emptyOutDir: true, assetsInlineLimit: 0 },
      }
    : {
        plugins: [svelte()],
        build: { outDir: "../crates/verkodo-web/web", emptyOutDir: true, assetsInlineLimit: 0 },
        server: { proxy: { "/api": "http://127.0.0.1:8080" }, fs: { allow: [".."] } },
      },
);
