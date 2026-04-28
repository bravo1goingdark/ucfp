<script lang="ts">
  import { untrack } from 'svelte';
  import { goto } from '$app/navigation';
  import { page } from '$app/stores';
  import LineChart, { type Series } from '$components/charts/LineChart.svelte';
  import DataTable, { type Column } from '$components/DataTable.svelte';
  import EmptyState from '$components/EmptyState.svelte';
  import type { UsageEvent, UsagePoint, Modality } from '$lib/types/api';

  let { data } = $props();

  // ── chart series toggle ───────────────────────────────────────────────
  // Modality buttons toggle local visibility; nothing leaves this page.
  let visible = $state<Record<Modality, boolean>>({ text: true, image: true, audio: true });

  // Collapse points into per-day per-modality counts.
  const chartData = $derived.by<{ xLabels: string[]; series: Series[] }>(() => {
    const points: UsagePoint[] = data.usage?.points ?? [];
    if (points.length === 0) return { xLabels: [], series: [] };

    // Build sorted day list + per-modality map.
    const days = Array.from(new Set(points.map((p) => p.day))).sort();
    const byMod: Record<Modality, Map<string, number>> = {
      text: new Map(),
      image: new Map(),
      audio: new Map()
    };
    for (const p of points) byMod[p.modality].set(p.day, (byMod[p.modality].get(p.day) ?? 0) + p.count);

    const colors: Record<Modality, string> = {
      text: 'var(--accent-ink)',
      image: 'var(--ink-2)',
      audio: 'var(--muted)'
    };

    const series: Series[] = (['text', 'image', 'audio'] as Modality[])
      .filter((m) => visible[m])
      .map((m) => ({
        label: m,
        color: colors[m],
        values: days.map((d) => byMod[m].get(d) ?? 0)
      }));

    // Friendlier x labels — strip the YYYY- prefix when long.
    const xLabels = days.map((d) => d.slice(5));
    return { xLabels, series };
  });

  // ── filter state (sync with querystring) ───────────────────────────────
  // Initial values come from the SSR-loaded `data.filters`. Reading
  // through `untrack` keeps Svelte from flagging "captures only initial
  // value" — that capture is exactly what we want here, since the
  // querystring is the source of truth and the inputs are editable.
  let modalityFilter = $state(untrack(() => data.filters.modality ?? ''));
  let statusFilter = $state(untrack(() => data.filters.status ?? ''));
  let daysFilter = $state(untrack(() => data.filters.days ?? '30'));

  function applyFilters() {
    const qs = new URLSearchParams();
    if (modalityFilter) qs.set('modality', modalityFilter);
    if (statusFilter) qs.set('status', statusFilter);
    if (daysFilter && daysFilter !== '30') qs.set('days', daysFilter);
    const target = qs.toString() ? `${$page.url.pathname}?${qs}` : $page.url.pathname;
    goto(target, { invalidateAll: true });
  }

  function nextPage() {
    if (filteredEvents.length === 0) return;
    const lastId = filteredEvents[filteredEvents.length - 1].id;
    const qs = new URLSearchParams($page.url.search);
    qs.set('before', String(lastId));
    goto(`${$page.url.pathname}?${qs}`, { invalidateAll: true });
  }

  function firstPage() {
    const qs = new URLSearchParams($page.url.search);
    qs.delete('before');
    const tail = qs.toString();
    goto(tail ? `${$page.url.pathname}?${tail}` : $page.url.pathname, { invalidateAll: true });
  }

  // ── events table ─────────────────────────────────────────────────────
  // The /api/usage endpoint currently returns the most recent N events
  // and doesn't support modality/status/before filtering server-side, so
  // we slice the response client-side to honour the user's filters.
  const allEvents = $derived<UsageEvent[]>(data.usage?.summary.recentEvents ?? []);

  const filteredEvents = $derived.by<UsageEvent[]>(() => {
    let rows = allEvents;
    if (modalityFilter) rows = rows.filter((r) => r.modality === modalityFilter);
    if (statusFilter) rows = rows.filter((r) => String(r.status) === statusFilter);
    const before = $page.url.searchParams.get('before');
    if (before) {
      const beforeId = Number(before);
      if (Number.isFinite(beforeId)) rows = rows.filter((r) => r.id < beforeId);
    }
    return rows.slice(0, 50);
  });

  function fmtTime(unixSec: number): string {
    return new Date(unixSec * 1000).toLocaleString();
  }

  const columns: Column<UsageEvent>[] = [
    { key: 'time', label: 'Time', get: (r) => fmtTime(r.createdAt) },
    { key: 'modality', label: 'Modality', get: (r) => r.modality },
    { key: 'algorithm', label: 'Algorithm', get: (r) => r.algorithm ?? '—' },
    { key: 'status', label: 'Status', numeric: true, get: (r) => r.status },
    { key: 'latency', label: 'Latency', numeric: true, get: (r) => `${r.latencyMs} ms` },
    { key: 'bytes', label: 'Bytes in', numeric: true, get: (r) => r.bytesIn }
  ];

  const noBackend = $derived(data.usage === null);
  const hasBefore = $derived(($page.url.searchParams.get('before') ?? '') !== '');
