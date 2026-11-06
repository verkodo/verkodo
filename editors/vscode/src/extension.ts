// SPDX-License-Identifier: AGPL-3.0-or-later
// The VS Code extension: .vk files as a language, and `verkodo lsp` as its language server – everything else
// (diagnostics, hover, go to definition, outline, formatting, Lean on save) comes from the server, as in any editor.
import * as vscode from "vscode";
import { LanguageClient, type LanguageClientOptions, type ServerOptions } from "vscode-languageclient/node";

let client: LanguageClient | undefined;

/** The options of `verkodo lsp` (docs/editors.md) from the settings. */
function initializationOptions(): Record<string, unknown> {
  const c = vscode.workspace.getConfiguration("verkodo");
  const options: Record<string, unknown> = { lang: c.get<string>("lang", "de") };
  const lean = c.get<string | boolean>("lean", "");
  if (lean === false || (typeof lean === "string" && lean !== "")) options.lean = lean;
  const style = c.get<string>("style", "");
  if (style) options.style = style;
  const width = c.get<number | null>("width", null);
  if (typeof width === "number") options.width = width;
  return options;
}

async function start(context: vscode.ExtensionContext): Promise<void> {
  const command = vscode.workspace.getConfiguration("verkodo").get<string>("path", "verkodo") || "verkodo";
  const server: ServerOptions = { command, args: ["lsp"] };
  const options: LanguageClientOptions = {
    documentSelector: [{ language: "verkodo" }],
    initializationOptions: initializationOptions(),
  };
  client = new LanguageClient("verkodo", "verkodo", server, options);
  try {
    await client.start();
  } catch (e) {
    const message = e instanceof Error ? e.message : String(e);
    void vscode.window.showErrorMessage(
      `verkodo: cannot start \`${command} lsp\` (${message}). Install it with \`cargo install verkodo\` or set \`verkodo.path\`.`,
    );
  }
  context.subscriptions.push({ dispose: () => void client?.stop() });
}

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  context.subscriptions.push(
    vscode.commands.registerCommand("verkodo.restart", async () => {
      await client?.stop();
      await start(context);
    }),
    vscode.workspace.onDidChangeConfiguration(async (e) => {
      if (e.affectsConfiguration("verkodo")) {
        await client?.stop();
        await start(context);
      }
    }),
  );
  await start(context);
}

export async function deactivate(): Promise<void> {
  await client?.stop();
}
