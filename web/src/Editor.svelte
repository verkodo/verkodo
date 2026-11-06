<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  The web editor: .vk files in CodeMirror, checked in the browser by verkodo compiled to WebAssembly – diagnostics
  while typing, hover cards, go to definition (F12 or Ctrl/Cmd-click), an outline and a live preview of the pages.
  The files stay in this browser (localStorage); only “Check with Lean” sends them – to the verkodo serve it came from.
-->
<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { Compartment, EditorState, type Extension } from "@codemirror/state";
  import {
    EditorView,
    hoverTooltip,
    keymap,
    lineNumbers,
    highlightActiveLine,
    drawSelection,
  } from "@codemirror/view";
  import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
  import { bracketMatching } from "@codemirror/language";
  import { lintGutter, setDiagnostics, type Diagnostic as CmDiagnostic } from "@codemirror/lint";
  import { verkodo as vkLanguage, theme, highlighting } from "./lib/language";
  import { Verkodo, type GoalsAt, type Loaded, type SourceFile } from "./lib/verkodo";
  import { api } from "./lib/api";
  import { icons } from "./lib/icons";
  import { settings, saveSettings, applyTheme, type Theme } from "./lib/settings.svelte";
  import Modal from "./Modal.svelte";
  import Debug from "./Debug.svelte";

  // The examples of the repository, as a start (CC BY 4.0, see each file's header).
  const examples = import.meta.glob("../../examples/**/*.vk", {
    query: "?raw",
    import: "default",
    eager: true,
  }) as Record<string, string>;
  const STORE = "verkodo.editor.files";

  function exampleFiles(): SourceFile[] {
    return Object.entries(examples)
      .map(([p, text]) => ({ path: p.replace("../../", ""), text }))
      .sort((a, b) => a.path.localeCompare(b.path));
  }

  function storedFiles(): SourceFile[] | null {
    try {
      const raw = localStorage.getItem(STORE);
      const files = raw ? (JSON.parse(raw) as SourceFile[]) : null;
      return Array.isArray(files) && files.length > 0 ? files : null;
    } catch {
      return null;
    }
  }

  let files = $state<SourceFile[]>(storedFiles() ?? exampleFiles());
  let current = $state(0);
  // The interface speaks German or English; labels and texts may be in any language of the files.
  const de = $derived(settings.lang === "de");
  const t = (german: string, english: string) => (de ? german : english);
  let loaded = $state<Loaded>({ diagnostics: [], lean: null, outline: [], paths: [] });
  // Counts the analyses, so that the debugger evaluates again.
  let version = $state(0);
  // Whether the server lets Lean check (`verkodo serve --lean <dir>`), and what Lean is doing.
  let leanAvailable = $state(false);
  // Built for a static host (`pnpm build:pages`, GitHub Pages): no server, so no API and no Lean.
  const hosted = import.meta.env.MODE === "pages";
  let proving = $state(false);
  let leanNote = $state("");
  let note = $state("");
  let panel = $state<"outline" | "preview">("outline");
  let bottom = $state<"problems" | "lean" | "goals" | "debug" | null>(null);
  // The goals at the cursor, while Lean's verdict holds.
  let goalsAt = $state<GoalsAt | null>(null);
  let dialog = $state<"settings" | "keys" | null>(null);
  let previewPath = $state<string | null>(null);
  let previewHtml = $state("");
  let failure = $state("");

  // The nonce of this page's Content-Security-Policy, which `verkodo serve` sets: the editor's style elements and the
  // preview's inline style and script carry it.
  const nonce = document.querySelector('meta[name="csp-nonce"]')?.getAttribute("content") ?? "";

  let vk = $state<Verkodo | null>(null);
  let host: HTMLDivElement;
  let frame = $state<HTMLIFrameElement>();
  let view: EditorView | null = null;
  // One editor state per file, so that each keeps its undo history.
  let states: EditorState[] = [];
  // What the settings change in the editor: line wrapping and the size of the font.
  const wrapping = new Compartment();
  const fontSize = new Compartment();

  const errors = $derived(loaded.diagnostics.filter((d) => d.severity === "error").length);
  const warnings = $derived(loaded.diagnostics.filter((d) => d.severity === "warning").length);
  const outline = $derived(loaded.outline.filter((s) => s.file === current));
  // Every diagnostic of every file, by file and place, with its line.
  const problems = $derived(
    loaded.diagnostics
      .map((d) => ({ ...d, line: (files[d.file]?.text.slice(0, d.from).match(/\n/g)?.length ?? 0) + 1 }))
      .sort((a, b) => a.file - b.file || a.from - b.from),
  );
  // The languages of labels in the files: the originals and those of translations.
  const languages = $derived(
    [...new Set(["de", "en", ...files.flatMap((f) => [...f.text.matchAll(/\b(?:label|text) ([a-z]{2})\b/g)].map((m) => m[1]!))])].sort(),
  );

  const KEYS: [string, string, string][] = [
    ["F12, Strg/⌘-Klick", "Zur Definition", "Go to definition"],
    ["Umschalt-Alt-F", "Formatieren", "Format"],
    ["Strg/⌘-Umschalt-B", "Mit Lean prüfen", "Check with Lean"],
    ["F5", "Debug: Normen auswerten", "Debug: evaluate norms"],
    ["Strg/⌘-Umschalt-G", "Lean-Ziele am Cursor", "Lean goals at the cursor"],
    ["Strg/⌘-Umschalt-M", "Probleme", "Problems"],
    ["Strg/⌘-,", "Einstellungen", "Settings"],
    ["F1", "Diese Übersicht", "This overview"],
    ["Strg/⌘-/", "Kommentar ein/aus", "Toggle comment"],
    ["Strg/⌘-Z, Strg/⌘-Umschalt-Z", "Rückgängig, Wiederholen", "Undo, redo"],
    ["Alt-↑, Alt-↓", "Zeile verschieben", "Move line"],
    ["Strg/⌘-]", "Einrücken", "Indent"],
    ["Strg/⌘-[", "Ausrücken", "Outdent"],
    ["Escape", "Dialog schließen", "Close a dialog"],
  ];

  function save() {
    try {
      localStorage.setItem(STORE, JSON.stringify(files));
    } catch {
      // Private mode or full storage: the editor still works, it only forgets.
    }
  }

  /** Analyses all files again and shows the result: ~10 ms for the examples, so on every change. */
  function analyse() {
    if (!vk) return;
    try {
      loaded = vk.load(files, settings.lang);
      failure = "";
    } catch (e) {
      failure = String(e);
      return;
    }
    version++;
    showDiagnostics();
    if (panel === "preview") refreshPreview();
  }

  function showDiagnostics() {
    if (!view) return;
    const len = view.state.doc.length;
    const diags: CmDiagnostic[] = loaded.diagnostics
      .filter((d) => d.file === current)
      .map((d) => ({
        from: Math.min(d.from, len),
        to: Math.min(Math.max(d.to, d.from), len),
        severity: d.severity,
        message: d.message,
      }));
    view.dispatch(setDiagnostics(view.state, diags));
  }

  function refreshPreview() {
    if (!vk) return;
    previewHtml = vk.preview(previewPath, nonce);
  }

  /** Writes the current file in the way of writing of the settings; a file with errors stays as it is. */
  function format(): boolean {
    if (!vk || !view) return false;
    const text = view.state.doc.toString();
    const result = vk.format(text, settings.style, settings.width);
    if (result.error !== undefined) {
      note = t("Nicht formatiert: ", "Not formatted: ") + result.error;
      return true;
    }
    note = "";
    if (result.text !== undefined && result.text !== text) {
      const head = Math.min(view.state.selection.main.head, result.text.length);
      view.dispatch({ changes: { from: 0, to: text.length, insert: result.text }, selection: { anchor: head } });
    }
    return true;
  }

  /** Sends all files to the server for Lean; the verdict holds until the next change. */
  async function prove() {
    if (!vk || proving || !leanAvailable) return;
    bottom = "lean";
    const sent = files.map((f) => ({ path: f.path, text: f.text }));
    proving = true;
    leanNote = "";
    try {
      const { data, error } = await api.POST("/api/v1/prove", { body: { files: sent } });
      if (!data) {
        leanNote = error?.error ?? "Lean?";
        return;
      }
      const answer = vk.lean(data, sent);
      if (answer?.taken) {
        loaded = answer.loaded;
        updateGoals();
        showDiagnostics();
        if (panel === "preview") refreshPreview();
      } else {
        leanNote = t("Geändert, während Lean prüfte – bitte erneut prüfen.", "Changed while Lean was checking – check again.");
      }
    } catch (e) {
      leanNote = String(e);
    } finally {
      proving = false;
    }
  }

  function goTo(file: number, from: number, to: number) {
    if (!view) return;
    if (file !== current) switchTo(file);
    view.dispatch({ selection: { anchor: from, head: to }, scrollIntoView: true });
    view.focus();
  }

  function goToId(id: string) {
    const s = loaded.outline.find((d) => d.id === id);
    if (s) goTo(s.file, s.from, s.to);
  }

  function definitionAt(pos: number): boolean {
    const place = vk?.definition(current, pos);
    if (!place) return false;
    goTo(place.file, place.from, place.to);
    return true;
  }

  /** The goals of the tactic at the cursor. */
  function updateGoals() {
    goalsAt = view && vk && loaded.lean ? vk.goals(current, view.state.selection.main.head) : null;
  }

  function toggle(which: "problems" | "lean" | "goals" | "debug") {
    bottom = bottom === which ? null : which;
  }

  /** The keys of the editor, also where the focus is outside the code. */
  function command(e: KeyboardEvent): boolean {
    const mod = e.metaKey || e.ctrlKey;
    if (e.key === "F1") dialog = "keys";
    else if (e.key === "F5") bottom = "debug";
    else if (mod && e.shiftKey && e.key.toLowerCase() === "b") void prove();
    else if (mod && e.shiftKey && e.key.toLowerCase() === "m") toggle("problems");
    else if (mod && e.shiftKey && e.key.toLowerCase() === "g") toggle("goals");
    else if (mod && e.key === ",") dialog = "settings";
    else if (e.altKey && e.shiftKey && e.code === "KeyF") format();
    else return false;
    e.preventDefault();
    return true;
  }

  function editorSettings(): Extension[] {
    return [
      wrapping.of(settings.wrap ? EditorView.lineWrapping : []),
      fontSize.of(EditorView.theme({ ".cm-scroller": { fontSize: `${settings.fontSize}px` } })),
    ];
  }

  function extensions(): Extension[] {
    return [
      EditorView.cspNonce.of(nonce),
      lineNumbers(),
      history(),
      drawSelection(),
      highlightActiveLine(),
      bracketMatching(),
      vkLanguage,
      theme,
      highlighting,
      lintGutter(),
      ...editorSettings(),
      keymap.of([
        { key: "F12", run: (v) => definitionAt(v.state.selection.main.head) },
        { key: "Shift-Alt-f", run: () => format() },
        ...defaultKeymap,
        ...historyKeymap,
      ]),
      hoverTooltip((_view, pos) => {
        const h = vk?.hover(current, pos);
        if (!h) return null;
        return {
          pos: h.from,
          end: h.to,
          above: true,
          create() {
            const dom = document.createElement("div");
            dom.className = "vk-card";
            // HTML from verkodo's renderer, which escapes every text it puts in.
            dom.innerHTML = h.html;
            dom.addEventListener("click", (e) => {
              const ref = (e.target as HTMLElement).closest("a[data-ref]");
              if (ref) {
                e.preventDefault();
                goToId(ref.getAttribute("data-ref") ?? "");
              }
            });
            return { dom };
          },
        };
      }),
      EditorView.domEventHandlers({
        mousedown(e, v) {
          if (!(e.metaKey || e.ctrlKey)) return false;
          const pos = v.posAtCoords({ x: e.clientX, y: e.clientY });
          return pos !== null && definitionAt(pos);
        },
      }),
      EditorView.updateListener.of((u) => {
        if (u.selectionSet && !u.docChanged) updateGoals();
        if (!u.docChanged) return;
        states[current] = u.state;
        files[current] = { path: files[current]!.path, text: u.state.doc.toString() };
        save();
        analyse();
      }),
    ];
  }

  function stateFor(i: number): EditorState {
    states[i] ??= EditorState.create({ doc: files[i]!.text, extensions: extensions() });
    return states[i]!;
  }

  /** The settings into the current file's editor (each file has its own state). */
  function applyEditorSettings() {
    view?.dispatch({
      effects: [
        wrapping.reconfigure(settings.wrap ? EditorView.lineWrapping : []),
        fontSize.reconfigure(EditorView.theme({ ".cm-scroller": { fontSize: `${settings.fontSize}px` } })),
      ],
    });
  }

  function switchTo(i: number) {
    if (!view || i === current) return;
    if (settings.formatOnSwitch) format();
    states[current] = view.state;
    current = i;
    view.setState(stateFor(i));
    applyEditorSettings();
    showDiagnostics();
  }

  function reset() {
    if (!confirm(t("Alle Änderungen verwerfen und die Beispiele laden?", "Discard all changes and load the examples?"))) return;
    files = exampleFiles();
    states = [];
    current = 0;
    view?.setState(stateFor(0));
    applyEditorSettings();
    save();
    analyse();
    dialog = null;
  }

  function newFile() {
    const name = prompt(t("Name der Datei (z. B. mein/modul.vk)", "File name (e.g. my/module.vk)"));
    if (!name) return;
    const path = name.endsWith(".vk") ? name : `${name}.vk`;
    const module = path.replace(/\.vk$/, "").replaceAll("/", ".").replaceAll("-", "_");
    files.push({ path, text: `module ${module}\n\n` });
    save();
    switchTo(files.length - 1);
    analyse();
  }

  function setTheme(theme: Theme) {
    settings.theme = theme;
  }

  // Links in the preview: pages of paths and the index stay inside the preview.
  function wirePreview() {
    const doc = frame?.contentDocument;
    if (!doc) return;
    doc.addEventListener("click", (e) => {
      const a = (e.target as HTMLElement).closest("a");
      const href = a?.getAttribute("href");
      if (!a || !href) return;
      e.preventDefault();
      const [page, anchor] = href.split("#");
      const next = page === "index.html" || page === "" ? null : page!.replace(/\.html$/, "");
      if (page && next !== previewPath) {
        previewPath = next;
        refreshPreview();
      }
      if (anchor) setTimeout(() => frame?.contentDocument?.getElementById(anchor)?.scrollIntoView(), 50);
    });
  }

  onMount(() => {
    view = new EditorView({ parent: host, state: stateFor(0) });
    Verkodo.start()
      .then((v) => {
        vk = v;
        analyse();
      })
      .catch((e) => (failure = String(e)));
    if (!hosted) api
      .GET("/api/v1/lean")
      .then(({ data }) => (leanAvailable = data?.available ?? false))
      .catch(() => (leanAvailable = false));
    const keys = (e: KeyboardEvent) => {
      if (!e.defaultPrevented) command(e);
    };
    addEventListener("keydown", keys);
    return () => {
      removeEventListener("keydown", keys);
      view?.destroy();
    };
  });

  // Settings: kept, applied to page and editor; another language analyses again.
  $effect(() => {
    void [settings.lang, settings.theme, settings.style, settings.width, settings.formatOnSwitch, settings.fontSize, settings.wrap];
    untrack(() => {
      saveSettings();
      applyTheme(settings.theme);
      applyEditorSettings();
    });
  });
  $effect(() => {
    void settings.lang;
    // Only the language: what the analysis reads and writes itself must not run it again.
    untrack(analyse);
  });
