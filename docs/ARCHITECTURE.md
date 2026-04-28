# UCFP minimal Rust stack: one binary, one disk

## 1. Recommended stack

Ten new direct dependencies. Everything pure Rust except where noted; SDK-side `bytemuck` is already on the dep graph and not counted as new.

| Concern | Crate | Version (Apr 2026) | Rationale |
|---|---|---|---|
| HTTP server, routing, extractors | **axum** | 0.8.9 | Tokio-team maintained, Tower-native, `#![forbid(unsafe_code)]`. The 0.8 line is stable since Jan 2025; only mechanical migration cost from 0.7 (path syntax `/{id}`, `Sync` handlers). No reason to consider actix-web — its actor middleware buys nothing at this scale. |
| Async runtime | **tokio** | 1.47.x LTS | Pin to the `~1.47` LTS line, supported through Sep 2026; `1.51` LTS through Mar 2027 if you want longer reach. Avoid 1.43.x (LTS ended Mar 2026). |
| Auth, body limits, request-id, timeouts | **tower-http** | 0.6.6 | `ValidateRequestHeaderLayer::bearer("…")` is the idiomatic static-token middleware — constant-time-ish compare, correct `WWW-Authenticate`, audited. Enable feature `validate-request` only. |
| Persistence (fingerprints + metadata + posting lists) | **redb** | 3.0.0 | Pure Rust copy-on-write B-tree, MVCC (1 writer / N readers), XXH3-128 page checksums, single-fsync ACID commit, file format stable since 1.0. Single file. See §2. |
| Vector ANN | **hnsw_rs** | 0.3.4 | Pure Rust HNSW, rich distance set (incl. cosine, dot), parallel build, dump/reload, mmap of dumped vectors. Used over `instant-distance` 0.6.1 (no built-in metrics, generic distance pays a virtual call) and over `usearch` (C++ FFI). |
| SIMD f32 dot product (brute-force path) | **pulp** | 0.22.2 | Pure Rust, runs on **stable** (`std::simd` is still nightly per rust-lang/portable-simd #364), built-in runtime multiversioning across SSE4 / AVX2 / AVX-512 / NEON via `Arch::dispatch`. Powers `faer`. |
| Metadata bitmap filters | **roaring** | 0.11.2 | Pure Rust RoaringBitmap. One bitmap per facet (tag, content_type, date_bucket); intersect at query time; pass result as allow-list to HNSW or brute-force scan. |
| Metadata serialization | **rkyv** | 0.8.16 | Zero-copy archived metadata access (no per-query alloc), variable-length, optional fields. One leading byte = schema version; reordering = new version + migration. Fingerprints themselves stay raw `bytemuck::cast_slice`. |
| Logs (JSON to stdout) | **tracing-subscriber** | 0.3.22 | `fmt().json().init()` → NDJSON to stdout, scraped by journald/docker. ≥0.3.20 mandatory (RUSTSEC-2025-0055 ANSI patch). `tracing` 0.1 façade is transitive. |
| Metrics (`/metrics` pull) | **metrics-exporter-prometheus** | 0.18.1 | Build a recorder, render from an axum route — no second listener, no openssl. **Set `default-features = false`** to drop the `push-gateway` openssl FFI. |

Transitive but free: `hyper` 1.8.x, `tower` 0.5.2, `serde` 1.0.228, `serde_json` 1.0.149, `bytemuck` 1.25, `tracing` 0.1.x, `metrics` 0.24. Input validation via `garde` 0.22 is recommended only if you outgrow hand-rolled `serde` + `Result` returns; not counted in the budget.

## 2. Persistence decision

**One store: redb 3.0, single file.** All four artifacts — fingerprint bytes (`bytemuck::cast_slice`), metadata (`rkyv` archived), posting lists for optional BM25 (`fst` + roaring-encoded values), and the persisted HNSW dump — live in one redb database with separate tables.

Why one is enough at the stated load. 100 M records × ~500 B fingerprint + ~200 B metadata ≈ 70 GB. redb's COW B-tree handles this comfortably on one NVMe. Reads are MVCC-snapshotted and lock-free against the single writer, which matches the workload (read-heavy queries, batched writes from ingest). Crash safety is one fsync per commit with checksums on every page; non-durable mode exists for batch ingest if you accept replay.

Why not the alternatives.

- **sled** — last release `1.0.0-alpha.124` (Oct 2024), still alpha since 2023, README still warns of unstable on-disk format, author Tyler Neely now points users at RocksDB or LMDB. **Dormant. Do not use.**
- **fjall** — pure Rust LSM, currently 3.1 (Mar 2026). Excellent write throughput (Fjall 3.0 post: WA ≈ 2–3 with key-value separation, beats redb on bulk writes). But maintainer marvin-j97 stated v3 is the capstone and "active development on new features will mostly wind down going into 2026." Choose only if sustained ingest > ~30 k writes/s with values >1 KiB. We're not.
- **rust-rocksdb (zaidoon1 fork)** — 0.46.0 (Feb 2026), actively tracking upstream. **C++ FFI**: cross-compile to musl and aarch64 routinely fails on `__dso_handle`, missing `libstdc++`, `bits/libc-header-start.h` (issues #174, #440, #550, #635). Adds 150–300 MB to image size and 2–10 min to build. Only justified if you need its operational tooling.
- **heed** (LMDB FFI) — 0.22.0, mature but slow-moving, single-writer, mmap-based. Functionally fine but pulls C and gives up nothing redb doesn't already provide.
- **native_db** 0.8.2 — typed-model layer over redb 2.x; README admits "API not stable yet." Skip; drop down to redb directly.

Benchmark anchor (cberner/redb README, Aug 2025, Ryzen 9950X3D + Samsung 9100 PRO NVMe, ms): individual writes — **redb 920**, lmdb 1598, rocksdb 2432, sled 2701, fjall 3488. Batch writes invert: fjall 353, rocksdb 451, lmdb 942, sled 853, redb 1595. Bulk load: lmdb 9232, rocksdb 13969, redb 17063, fjall 18619. The Fjall 3.0 post (Jan 2026) confirms the same shape with first-party numbers. No neutral 2024–2026 third-party benchmark exists; both first-party harnesses agree on the ordering. Translation: **redb wins single-row write latency and read latency; fjall wins write throughput; choose redb for our workload.**

Write amplification: redb's COW root-to-leaf rewrite is ~5–20× for small values at depth 4 and 4 KiB pages. fjall LSM amortizes 10–30× without KV-separation, 2–3× with. At our write rate (target single-host, < 10 k/s steady), the B-tree's predictable WA wins on tail latency.

## 3. Vector ANN decision

**Below ~1 M vectors at dim 512–1024 → brute-force cosine with `pulp` + `rayon`. At ≥ 1 M → `hnsw_rs`.** Threshold scales with dimension: ~3 M for dim 256, ~500 k for dim 1024.

The arithmetic. A 768-d f32 cosine on AVX-512 is ~24 cycles ≈ 8 ns of compute, but is memory-bandwidth-bound at ~30–60 ns hot-cache, ~80–150 ns from L3/DRAM (consistent with SimSIMD measurements at 1536-d on Sapphire Rapids and Lemire's bandwidth analysis). Amortized across 16 cores at ~5–8 ns/vector:

- 100 k → 0.5–1 ms
- 1 M → **5–10 ms** (well under 50 ms p99)
- 10 M → 50–100 ms (over budget)
- 100 M → 500–1000 ms (impossible)

HNSW with m=16, ef_search=128 issues a few hundred distance computations per query regardless of N: 0.1–2 ms even at 100 M. `hnsw_rs` reports ~15 k QPS @ recall 0.99 on SIFT1M single-threaded.

External anchors. Qdrant's docs put their internal `full_scan_threshold` near 10 k vectors, but that figure includes payload/filter overhead. usearch's docs put the practical brute-force ceiling "in the millions." Pinecone published 79 ms at 1.2 M × 768-d managed (network included) — pure in-process Rust beats that.

The defensible recommendation. Code both paths. Use brute-force + pulp + rayon on the f32 array stored in a redb blob (or mmap'd file alongside) until corpus crosses **1 M for dim ≥ 512** or **3 M for dim ≤ 256**. Above that, build `hnsw_rs` lazily and persist its dump alongside. This gets you exact recall at small N (huge in early production), and you eat HNSW's recall trade-off only when you must.

Why not `usearch` despite higher peak throughput: it links C++ via `cxx-build`, breaking the pure-Rust musl/aarch64 cross-compile story. Reconsider only if you adopt int8 / bf16 quantization (usearch's strongest card) at >50 M vectors. `instant-distance` 0.6.1 is pure Rust but its generic-only `Point::distance` defeats SIMD specialization. `space` provides only traits and a linear-search struct — useful as the brute-force trait surface, not as an index.

SIMD primitive: `pulp` is the right default. `wide` is fine if you commit to `-C target-cpu=x86-64-v3` and one binary per arch. `std::simd` is still nightly. `simsimd` is the FFI ceiling and only worth it if you adopt its f16/bf16/i8 paths.

## 4. Hybrid retrieval

**Default: vector + metadata pre-filter via roaring. Add BM25 only when text fields actually demand it. Tantivy is overkill at this scale.**

Metadata filtering. For each filterable facet (tag, content_type, source_id, date bucket), maintain `RoaringBitmap` keyed by internal u32 doc-id, persisted as a redb value. At query time intersect the facet bitmaps; the result is the candidate allow-list. If `popcount < ~10× k` (say <1 000 for k=10), brute-force inside the bitmap — exact, fast, no graph traversal. Otherwise pass the bitmap into the HNSW search loop and skip non-set candidates during expansion. This is the same recipe Weaviate's ACORN, Qdrant's filterable HNSW, and Vespa's pre-filter mode use; Weaviate's blog confirms recall stays at unfiltered levels under both extremes.

BM25 without tantivy. The math is ~30 lines: store `term_dict` as an `fst::Map<term, term_id>` (BurntSushi `fst` 0.4.7, mmap-friendly), `postings` as a redb table `term_id → roaring(doc_id)` plus a parallel `term_id → Vec<(doc_id, tf)>` for scoring, and `doc_lens` as a redb table `doc_id → u32`. Compute `Σ idf · ((tf·(k1+1)) / (tf + k1·(1 - b + b·|D|/avgdl)))` at query time. The `bm25` crate (Michael-JB) and `bm25-vectorizer` (ep9io, Sep 2025) are tiny pure-Rust options if you prefer not to roll your own; both fit in-RAM term universes up to a few GB.

When tantivy is justified. Adopt **tantivy 0.25.0** only when you need (a) phrase / proximity / fuzzy / regex queries, (b) faceted aggregation, or (c) >100 M short-text documents where the FST + roaring approach blows the page cache. Cost: ~10 MB binary bloat, multi-file segment directory, separate IndexWriter heap (50 MB–1 GB). Until then, stay with fst + roaring in the same redb file.

Fusion. Use Reciprocal Rank Fusion: `score(d) = Σ_i 1/(60 + rank_i(d))`, where `i` ranges over the active rankers (vector, BM25, optional rerank). k=60 is the universal default (Azure AI Search, Elasticsearch, OpenSearch, Qdrant, Weaviate). OpenSearch's RRF benchmark reports 91% recall@10 with RRF vs 78% dense-only on RAG corpora. **Do not pull a crate** — implementation is ~20 lines of `HashMap<DocId, f32>`, score-normalization-free.

## 5. Trait boundaries

Three traits in `ucfp-core`. Concrete embedded impls live in `ucfp-index` and `ucfp-ingest`; managed-service impls (Qdrant, S3, a remote reranker) can be added in separate crates without touching the matcher.

```rust
// Where new fingerprint records come from. Embedded impl reads
// from axum POST handlers; later impls can pull from S3, GCS, or a queue.
#[async_trait::async_trait]
pub trait IngestSource: Send + Sync {
    type Item: Send + 'static;
    type Error: std::error::Error + Send + Sync + 'static;

    async fn next_batch(&self, max: usize) -> Result<Vec<Self::Item>, Self::Error>;
    async fn ack(&self, ids: &[u64]) -> Result<(), Self::Error>;
}
```

```rust
// Storage + ANN behind one interface. Embedded impl is redb + hnsw_rs +
// roaring; graduation impl can be qdrant-client or lancedb.
#[async_trait::async_trait]
pub trait IndexBackend: Send + Sync {
    type Filter: Send + Sync; // Roaring expression locally; JSON predicate remotely.
    type Error: std::error::Error + Send + Sync + 'static;

    async fn upsert(&self, batch: &[Record]) -> Result<(), Self::Error>;
    async fn delete(&self, ids: &[u64]) -> Result<(), Self::Error>;

    async fn knn(
        &self,
        query: &[f32],
        k: usize,
        filter: Option<&Self::Filter>,
    ) -> Result<Vec<Hit>, Self::Error>;

    async fn bm25(
        &self,
        terms: &[&str],
        k: usize,
        filter: Option<&Self::Filter>,
    ) -> Result<Vec<Hit>, Self::Error>;

    async fn flush(&self) -> Result<(), Self::Error>;
}
```

```rust
// Optional second-stage rerank. Embedded impl may be a no-op or a small
// cross-encoder via `candle` / ONNX; later impls can call a managed service.
#[async_trait::async_trait]
pub trait Reranker: Send + Sync {
    type Error: std::error::Error + Send + Sync + 'static;
    async fn rerank(&self, query: &Query, hits: Vec<Hit>) -> Result<Vec<Hit>, Self::Error>;
}
```

The matcher composes these: `IndexBackend::knn` ∥ `IndexBackend::bm25` → RRF fusion → optional `Reranker::rerank`. None of the three traits leaks redb, hnsw_rs, or roaring types through their public API, which is the precondition for swapping in a managed backend later.

## 6. Scale-up triggers

| Metric | Threshold | What to add | Why this threshold |
|---|---|---|---|
| Vector p99 query latency | > 50 ms at default profile, > 100 ms at stretch | Switch brute-force → `hnsw_rs`; if already on HNSW, add int8 quantization (usearch) or shard by tenant | Brute-force at 16 cores hits ~50 ms around 10 M × 768-d; HNSW buys two orders of magnitude headroom |
| Vector corpus size | > 100 M records or > 500 GB on disk | Migrate index to **Qdrant** (single binary, Rust-native) keeping redb as the source-of-truth blob store | redb B-tree depth and page-cache pressure on one NVMe degrade tail latency past this point |
| Sustained write rate | > ~30 k inserts/s with values > 1 KiB | Swap redb for **fjall** under the same `IndexBackend` impl | LSM amortizes WA at this regime; redb COW dominates commit cost |
| Multi-tenant isolation requirement | Any contractual data isolation between tenants | Add JWT auth, per-tenant database file or namespace; consider per-tenant Qdrant collections | Static bearer + one redb file cannot enforce isolation; multi-tenancy is a security property, not a perf one |
| HA / failover requirement | RTO < 1 hr or active-active needed | Promote to **Qdrant cluster** or **LanceDB on S3**, run UCFP stateless in front | Embedded single-process stores have no replication story |
| Full-text query complexity | Phrase / fuzzy / regex / faceting required | Add **tantivy 0.25**, keep redb for blobs | fst + manual BM25 covers tag/title scoring, not linguistic queries |
| Ingest decoupling | Producers exceed UCFP capacity for > 10 min sustained, or you need replay | Add an **S3 prefix as `IngestSource`**, not Kafka | Object storage is operationally free vs. running brokers |
| Per-host memory pressure | Working set > 70 % RAM | Enable mmap for HNSW dump and vector blob; quantize to int8 | redb is mmap-friendly; hnsw_rs supports `NoData` reload |

The intended graduation path is redb + hnsw_rs (day one) → redb + Qdrant single-node (~100 M) → Qdrant cluster + S3 blob store (multi-host). LanceDB is the alternative graduation when columnar/multimodal access patterns dominate; its embeddable `lancedb` 0.26.2 and managed offering share the same Rust API, which preserves the `IndexBackend` impl across the jump.

## 7. What to explicitly NOT add

**Kafka / Redpanda / NATS JetStream.** Brokered queueing buys nothing at <10 k events/s on one host. axum's bounded `mpsc` channel between handlers and an ingest worker is sufficient for backpressure; persistence is redb's job. If you ever need cross-process replay, an S3 prefix scanned by the `IngestSource` trait is one-tenth the operational cost of running brokers and gives you free object-store durability. Reconsider only at multi-host with >50 k events/s.

**Qdrant / Milvus / Vespa on day one.** Qdrant is the *graduation* target precisely because it is the right shape — but adding it before crossing 100 M vectors means running and backing up another stateful service for no latency win. Milvus pulls etcd + MinIO + Pulsar; Vespa is a JVM cluster. Both are stateful-platform commitments, not libraries. The only correct day-one vector store is in-process.

**Postgres / pgvector.** Tempting because "we already run it." You don't. Adding Postgres adds a process, a backup story, a connection-pool tuning task, a schema-migration tool, and ~80 MB of memory at idle, in exchange for slower vector search than `hnsw_rs` and worse blob throughput than redb. If a future product requirement demands SQL ad-hoc queries over metadata, expose a read-only DuckDB view over the redb-derived parquet snapshot — still no Postgres process.

**OpenTelemetry collector + Jaeger + Tempo.** OTLP buys distributed-trace correlation across services. UCFP is one binary. `tracing-subscriber` JSON to stdout, scraped by journald or docker, gives you the same data with zero collector to run, version-pin, secure, or restart. `metrics-exporter-prometheus` exposes `/metrics` for pull-style scraping — the user explicitly approved pull, and that's all Prometheus / VictoriaMetrics / Grafana Alloy need. Add OTLP only if/when UCFP is split across services and a trace must span them.

**Tantivy on day one.** ~10 MB binary bloat, ~50 deps, multi-file segment directory, separate IndexWriter heap. For tags and titles on 10–100 M docs, `bm25` crate or fst + roaring inside redb costs roughly 200 lines of code and zero new external state. Promote to tantivy only when phrase, fuzzy, regex, or faceted queries become product requirements.

**rust-rocksdb.** Mature, but C++ FFI defeats the pure-Rust musl + aarch64 cross-compile story that makes single-binary deployment trivial. redb covers the durability and read-latency requirements; fjall covers the write-throughput escape hatch. RocksDB is justified only when you need its specific operational tooling (column families, backups via Checkpoint API, tiered storage) — none of which UCFP needs at the stated load.

**JWT, RBAC, OAuth on day one.** Static bearer via `tower_http::ValidateRequestHeaderLayer::bearer` is one line, audited, and enough for an internal integrator. JWT and `axum-extra::TypedHeader` come in only with the multi-tenant trigger in §6.

**`std::simd` / nightly toolchain.** Still nightly in 2026 (rust-lang/portable-simd #364). Locking to nightly to chase a 10–20 % SIMD win is a poor trade against `pulp`'s runtime dispatch on stable. Revisit when stabilization actually lands.

**Heavy serialization (rkyv) for the fingerprints themselves.** They're already `bytemuck::Pod` fixed-size structs — `bytemuck::cast_slice` is a `transmute`, faster than rkyv, mmap-friendly, no derive. Reserve rkyv for the variable-length metadata side.

## 8. Addenda

Five clarifications added during architecture review. Each is a concrete decision the original sections leave implicit.

### 8.1. Per-tenant key prefixing is day-one schema, not a scale-up trigger

§6 lists per-tenant isolation under "scale-up triggers", but the schema decision must be made on day one — retrofitting tenant prefixes onto a populated redb file is a full export/import. Bake it in from the first commit:

```text
redb table  fingerprints   key = (tenant_id: u32, record_id: u64)  → bytemuck::cast_slice
redb table  metadata       key = (tenant_id: u32, record_id: u64)  → rkyv archived
redb table  facets         key = (tenant_id: u32, facet_id: u32)   → roaring bitmap
```

Single-tenant deployments use `tenant_id = 0`. Adding tenants later is a write of new prefixed keys; no migration. Per-tenant range scans become free (`(tid, 0) ..= (tid, u64::MAX)`).

### 8.2. Backup is `cp` while the writer is open

redb's MVCC + COW means a filesystem-level snapshot taken while readers/writers are active is consistent — the snapshot sees only fully-committed pages because the writer rewrites root-to-leaf and only swaps the new root atomically.

```bash
# Hourly cron, or per-deploy. No quiesce needed.
cp ucfp.redb backups/ucfp-$(date -u +%Y%m%dT%H%M%SZ).redb
# Even cheaper on ZFS / Btrfs:
btrfs subvolume snapshot -r /var/lib/ucfp /var/lib/ucfp/.snap-$(date -u +%s)
```

Restore is `cp` in the other direction. No replication, no logical-backup tool, no WAL shipping. Promote to a real replication story only at the §6 HA trigger.

### 8.3. Production hygiene checklist (axum + tower)

The doc names the auth middleware but skips the four other limits that separate "demo" from "deployable". Wire all five before exposing the binary:

```rust
use std::time::Duration;
use tower::limit::ConcurrencyLimitLayer;
use tower_http::{
    limit::RequestBodyLimitLayer,
    timeout::TimeoutLayer,
    trace::TraceLayer,
    validate_request::ValidateRequestHeaderLayer,
};

let app = Router::new()
    .merge(routes())
    .layer(ValidateRequestHeaderLayer::bearer(&token))     // §1: auth
    .layer(RequestBodyLimitLayer::new(16 * 1024 * 1024))   // 16 MiB cap
    .layer(ConcurrencyLimitLayer::new(512))                // in-flight cap
    .layer(TimeoutLayer::new(Duration::from_secs(10)))     // per-request deadline
    .layer(TraceLayer::new_for_http());                    // request_id + spans

// And graceful shutdown:
axum::serve(listener, app)
    .with_graceful_shutdown(async { tokio::signal::ctrl_c().await.ok(); })
    .await?;
```

Also add `GET /healthz` returning 200 + a redb open-status check — load balancers and orchestrators need it, and a 20-line handler covers the requirement.

### 8.4. `hnsw_rs` is FFI-free — same musl/aarch64 virtue as redb

The doc gives `usearch` an explicit FFI demerit but doesn't claim the symmetric upside for `hnsw_rs`. State it: pure-Rust HNSW means `cargo build --target x86_64-unknown-linux-musl` and `--target aarch64-unknown-linux-gnu` Just Work — no `bindgen`, no `cxx-build`, no `libstdc++`, no platform-conditional `build.rs`. Combined with redb (also pure Rust), the entire UCFP binary cross-compiles cleanly to a static musl artifact suitable for distroless or scratch images. This is load-bearing for the "one binary, one disk" thesis.

### 8.5. `Record`, `Hit`, `Query` — the types the trait sketches assume

§5's traits reference `Record`, `Hit`, and `Query` without defining them. Concrete shapes for `ucfp-core`:

```rust
/// A unit of work flowing into the index. Modality-agnostic — the
/// fingerprint blob is whatever the producing SDK emits via
/// `bytemuck::cast_slice`.
pub struct Record {
    pub tenant_id: u32,
    pub record_id: u64,                    // monotonic per tenant; UUIDv5 of canonical input
    pub format_version: u32,               // matches the producing SDK's FORMAT_VERSION
    pub algorithm: &'static str,           // e.g. "minhash-h128", "phash-mhf-v1"
    pub config_hash: u64,                  // from txtfp::config_hash or equivalent
    pub fingerprint: Bytes,                // raw bytemuck-cast bytes
    pub embedding: Option<Vec<f32>>,       // optional dense vector (Embedding.vector)
    pub model_id: Option<String>,          // matches Embedding.model_id when embedding is some
    pub metadata: ArchivedMetadata,        // rkyv-archived, variable-length
}

/// Compact result row — what `IndexBackend::knn` and `bm25` return.
/// The matcher composes `Vec<Hit>`s into a fused ranking.
#[derive(Clone, Debug)]
pub struct Hit {
    pub tenant_id: u32,
    pub record_id: u64,
    pub score: f32,                        // higher is better; cosine sim or BM25 score
    pub source: HitSource,                 // which ranker produced this hit (for explainability)
}

#[derive(Copy, Clone, Debug)]
pub enum HitSource { Vector, Bm25, Filter, Reranker }

/// Query envelope passed to the matcher. Each rancher (knn / bm25)
/// reads only the fields it needs.
pub struct Query {
    pub tenant_id: u32,
    pub k: usize,
    pub vector: Option<Vec<f32>>,          // dense query vector; None = BM25-only
    pub terms: Vec<String>,                // tokenized query text; empty = vector-only
    pub filter: Option<RoaringFilter>,     // facet AND/OR/NOT expression
    pub rrf_k: u32,                        // fusion constant; default 60 per §4
}
```

`tenant_id` lives on every shape — see §8.1. `format_version` and `config_hash` on `Record` mirror the producing SDK's stability guarantees so the matcher can refuse cross-version compares without a separate manifest lookup. `Bytes` is `bytes::Bytes` (transitive via hyper) — zero-copy slices, ref-counted, free on the dep graph.

## 9. Multi-tenant auth & quota

Section §5 ("Trait boundaries") covers the three pipeline traits — `IngestSource`, `IndexBackend`, `Reranker`. The HTTP server adds three more, all in `src/server/`, that gate every protected request:

| Trait | File | Responsibility |
|---|---|---|
| `ApiKeyLookup` | `src/server/apikey.rs` | Resolve `Authorization: Bearer <token>` → `ApiKeyContext { tenant_id, key_id, scopes, rate_class }`. `Ok(None)` → 401, `Err(_)` → 5xx. |
| `TenantRateLimiter` | `src/server/ratelimit.rs` | Charge `cost` tokens against the tenant's bucket; return `Allow { remaining, reset_ms }` or `Deny { retry_after_ms }`. |
| `UsageSink` | `src/server/usage.rs` | Receive a `UsageEvent` after each protected request. Must never block the request path — fire-and-forget over `tokio::spawn`. |

The bin (`src/bin/ucfp.rs`) selects one impl per trait at startup from env vars and assembles a `ServerState<EmbeddedBackend>`. The `router_with_state` constructor in `src/server/mod.rs` wires the three traits onto the protected route table; the `public_router` half (`/healthz`, `/v1/info`, `/metrics`) is merged in unauthenticated.

```text
                     ┌── public ───────────────────────────────┐
                     │  /healthz  /v1/info  /metrics           │
   incoming HTTP ──► │                                         │
                     ├── protected ────────────────────────────┤
                     │  Bearer  ─►  ApiKeyLookup ─►  401 / ctx │
                     │  ctx     ─►  TenantRateLimiter ─►  429  │
                     │             handler                     │
                     │             ↓                           │
                     │             UsageSink (spawn, no wait)  │
                     └─────────────────────────────────────────┘
```

### Env-var matrix (resolved at bin startup)

The bin refuses to start if NONE of the three auth-source vars is set.

| Concern | Env var | Concrete impl |
|---|---|---|
| Auth | `UCFP_TOKEN` | `StaticSingleKey { tenant_id: 0 }` (legacy compat) |
| Auth | `UCFP_KEYS_FILE=/path` | `StaticMapKey::from_toml(read_to_string(path))` |
| Auth | `UCFP_KEY_LOOKUP_URL` | `WebhookKeyLookup::new(client, url)` *(requires `multi-tenant`)* |
| Rate limit | `UCFP_RATELIMIT_URL` set | `WebhookRateLimiter::new(client, url)` *(requires `multi-tenant`)* |
| Rate limit | `UCFP_RATELIMIT_URL` unset | `InMemoryTokenBucket::with_limits(100, 200)` (100 rps × 200 burst) |
| Usage | `UCFP_USAGE_WEBHOOK_URL` set | `WebhookUsageSink::spawn(client, url)` *(requires `multi-tenant`)* |
| Usage | `UCFP_USAGE_LOG_PATH` set | `LogUsageSink::open(path)` (NDJSON append) |
| Usage | both unset | `NoopUsageSink` |

Webhook impls live behind the `multi-tenant` Cargo feature (which pulls `reqwest`); the bin emits a clean error if a webhook env var is set on a build without that feature.

### Resolution order and precedence

`ApiKeyLookup` resolution checks `UCFP_KEY_LOOKUP_URL` first (richest), then `UCFP_KEYS_FILE`, then `UCFP_TOKEN`. Setting `UCFP_TOKEN` alongside a webhook is harmless — the webhook wins. Self-hosters keep the one-line `UCFP_TOKEN` deploy; SaaS deploys layer the webhook in front of it.

`router_with_state` uses axum's `FromRef` machinery so handlers continue to take `State<Arc<I>>` without knowing about the auth/rate/usage scaffold. Existing `router(index)` (no auth) is preserved for in-process tests and library consumers that do their own auth.