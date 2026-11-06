// SPDX-License-Identifier: AGPL-3.0-or-later
// verkodo in the browser: the WebAssembly module from crates/verkodo-wasm (`pnpm wasm` builds it). Requests and
// answers are JSON in a buffer of the module's memory; positions are UTF-16 offsets, as in the editor.
import wasmUrl from "./verkodo.wasm?url";

export interface SourceFile {
  path: string;
  text: string;
}

export interface Diagnostic {
  file: number;
  from: number;
  to: number;
  severity: "error" | "warning";
  message: string;
}

export interface Declaration {
  file: number;
  id: string;
  kind: string;
  label: string;
  from: number;
  to: number;
}

export interface Loaded {
  diagnostics: Diagnostic[];
  /** Lean's counts [checked, open, rejected], while its verdict holds (the files are the ones Lean checked). */
  lean: [number, number, number] | null;
  outline: Declaration[];
  /** Paths as [id, label]. */
  paths: [string, string][];
}

export interface Hover {
  from: number;
  to: number;
  html: string;
}

export interface Place {
  file: number;
  from: number;
  to: number;
}

interface Exports {
  memory: WebAssembly.Memory;
  vk_buffer(len: number): number;
  vk_address(): number;
  vk_load(): number;
  vk_hover(): number;
  vk_definition(): number;
  vk_preview(): number;
  vk_lean(): number;
  vk_format(): number;
  vk_norms(): number;
  vk_eval(): number;
  vk_goals(): number;
}

/** A tactic of a Lean proof with the goals before and after it, as Lean prints them. */
export interface GoalsAt {
  from: number;
  to: number;
  before: string[];
  after: string[];
}

/** A norm and what the debugger asks for. */
export interface NormInfo {
  id: string;
  label: string | null;
  /** `type`: as written (`money`, `residence`, `list supply`); `kind`: what it is (`bool`, `enumeration`, `list` …). */
  inputs: {
    name: string;
    label: string;
    type: string;
    kind: string;
    /** The values of an enumeration, to choose from. */
    values: { name: string; label: string | null }[];
    judgement: boolean;
  }[];
}

/** A member of a norm as evaluated: its value and, for a rule, its default and each exception tried. */
export interface EvalStep {
  id: string;
  kind: string;
  label: string | null;
  value: string;
  known: boolean;
  default: string | null;
  /** [condition's value, whether it decided] */
  exceptions: [string, boolean][];
  file: number;
  from: number;
  to: number;
}

export type Style = "delimiters" | "indentation" | "tabs";

const encoder = new TextEncoder();
const decoder = new TextDecoder();

export class Verkodo {
  private constructor(private readonly x: Exports) {}

  static async start(): Promise<Verkodo> {
    const bytes = await (await fetch(wasmUrl)).arrayBuffer();
    const { instance } = await WebAssembly.instantiate(bytes, {});
    return new Verkodo(instance.exports as unknown as Exports);
  }

  private call<T>(
    op: "vk_load" | "vk_hover" | "vk_definition" | "vk_preview" | "vk_lean" | "vk_format" | "vk_norms" | "vk_eval" | "vk_goals",
    request: unknown,
  ): T {
    const bytes = encoder.encode(JSON.stringify(request));
    const at = this.x.vk_buffer(bytes.length);
    new Uint8Array(this.x.memory.buffer, at, bytes.length).set(bytes);
    const len = this.x[op]();
    return JSON.parse(decoder.decode(new Uint8Array(this.x.memory.buffer, this.x.vk_address(), len))) as T;
  }

  /** Parses and analyses all files; later calls answer about this state. */
  load(files: SourceFile[], lang: string): Loaded {
    return this.call("vk_load", { files, lang });
  }

  /**
   * Takes what `POST /api/v1/prove` answered for `files`. It holds only if these are still the files of the last
   * `load`; then the diagnostics are Lean's too, and cards, hover and preview show its verdict.
   */
  lean(answer: object, files: SourceFile[]): { taken: boolean; loaded: Loaded } | null {
    return this.call("vk_lean", { ...answer, files });
  }

  /** The text in a way of writing; a file with errors is left alone (`error` says why). */
  format(text: string, style: Style, width: number | null): { text?: string; error?: string } {
    return this.call("vk_format", { text, style: style === "tabs" ? null : style, width });
  }

  /** The goals of the tactic at a place – while Lean's verdict holds. */
  goals(file: number, pos: number): GoalsAt | null {
    return this.call("vk_goals", { file, pos });
  }

  norms(): NormInfo[] {
    return this.call("vk_norms", {}) ?? [];
  }

  evaluate(norm: string, inputs: Record<string, string>): EvalStep[] {
    return this.call("vk_eval", { norm, inputs }) ?? [];
  }

  hover(file: number, pos: number): Hover | null {
    return this.call("vk_hover", { file, pos });
  }

  definition(file: number, pos: number): Place | null {
    return this.call("vk_definition", { file, pos });
  }

  /**
   * The page of a path, or the index without one, as a whole HTML document. Shown in a frame of the page, it inherits
   * the page's Content-Security-Policy: its inline style and script carry the page's `nonce`.
   */
  preview(path: string | null, nonce: string): string {
    const html: string = this.call("vk_preview", { path });
    return html.replace("<style>", `<style nonce="${nonce}">`).replace("<script>", `<script nonce="${nonce}">`);
  }
}