</script>

<svelte:head><meta name="color-scheme" content={settings.theme === "system" ? "light dark" : settings.theme} /></svelte:head>

<div class="editor">
  <header>
    <strong class="brand">verkodo</strong>
    <nav class="files">
      {#each files as f, i (f.path)}
        <button class="tab" class:active={i === current} onclick={() => switchTo(i)} title={f.path}>
          {f.path.split("/").pop()}
          {#if loaded.diagnostics.some((d) => d.file === i && d.severity === "error")}<span class="dot"></span>{/if}
        </button>
      {/each}
      <button class="tab" onclick={newFile} title={t("Neue Datei", "New file")}>+</button>
    </nav>
    <div class="tools">
      <button class="tool" onclick={() => toggle("problems")} class:on={bottom === "problems"}
        title={t("Prüfen: Probleme (Strg/⌘-Umschalt-M)", "Check: problems (Ctrl/⌘-Shift-M)")}>
        {@html icons.check}<span>{t("Prüfen", "Check")}</span>
        {#if errors + warnings > 0}<span class="count" class:bad={errors > 0}>{errors + warnings}</span>{/if}
      </button>
      <button class="tool" onclick={prove} disabled={!leanAvailable || proving || !!failure}
        title={leanAvailable ? t("Mit Lean prüfen (Strg/⌘-Umschalt-B)", "Check with Lean (Ctrl/⌘-Shift-B)") : hosted ? t("Lean braucht verkodo serve --lean auf dem eigenen Gerät", "Lean needs verkodo serve --lean on your own device") : t("Lean ist auf diesem Server aus (verkodo serve --lean <dir>)", "Lean is off on this server (verkodo serve --lean <dir>)")}>
        {@html icons.build}<span>{proving ? t("Lean …", "Lean …") : t("Kompilieren", "Compile")}</span>
      </button>
      <button class="tool" onclick={() => toggle("debug")} class:on={bottom === "debug"} title={t("Debug: Normen auswerten (F5)", "Debug: evaluate norms (F5)")}>
        {@html icons.debug}<span>Debug</span>
      </button>
      <button class="tool" onclick={format} title={t("Formatieren (Umschalt-Alt-F)", "Format (Shift-Alt-F)")}>
        {@html icons.format}<span>{t("Formatieren", "Format")}</span>
      </button>
      <span class="sep"></span>
      <div class="themes" role="group" aria-label={t("Darstellung", "Appearance")}>
        <button class="icon" aria-pressed={settings.theme === "light"} onclick={() => setTheme("light")} title={t("Hell", "Light")}>{@html icons.sun}</button>
        <button class="icon" aria-pressed={settings.theme === "dark"} onclick={() => setTheme("dark")} title={t("Dunkel", "Dark")}>{@html icons.moon}</button>
        <button class="icon" aria-pressed={settings.theme === "system"} onclick={() => setTheme("system")} title={t("Wie das System", "Like the system")}>{@html icons.system}</button>
      </div>
      <button class="icon" onclick={() => (dialog = "keys")} title={t("Tastenkürzel (F1)", "Keyboard shortcuts (F1)")}>{@html icons.keyboard}</button>
      <button class="icon" onclick={() => (dialog = "settings")} title={t("Einstellungen (Strg/⌘-,)", "Settings (Ctrl/⌘-,)")}>{@html icons.settings}</button>
    </div>
  </header>

  <div class="body" class:with-bottom={bottom !== null}>
    <div class="code" bind:this={host}></div>
    <aside>
      <div class="tabs">
        <button class="tab" class:active={panel === "outline"} onclick={() => (panel = "outline")}>{t("Gliederung", "Outline")}</button>
        <button class="tab" class:active={panel === "preview"} onclick={() => { panel = "preview"; refreshPreview(); }}>{t("Vorschau", "Preview")}</button>
        {#if panel === "preview"}
          <select bind:value={previewPath} onchange={refreshPreview} aria-label="Path">
            <option value={null}>{t("Übersicht", "Index")}</option>
            {#each loaded.paths as [id, label] (id)}<option value={id}>{label || id}</option>{/each}
          </select>
        {/if}
      </div>
      {#if panel === "outline"}
        <ul class="outline">
          {#each outline as s (s.id)}
            <li class={`k-${s.kind}`}>
              <button onclick={() => goTo(s.file, s.from, s.to)}>
                <span class="kind">{s.kind}</span> {s.id.split(".").pop()}
                {#if s.label}<span class="label">– {s.label}</span>{/if}
              </button>
            </li>
          {/each}
        </ul>
      {:else}
        <iframe bind:this={frame} title="Preview" srcdoc={previewHtml} onload={wirePreview}
          style:color-scheme={settings.theme === "system" ? "normal" : settings.theme}></iframe>
      {/if}
    </aside>
    {#if bottom}
      <section class="bottom">
        <div class="tabs">
          <button class="tab" class:active={bottom === "problems"} onclick={() => (bottom = "problems")}>{t("Probleme", "Problems")} ({problems.length})</button>
          <button class="tab" class:active={bottom === "lean"} onclick={() => (bottom = "lean")}>Lean</button>
          <button class="tab" class:active={bottom === "goals"} onclick={() => { bottom = "goals"; updateGoals(); }}>{t("Ziele", "Goals")}</button>
          <button class="tab" class:active={bottom === "debug"} onclick={() => (bottom = "debug")}>Debug</button>
          <span class="grow"></span>
          <button class="icon" onclick={() => (bottom = null)} title={t("Schließen", "Close")}>{@html icons.close}</button>
        </div>
        <div class="bottom-body">
          {#if bottom === "problems"}
            {#if problems.length === 0}
              <p class="hint">{t("Keine Probleme.", "No problems.")}</p>
            {:else}
              <ul class="problems">
                {#each problems as p, k (k)}
                  <li>
                    <button onclick={() => goTo(p.file, p.from, p.to)}>
                      <span class={p.severity}>{p.severity === "error" ? "●" : "▲"}</span>
                      <span class="where">{files[p.file]?.path.split("/").pop()}:{p.line}</span>
                      {p.message}
                    </button>
                  </li>
                {/each}
              </ul>
            {/if}
          {:else if bottom === "lean"}
            <div class="lean-panel">
              {#if hosted}
                <p class="hint">{t("Dieser Editor läuft ganz im Browser – die Dateien bleiben hier, nichts wird gesendet. Lean prüft auf dem eigenen Gerät: „cargo install verkodo“, dann „verkodo serve --lean lean“.", "This editor runs entirely in the browser – the files stay here, nothing is sent. Lean checks on your own device: “cargo install verkodo”, then “verkodo serve --lean lean”.")}</p>
              {:else if !leanAvailable}
                <p class="hint">{t("Lean ist auf diesem Server ausgeschaltet. Mit „verkodo serve --lean lean“ prüft Lean die Dateien auf dem Server.", "Lean is switched off on this server. With “verkodo serve --lean lean” Lean checks the files on the server.")}</p>
              {:else}
                <button class="tool" onclick={prove} disabled={proving || !!failure}>{@html icons.play}<span>{t("Mit Lean prüfen", "Check with Lean")}</span></button>
                {#if proving}<p>{t("Lean prüft … (für die Beispiele etwa eine halbe Minute)", "Lean is checking … (about half a minute for the examples)")}</p>
                {:else if loaded.lean}
                  <p class:bad={loaded.lean[2] > 0} class:good={loaded.lean[2] === 0}>
                    {loaded.lean[0]} {t("geprüft", "checked")}, {loaded.lean[1]} {t("offen", "open")}, {loaded.lean[2]} {t("abgelehnt", "rejected")}
                  </p>
                  <p class="hint">{t("Das Urteil gilt bis zur nächsten Änderung; Lean-Fehler stehen unter „Probleme“.", "The verdict holds until the next change; Lean's errors are under “Problems”.")}</p>
                {:else if leanNote}<p class="bad">{leanNote}</p>
                {:else}<p class="hint">{t("Noch nicht geprüft – oder seit der letzten Prüfung geändert.", "Not checked yet – or changed since.")}</p>{/if}
              {/if}
            </div>
          {:else if bottom === "goals"}
            <div class="goals">
              {#if !loaded.lean}
                <p class="hint">{t("Ziele gibt es, sobald Lean geprüft hat (Kompilieren) – bis zur nächsten Änderung.", "Goals are there once Lean has checked (Compile) – until the next change.")}</p>
              {:else if !goalsAt}
                <p class="hint">{t("Den Cursor in eine Taktik eines Lean-Beweises setzen.", "Put the cursor into a tactic of a Lean proof.")}</p>
              {:else}
                <div class="goal-col">
                  <h4>{t("Vor der Taktik", "Before the tactic")}</h4>
                  {#each goalsAt.before as g, k (k)}<pre>{g}</pre>{:else}<p class="hint">{t("keine", "none")}</p>{/each}
                </div>
                <div class="goal-col">
                  <h4>{t("Danach", "After")}</h4>
                  {#each goalsAt.after as g, k (k)}<pre>{g}</pre>{:else}<p class="good">{t("keine – hier ist alles gezeigt", "none – all is shown here")}</p>{/each}
                </div>
              {/if}
            </div>
          {:else}
            <Debug {vk} {version} {de} {goTo} />
          {/if}
        </div>
      </section>
    {/if}
  </div>
  <footer>
    {#if note}<span class="bad">{note}</span>{:else}
      <span>{errors} {t("Fehler", errors === 1 ? "error" : "errors")}, {warnings} {t("Warnungen", warnings === 1 ? "warning" : "warnings")}</span>
      {#if loaded.lean}<span>· Lean: {loaded.lean[0]} {t("geprüft", "checked")}, {loaded.lean[1]} {t("offen", "open")}, {loaded.lean[2]} {t("abgelehnt", "rejected")}</span>{/if}
      {#if failure}<span class="bad">· {failure}</span>{/if}
    {/if}
    <span class="grow"></span>
    <span>{t("Schreibweise", "Writing")}: {settings.style === "indentation" ? t("Einrückung", "indentation") : settings.style === "delimiters" ? t("Begrenzer", "delimiters") : "Tabs → 2"}{settings.width ? ` · ${settings.width}` : ""}</span>
    <span>· {settings.lang}</span>
    <span>· {t("Dateien bleiben in diesem Browser", "Files stay in this browser")}</span>
  </footer>
</div>

{#if dialog === "settings"}
  <Modal title={t("Einstellungen", "Settings")} onclose={() => (dialog = null)}>
    <div class="form">
      <label>
        <span>{t("Sprache", "Language")}</span>
        <select bind:value={settings.lang}>
          {#each languages as l (l)}<option value={l}>{l === "de" ? "Deutsch" : l === "en" ? "English" : l === "fr" ? "Français" : l}</option>{/each}
        </select>
        <small>{t("Oberfläche auf Deutsch oder Englisch; Labels und Texte in dieser Sprache, sonst Englisch.", "Interface in German or English; labels and texts in this language, else English.")}</small>
      </label>
      <label>
        <span>{t("Darstellung", "Appearance")}</span>
        <select bind:value={settings.theme}>
          <option value="system">{t("Wie das System", "Like the system")}</option>
          <option value="light">{t("Hell", "Light")}</option>
          <option value="dark">{t("Dunkel", "Dark")}</option>
        </select>
      </label>
      <label>
        <span>{t("Formatieren: Schreibweise", "Formatting: way of writing")}</span>
        <select bind:value={settings.style}>
          <option value="delimiters">{t("Begrenzer – { … }, \"…\", ```", "Delimiters – { … }, \"…\", ```")}</option>
          <option value="indentation">{t("Einrückung – ohne Begrenzer", "Indentation – without delimiters")}</option>
          <option value="tabs">{t("Nur Tabs zu Leerzeichen", "Only tabs to spaces")}</option>
        </select>
        <small>{t("Einrückung: 2 Leerzeichen je Ebene; ein Tab zählt bis zur nächsten geraden Spalte.", "Indentation: 2 spaces per level; a tab counts up to the next even column.")}</small>
      </label>
      <label>
        <span>{t("Formatieren: maximale Breite von Texten", "Formatting: widest line of text")}</span>
        <select
          value={settings.width === null ? "" : String(settings.width)}
          onchange={(e) => {
            const v = (e.currentTarget as HTMLSelectElement).value;
            settings.width = v === "" ? null : Number(v);
          }}
        >
          <option value="">{t("keine", "none")}</option>
          {#each [80, 100, 120] as w (w)}<option value={String(w)}>{w} {t("Zeichen", "characters")}</option>{/each}
        </select>
        <small>{t("Längere Labels, Texte, Maßstäbe und Quellen werden umgebrochen – ein Einzeiler wird zum Mehrzeiler. Lean-Code und Ausdrücke nie.", "Longer labels, texts, criteria and sources are broken into lines – a one-liner becomes a block. Lean code and expressions never.")}</small>
      </label>
      <label class="check"><input type="checkbox" bind:checked={settings.formatOnSwitch} /> {t("Beim Wechsel der Datei formatieren", "Format when switching files")}</label>
      <label>
        <span>{t("Schriftgröße", "Font size")}: {settings.fontSize}px</span>
        <input type="range" min="11" max="22" bind:value={settings.fontSize} />
      </label>
      <label class="check"><input type="checkbox" bind:checked={settings.wrap} /> {t("Lange Zeilen umbrechen", "Wrap long lines")}</label>
      <div class="row">
        <span>Lean: {hosted ? t("nur mit verkodo serve --lean auf dem eigenen Gerät", "only with verkodo serve --lean on your own device") : leanAvailable ? t("auf diesem Server eingeschaltet", "switched on on this server") : t("aus (verkodo serve --lean lean)", "off (verkodo serve --lean lean)")}</span>
      </div>
      <div class="row">
        <button class="tool" onclick={reset}>{t("Beispiele neu laden", "Load the examples again")}</button>
        <small>{t("verwirft alle Änderungen in diesem Browser", "discards all changes in this browser")}</small>
      </div>
    </div>
  </Modal>
{:else if dialog === "keys"}
  <Modal title={t("Tastenkürzel", "Keyboard shortcuts")} onclose={() => (dialog = null)}>
    <table class="keys">
      <tbody>
        {#each KEYS as [key, german, english] (key)}
          <tr><td><kbd>{de ? key : key.replaceAll("Strg", "Ctrl").replaceAll("Umschalt", "Shift")}</kbd></td><td>{de ? german : english}</td></tr>
        {/each}
      </tbody>
    </table>
  </Modal>
{/if}

<style>
  .editor {
    display: flex;
    flex-direction: column;
    height: 100vh;
  }
  header,
  footer {
    display: flex;
    gap: 0.5rem;
    align-items: center;
    padding: 0.35rem 0.6rem;
    font-size: 0.88rem;
  }
  header {
    border-bottom: 1px solid var(--border);
    background: var(--surface);
  }
  footer {
    border-top: 1px solid var(--border);
    color: var(--accent);
    font-size: 0.78rem;
    background: var(--surface);
  }
  .brand {
    margin-right: 0.25rem;
  }
  .files {
    display: flex;
    gap: 0.2rem;
    overflow-x: auto;
    flex: 1;
    min-width: 0;
  }
  .tools {
    display: flex;
    align-items: center;
    gap: 0.2rem;
  }
  button {
    font: inherit;
    cursor: pointer;
    white-space: nowrap;
    color: inherit;
    background: transparent;
    border: 1px solid transparent;
    border-radius: 5px;
  }
  button:hover:not(:disabled) {
    border-color: var(--border);
  }
  button:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .tab {
    padding: 0.2rem 0.55rem;
  }
  .tab.active,
  .tool.on,
  .icon[aria-pressed="true"] {
    background: var(--border);
  }
  .tool {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    padding: 0.25rem 0.5rem;
  }
  .icon {
    display: inline-flex;
    padding: 0.3rem;
  }
  .themes {
    display: inline-flex;
    gap: 0.05rem;
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 0.05rem;
  }
  .sep {
    width: 1px;
    align-self: stretch;
    background: var(--border);
    margin: 0 0.25rem;
  }
  .count {
    font-size: 0.72rem;
    border-radius: 999px;
    padding: 0 0.4rem;
    background: var(--border);
  }
  .dot {
    display: inline-block;
    width: 0.45rem;
    height: 0.45rem;
    border-radius: 50%;
    background: var(--bad);
    margin-left: 0.25rem;
  }
  .bad,
  .error {
    color: var(--bad);
  }
  .good {
    color: var(--good);
  }
  .warning {
    color: #b8860b;
  }
  .grow {
    flex: 1;
  }
  select {
    font: inherit;
    padding: 0.2rem;
    background: var(--bg);
    color: inherit;
    border: 1px solid var(--border);
    border-radius: 4px;
  }
  .body {
    flex: 1;
    display: grid;
    grid-template-columns: minmax(0, 3fr) minmax(16rem, 2fr);
    grid-template-rows: minmax(0, 1fr);
    min-height: 0;
  }
  .body.with-bottom {
    grid-template-rows: minmax(0, 1fr) minmax(9rem, 38%);
  }
  .code {
    min-height: 0;
    overflow: hidden;
    border-right: 1px solid var(--border);
  }
  .code :global(.cm-editor) {
    height: 100%;
  }
  .code :global(.cm-scroller) {
    font-family: ui-monospace, "SF Mono", Menlo, monospace;
  }
  aside {
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .tabs {
    display: flex;
    align-items: center;
    gap: 0.25rem;
    padding: 0.3rem 0.4rem;
    border-bottom: 1px solid var(--border);
  }
  .outline {
    overflow: auto;
    margin: 0;
    padding: 0.25rem 0;
    font-size: 0.85rem;
    list-style: none;
  }
  .outline button,
  .problems button {
    border: 0;
    border-radius: 0;
    width: 100%;
    text-align: left;
    padding: 0.15rem 0.75rem;
    white-space: normal;
  }
  .outline button:hover,
  .problems button:hover {
    background: var(--border);
  }
  .outline .kind {
    color: var(--accent);
    font-size: 0.72rem;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .outline .label {
    color: var(--accent);
  }
  .outline .k-variant,
  .outline .k-input,
  .outline .k-judgement,
  .outline .k-constant,
  .outline .k-rule {
    padding-left: 1rem;
  }
  iframe {
    flex: 1;
    border: 0;
    width: 100%;
  }
  .bottom {
    grid-column: 1 / -1;
    border-top: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    min-height: 0;
    background: var(--bg);
  }
  .bottom-body {
    overflow: auto;
    flex: 1;
  }
  .problems {
    list-style: none;
    margin: 0;
    padding: 0.2rem 0;
    font-size: 0.85rem;
  }
  .where {
    color: var(--accent);
    margin: 0 0.4rem;
    font-family: ui-monospace, monospace;
    font-size: 0.8rem;
  }
  .goals {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0.75rem;
    padding: 0.4rem 0.75rem;
  }
  .goals .hint {
    grid-column: 1 / -1;
    padding: 0;
  }
  .goal-col h4 {
    margin: 0.2rem 0;
    font-size: 0.8rem;
    color: var(--accent);
    font-weight: normal;
  }
  .goal-col pre {
    margin: 0 0 0.5rem;
    padding: 0.4rem 0.5rem;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 4px;
    font: 0.82rem/1.45 ui-monospace, "SF Mono", Menlo, monospace;
    white-space: pre-wrap;
  }
  .lean-panel {
    padding: 0.5rem 0.75rem;
    font-size: 0.88rem;
  }
  .hint {
    color: var(--accent);
    padding: 0 0.75rem;
  }
  .form {
    display: flex;
    flex-direction: column;
    gap: 0.8rem;
  }
  .form label {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }
  .form label.check {
    flex-direction: row;
    align-items: center;
    gap: 0.4rem;
  }
  .form small {
    color: var(--accent);
  }
  .form .row {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }
  .keys {
    border-collapse: collapse;
    width: 100%;
    font-size: 0.9rem;
  }
  .keys td {
    padding: 0.3rem 0.4rem;
    border-bottom: 1px solid var(--border);
  }
  kbd {
    font: 0.82rem ui-monospace, monospace;
    border: 1px solid var(--border);
    border-bottom-width: 2px;
    border-radius: 4px;
    padding: 0.05rem 0.35rem;
    white-space: nowrap;
  }
  @media (max-width: 60rem) {
    .tool span {
      display: none;
    }
  }
  @media (max-width: 50rem) {
    .body,
    .body.with-bottom {
      grid-template-columns: 1fr;
      grid-template-rows: 3fr 2fr;
    }
    .body.with-bottom {
      grid-template-rows: 3fr 2fr 2fr;
    }
    .code {
      border-right: 0;
      border-bottom: 1px solid var(--border);
    }
  }
</style>
