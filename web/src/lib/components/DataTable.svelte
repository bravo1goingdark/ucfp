<script module lang="ts">
  import type { Snippet } from 'svelte';

  export interface Column<R> {
    key: string;
    label: string;
    /** Optional render snippet — receives the row. */
    cell?: Snippet<[R]>;
    /** Static accessor; ignored if `cell` is provided. */
    get?: (row: R) => string | number | null | undefined;
    /** Render numbers in monospace + right-align. */
    numeric?: boolean;
    /** CSS width hint, e.g. `'120px'` or `'20%'`. */
    width?: string;
  }
</script>

<script lang="ts" generics="T">
  interface Props {
    columns: Column<T>[];
    rows: T[];
    /** Stable key extractor for `{#each}` keying. */
    rowKey?: (row: T, i: number) => string | number;
    caption?: string;
  }

  let { columns, rows, rowKey, caption }: Props = $props();
</script>

<div class="data-scroll">
  <table class="data">
    {#if caption}<caption class="sr-only">{caption}</caption>{/if}
    <thead>
      <tr>
        {#each columns as col (col.key)}
          <th
            scope="col"
            class:num={col.numeric}
            style={col.width ? `width:${col.width}` : undefined}
          >{col.label}</th>
        {/each}
      </tr>
    </thead>
    <tbody>
      {#each rows as row, i (rowKey ? rowKey(row, i) : i)}
        <tr>
          {#each columns as col (col.key)}
            <td class:num={col.numeric}>
              {#if col.cell}
                {@render col.cell(row)}
              {:else if col.get}
                {col.get(row) ?? ''}
              {/if}
            </td>
          {/each}
        </tr>
      {/each}
    </tbody>
  </table>
</div>
