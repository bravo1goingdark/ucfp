<!--
  Coloured byte grid + truncated hex display for fingerprint preview.
  Click the hex string to copy the full value via the parent's handler.
-->
<script lang="ts">
  interface Cell {
    color: string;
  }
  interface Props {
    cells: Cell[];
    hex: string;
    onCopy: (hex: string) => void;
  }
  let { cells, hex, onCopy }: Props = $props();
</script>

<div class="hex-grid" aria-label="Fingerprint byte visualization">
  {#each cells as cell (cell)}
    <div class="hex-cell" style:background={cell.color}></div>
  {/each}
</div>
<button
  type="button"
  class="hex-str copyable"
  title="Click to copy full hex ({hex.length} chars)"
  onclick={() => onCopy(hex)}
>{hex.slice(0, 64)}{hex.length > 64 ? '…' : ''}</button>

<style>
  .hex-grid {
    display: flex;
    flex-wrap: wrap;
    gap: 2px;
    margin-bottom: 0.75rem;
  }
  .hex-cell {
    width: 14px;
    height: 14px;
    border-radius: 2px;
    animation: pop-in 0.2s ease both;
  }
  @keyframes pop-in {
    from { transform: scale(0); opacity: 0; }
    to { transform: scale(1); opacity: 1; }
  }
  .hex-str {
    font-family: var(--mono);
    font-size: 0.6rem;
    color: var(--ink-2);
    word-break: break-all;
    margin-bottom: 0.75rem;
    padding: 0;
    background: none;
    border: 0;
    text-align: left;
    width: 100%;
  }
  .hex-str.copyable {
    cursor: pointer;
    transition: color 0.12s;
  }
  .hex-str.copyable:hover { color: var(--ink); }
  .hex-str.copyable:focus-visible {
    outline: 2px solid var(--accent-ink);
    outline-offset: 2px;
  }
</style>