</script>

<svelte:head><title>Usage — UCFP</title></svelte:head>

<section class="dash-section">
  <header class="dash-section-head">
    <h1>Usage</h1>
    <p class="muted">Your fingerprinting throughput over time.</p>
  </header>

  {#if noBackend}
    <EmptyState
      heading="Usage data is not available yet"
      description="The usage API is not yet provisioned. Once it is, your charts will appear here."
    />
  {:else}
    <div class="usage-toolbar">
      {#each ['text', 'image', 'audio'] as m (m)}
        <button
          type="button"
          class="toggle"
          class:on={visible[m as Modality]}
          onclick={() => (visible[m as Modality] = !visible[m as Modality])}
          aria-pressed={visible[m as Modality]}
        >{m}</button>
      {/each}
    </div>

    {#if chartData.series.length > 0}
      <div class="chart-frame">
        <LineChart
          series={chartData.series}
          xLabels={chartData.xLabels}
          yAxisLabel="requests per day"
        />
      </div>
    {:else}
      <p class="muted">No usage in the selected window.</p>
    {/if}
  {/if}
</section>

<section class="dash-section">
  <header class="dash-section-head">
    <h2>Events</h2>
    <p class="muted">Filterable, paginated 50 rows at a time.</p>
  </header>

  <form class="usage-filters" onsubmit={(e) => { e.preventDefault(); applyFilters(); }}>
    <label>
      <span>Modality</span>
      <select bind:value={modalityFilter}>
        <option value="">All</option>
        <option value="text">text</option>
        <option value="image">image</option>
        <option value="audio">audio</option>
      </select>
    </label>
    <label>
      <span>Status</span>
      <select bind:value={statusFilter}>
        <option value="">All</option>
        <option value="200">200 OK</option>
        <option value="400">400 errors</option>
        <option value="429">429 rate-limited</option>
        <option value="500">500 errors</option>
      </select>
    </label>
    <label>
      <span>Range</span>
      <select bind:value={daysFilter}>
        <option value="7">7 days</option>
        <option value="30">30 days</option>
        <option value="90">90 days</option>
      </select>
    </label>
    <button type="submit" class="btn alt">Apply</button>
  </form>

  {#if filteredEvents.length === 0}
    <EmptyState
      heading="No events match these filters"
      description="Try widening the date range or clearing the modality/status filters."
    />
  {:else}
    <DataTable {columns} rows={filteredEvents} rowKey={(r) => r.id} caption="Usage events" />
    <div class="pager">
      <button type="button" class="link-btn" onclick={firstPage} disabled={!hasBefore}>← Newest</button>
      <button
        type="button"
        class="link-btn"
        onclick={nextPage}
        disabled={filteredEvents.length < 50}
      >Older →</button>
    </div>
  {/if}
</section>
