<!--
  Algorithm grid + description + needs-input badges. Driven by the
  per-modality lists owned by the playground page so adding a new
  algorithm is a one-line change there, not here.
-->
<script lang="ts">
  interface Props {
    /** Available algorithm IDs for the active modality. */
    options: readonly string[];
    /** Currently selected algorithm. */
    selected: string;
    /** Display labels per ID. Falls back to the raw ID. */
    labels?: Record<string, string>;
    /** Description shown beneath the grid for `selected`. */
    descriptions?: Record<string, string>;
    /** IDs that require an API key (badge + tooltip). */
    needsApiKey?: ReadonlySet<string>;
    /** IDs that require a model path. */
    needsModel?: ReadonlySet<string>;
    /** Called when the user picks a different algorithm. */
    onSelect: (id: string) => void;
  }
  let {
    options,
    selected,
    labels = {},
    descriptions = {},
    needsApiKey = new Set<string>(),
    needsModel = new Set<string>(),
    onSelect,
  }: Props = $props();
</script>

<div class="pane-label" id="algo-label">Algorithm</div>
<div class="algo-grid" role="group" aria-labelledby="algo-label">
  {#each options as alg (alg)}
    <button
      type="button"
      class="algo-btn"
      class:selected={selected === alg}
      class:needs-input={needsApiKey.has(alg) || needsModel.has(alg)}
      onclick={() => onSelect(alg)}
      aria-pressed={selected === alg}
      title={needsApiKey.has(alg)
        ? 'Requires API key'
        : needsModel.has(alg)
          ? 'Requires model path'
          : ''}
    >
      {labels[alg] ?? alg}
    </button>
  {/each}
</div>

{#if descriptions[selected]}
  <p class="alg-desc">{descriptions[selected]}</p>
{/if}
