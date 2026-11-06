<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!-- A dialog over the editor: closes with Escape, the close button or a click beside it. -->
<script lang="ts">
  import type { Snippet } from "svelte";
  import { icons } from "./lib/icons";

  let { title, onclose, children }: { title: string; onclose: () => void; children: Snippet } = $props();
  let dialog: HTMLDialogElement;

  $effect(() => {
    dialog.showModal();
  });
</script>

<dialog
  bind:this={dialog}
  onclose={onclose}
  onclick={(e) => {
    if (e.target === dialog) dialog.close();
  }}
>
  <header>
    <h2>{title}</h2>
    <button class="icon" onclick={() => dialog.close()} aria-label="Close">{@html icons.close}</button>
  </header>
  <div class="content">{@render children()}</div>
</dialog>

<style>
  dialog {
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 0;
    background: var(--bg);
    color: var(--fg);
    width: min(36rem, calc(100vw - 2rem));
    max-height: calc(100vh - 4rem);
  }
  dialog::backdrop {
    background: rgb(0 0 0 / 0.35);
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.6rem 0.9rem;
    border-bottom: 1px solid var(--border);
  }
  h2 {
    font-size: 1rem;
    margin: 0;
  }
  .content {
    padding: 0.9rem;
    overflow: auto;
  }
  .icon {
    border: 0;
    padding: 0.2rem;
    display: inline-flex;
    cursor: pointer;
    background: transparent;
    color: inherit;
  }
</style>
