// SPDX-License-Identifier: AGPL-3.0-or-later
// The editor's settings, kept in this browser (localStorage) – a convenience: without storage the defaults apply.
import type { Style } from "./verkodo";

export type Theme = "light" | "dark" | "system";

export interface Settings {
  /** The language of the interface and of labels and texts (from translations where there are some). */
  lang: string;
  theme: Theme;
  /** How formatting writes a file. */
  style: Style;
  /** The widest a line of text may be when formatting – longer texts are broken into lines; null: none. */
  width: number | null;
  /** Formatting when switching to another file. */
  formatOnSwitch: boolean;
  fontSize: number;
  wrap: boolean;
}

const KEY = "verkodo.editor.settings";
const DEFAULTS: Settings = {
  lang: "de",
  theme: "system",
  style: "delimiters",
  width: null,
  formatOnSwitch: false,
  fontSize: 14,
  wrap: true,
};

function load(): Settings {
  try {
    const raw = localStorage.getItem(KEY);
    return raw ? { ...DEFAULTS, ...(JSON.parse(raw) as Partial<Settings>) } : { ...DEFAULTS };
  } catch {
    return { ...DEFAULTS };
  }
}

export const settings = $state<Settings>(load());

export function saveSettings(): void {
  try {
    localStorage.setItem(KEY, JSON.stringify(settings));
  } catch {
    // Private mode or full storage: the settings hold until the page is closed.
  }
}

/** Applies the theme to the page: `data-theme` for the app's colours, `color-scheme` for the browser's. */
export function applyTheme(theme: Theme): void {
  const root = document.documentElement;
  if (theme === "system") root.removeAttribute("data-theme");
  else root.setAttribute("data-theme", theme);
}
