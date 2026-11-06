// SPDX-License-Identifier: AGPL-3.0-or-later
// The web app is built into crates/verkodo-web/web/, which the verkodo binary embeds – inside the crate, so that it is
// part of the crate on crates.io too. In development, `pnpm dev` serves it and passes
// /api on to a `verkodo serve` running on the usual port. The editor bundles the examples from ../examples and the
// WebAssembly module from `pnpm wasm` (src/lib/verkodo.wasm).
import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

export default defineConfig({
  plugins: [svelte()],
  build: { outDir: "../crates/verkodo-web/web", emptyOutDir: true, assetsInlineLimit: 0 },
  server: { proxy: { "/api": "http://127.0.0.1:8080" }, fs: { allow: [".."] } },
});
