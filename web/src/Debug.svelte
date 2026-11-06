<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!--
  The debugger of the legal profile: choose a norm, give its inputs, and see every value and rule – for a rule its
  default and each exception tried, and which one decided. Exact, as the law computes; judgements are inputs too.
-->
<script lang="ts">
  import { untrack } from "svelte";
  import type { EvalStep, NormInfo, Verkodo } from "./lib/verkodo";

  let {
    vk,
    version,
    de,
    goTo,
  }: { vk: Verkodo | null; version: number; de: boolean; goTo: (file: number, from: number, to: number) => void } =
    $props();

  let norm = $state("");
  // Inputs per norm, as typed.
  let inputs = $state<Record<string, Record<string, string>>>({});
  let norms = $state<NormInfo[]>([]);

  // After every analysis: the norms again; the choice stays if the norm is still there.
  $effect(() => {
    void version;
    untrack(() => {
      norms = vk?.norms() ?? [];
      if (!norms.some((n) => n.id === norm)) norm = norms[0]?.id ?? "";
    });
  });
  // Every norm gets its own inputs, kept while switching between norms.
  $effect(() => {
    const n = norm;
    untrack(() => {
      if (n && !inputs[n]) inputs[n] = {};
    });
  });

  const current = $derived(norms.find((n) => n.id === norm));
  const steps = $derived.by<EvalStep[]>(() => {
    void version;
    return vk && norm ? vk.evaluate(norm, { ...(inputs[norm] ?? {}) }) : [];
  });
  const results = $derived(steps.filter((s) => s.kind !== "input" && s.kind !== "judgement"));

  const name = (id: string) => id.split(".").pop() ?? id;
</script>

<div class="debug">
  {#if norms.length === 0}
    <p class="hint">{de ? "Keine Norm in den Dateien – Debug wertet Normen aus." : "No norm in the files – debugging evaluates norms."}</p>
  {:else}
    <div class="inputs">
      <label class="norm">
        {de ? "Norm" : "Provision"}
        <select bind:value={norm}>
          {#each norms as n (n.id)}<option value={n.id}>{n.label ?? n.id}</option>{/each}
        </select>
      </label>
      {#if inputs[norm]}
      {@const given = inputs[norm]!}
      {#each current?.inputs ?? [] as i (i.name)}
        <label title={i.judgement ? (de ? "Wertung – begründen" : "Judgement – to be justified") : ""}>
          <span>{i.label || i.name}{#if i.judgement}<em> ({de ? "Wertung" : "judgement"})</em>{/if}</span>
          {#if i.type === "bool"}
            <select bind:value={given[i.name]}>
              <option value="">–</option>
              <option value="true">{de ? "ja" : "yes"}</option>
              <option value="false">{de ? "nein" : "no"}</option>
            </select>
          {:else}
            <input
              bind:value={given[i.name]}
              placeholder={i.type === "date" ? "2025-01-01" : i.type === "money" ? "25000 EUR" : "0"}
              spellcheck="false"
            />
          {/if}
        </label>
      {/each}
      {/if}
    </div>
    <table>
      <thead>
        <tr><th></th><th>{de ? "Wert" : "Value"}</th><th>{de ? "Wie" : "How"}</th></tr>
      </thead>
      <tbody>
        {#each results as s (s.id)}
          <tr>
            <td>
              <button class="link" onclick={() => goTo(s.file, s.from, s.to)}>
                <span class="kind">{s.kind}</span> {name(s.id)}
              </button>
              {#if s.label}<div class="label">{s.label}</div>{/if}
            </td>
            <td class:unknown={!s.known} class:yes={s.value === "true"} class:no={s.value === "false"}>{s.value}</td>
            <td class="how">
              {#if s.default !== null}
                <div>{de ? "Grundsatz" : "As a rule"}: {s.default}</div>
              {/if}
              {#each s.exceptions as [cond, decided], k (k)}
                <div class:decided>
                  {de ? "Ausnahme" : "Exception"} {k + 1}: {de ? "Bedingung" : "condition"} {cond}{#if decided}
                    – <strong>{de ? "entscheidet" : "decides"}</strong>{/if}
                </div>
              {/each}
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  {/if}
</div>

<style>
  .debug {
    display: grid;
    grid-template-columns: minmax(14rem, 1fr) minmax(0, 2fr);
    gap: 0.75rem;
    padding: 0.5rem 0.75rem;
    font-size: 0.85rem;
  }
  .inputs {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
  }
  label span,
  .norm {
    color: var(--accent);
  }
  input,
  select {
    font: inherit;
    padding: 0.2rem 0.35rem;
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--fg);
  }
  table {
    border-collapse: collapse;
    width: 100%;
    align-self: start;
  }
  th {
    text-align: left;
    color: var(--accent);
    font-weight: normal;
    border-bottom: 1px solid var(--border);
  }
  td {
    border-bottom: 1px solid var(--border);
    padding: 0.25rem 0.4rem;
    vertical-align: top;
  }
  td:nth-child(2) {
    font-family: ui-monospace, "SF Mono", Menlo, monospace;
    white-space: nowrap;
  }
  .unknown {
    color: var(--accent);
  }
  .yes {
    color: var(--good);
  }
  .no {
    color: var(--bad);
  }
  .how {
    color: var(--accent);
  }
  .decided {
    color: var(--fg);
  }
  .kind {
    font-size: 0.7rem;
    text-transform: uppercase;
    color: var(--accent);
  }
  .label {
    color: var(--accent);
  }
  .link {
    border: 0;
    background: transparent;
    color: var(--fg);
    padding: 0;
    cursor: pointer;
    font: inherit;
  }
  .hint {
    color: var(--accent);
    grid-column: 1 / -1;
  }
</style>
