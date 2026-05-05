<!--
  Three-way text / image / audio segmented switch. Used at the top of
  the playground to pick the active modality.
-->
<script lang="ts">
  type Modality = 'text' | 'image' | 'audio';
  interface Props {
    selected: Modality;
    onSelect: (m: Modality) => void;
  }
  let { selected, onSelect }: Props = $props();

  const tabs: { id: Modality; label: string }[] = [
    { id: 'text', label: '⟨T⟩ Text' },
    { id: 'image', label: '⬡ Image' },
    { id: 'audio', label: '♪ Audio' },
  ];
</script>

<div class="mod-tabs" role="tablist" aria-label="Modality">
  {#each tabs as t (t.id)}
    <button
      type="button"
      role="tab"
      aria-selected={selected === t.id}
      class="mod-tab"
      class:active={selected === t.id}
      onclick={() => onSelect(t.id)}
    >
      {t.label}
    </button>
  {/each}
</div>

<style>
  .mod-tabs { display: flex; gap: 0.5rem; }
  .mod-tab {
    font-family: var(--mono);
    font-size: 0.8rem;
    padding: 0.35rem 0.9rem;
    border: 1px solid var(--ink);
    background: transparent;
    color: var(--ink);
    border-radius: 3px;
    cursor: pointer;
    transition: background 0.1s, color 0.1s;
  }
  .mod-tab:hover { background: var(--bg-2); }
  .mod-tab.active { background: var(--ink); color: var(--bg); }
</style>
