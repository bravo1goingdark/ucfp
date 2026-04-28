<script lang="ts">
  import Donut from '$components/charts/Donut.svelte';
  import Sparkline from '$components/charts/Sparkline.svelte';
  import DataTable, { type Column } from '$components/DataTable.svelte';
  import EmptyState from '$components/EmptyState.svelte';
  import type { UsageEvent, UsagePoint, Modality } from '$lib/types/api';

  let { data } = $props();

  // Layout-load gives us the 7-day window; +page.ts supplies 30-day.
  const usage30 = $derived(data.usage);
  const summary7 = $derived(data.summary);

  const recent = $derived<UsageEvent[]>(summary7?.summary.recentEvents ?? []);

  // Modality breakdown — prefer 30-day if we have it, else fall back to 7-day.
  const modalityCounts = $derived.by(() => {
    const src = usage30?.summary.modalityBreakdown ?? summary7?.summary.modalityBreakdown;
    if (!src) return [] as { label: string; value: number }[];
    return (Object.entries(src) as [Modality, number][]).map(([k, v]) => ({
      label: k,
      value: v
    }));
  });

  // 7-day sparkline of total daily requests.
  const sparkValues = $derived.by<number[]>(() => {
    const points = summary7?.points ?? [];
    if (points.length === 0) return [];
    const byDay = new Map<string, number>();
    for (const p of points as UsagePoint[]) {
      byDay.set(p.day, (byDay.get(p.day) ?? 0) + p.count);
    }
    return Array.from(byDay.values());
  });

  const totalThisMonth = $derived(usage30?.summary.totalRequests ?? 0);
  const errorCount = $derived(usage30?.summary.errorCount ?? summary7?.summary.errorCount ?? 0);

  const recentColumns: Column<UsageEvent>[] = [
    { key: 'time', label: 'Time', get: (r) => new Date(r.createdAt * 1000).toLocaleString() },
    { key: 'modality', label: 'Modality', get: (r) => r.modality },
    { key: 'algorithm', label: 'Algorithm', get: (r) => r.algorithm ?? '—' },
    { key: 'status', label: 'Status', numeric: true, get: (r) => r.status },
    { key: 'latency', label: 'Latency', numeric: true, get: (r) => `${r.latencyMs} ms` },
    { key: 'bytes', label: 'Bytes', numeric: true, get: (r) => r.bytesIn }
  ];

  const noBackend = $derived(summary7 === null && usage30 === null);
</script>

<svelte:head><title>Dashboard — UCFP</title></svelte:head>

<section class="dash-section">
  <header class="dash-section-head">
    <h1>Overview</h1>
    <p class="muted">Your fingerprinting activity in the past 30 days.</p>
  </header>

  {#if noBackend}
    <EmptyState
      heading="Usage data is not yet available"
      description="The usage API has not been provisioned yet. Once it is, your overview will appear here."
    />
  {:else}
    <div class="dash-grid">
      <article class="dash-card">
        <div class="card-label">Requests · 30d</div>
        <div class="card-value">{totalThisMonth.toLocaleString()}</div>
        <div class="card-foot">
          <Sparkline values={sparkValues} label="Last 7 days" />
        </div>
      </article>

      <article class="dash-card">
        <div class="card-label">Modality breakdown</div>
        <Donut data={modalityCounts} size={140} />
      </article>

      <article class="dash-card">
        <div class="card-label">Errors · 30d</div>
        <div class="card-value">{errorCount.toLocaleString()}</div>
        <div class="card-foot muted">non-2xx responses</div>
      </article>

      <article class="dash-card">
        <div class="card-label">Account</div>
        <div class="card-row"><span class="muted">Tenant</span><span>{data.user.tenantId}</span></div>
        <div class="card-row"><span class="muted">Email</span><span>{data.user.email}</span></div>
      </article>
    </div>
  {/if}
</section>

<section class="dash-section">
  <header class="dash-section-head">
    <h2>Recent activity</h2>
    <p class="muted">The last 10 events across all of your keys.</p>
  </header>
  {#if recent.length === 0}
    <EmptyState
      heading="No recent activity"
      description="Once you make requests against /api/fingerprint or your custom keys, events will show up here."
    >
      {#snippet cta()}
        <a class="btn alt" href="/dashboard/keys">Create an API key →</a>
      {/snippet}
    </EmptyState>
  {:else}
    <DataTable
      columns={recentColumns}
      rows={recent.slice(0, 10)}
      rowKey={(r) => r.id}
      caption="Recent fingerprint events"
    />
  {/if}
</section>
