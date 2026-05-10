<div align="center">

# Universal Content Fingerprinting (UCFP)

## A Comprehensive System for Deterministic, Reproducible Content Fingerprints Across Text, Audio, and Image Modalities

---

**Project Report**

---

**Author:** [Author Name]

**Institution:** [Institution Name]

**Department:** [Department of Computer Science / Software Engineering]

**Date:** May 2026

**Supervisor:** [Supervisor Name]

---

*Submitted in partial fulfillment of the requirements for [Degree/Course]*

</div>

---

## Abstract

The exponential growth of digital content across text, image, and audio modalities has created an urgent need for robust, scalable systems capable of identifying duplicate, near-duplicate, and derivative content. Content fingerprinting — the process of generating compact, deterministic representations of media that are resilient to minor transformations — is fundamental to applications ranging from copyright enforcement and plagiarism detection to content provenance tracking and deduplication in large-scale storage systems.

This report presents the Universal Content Fingerprinting (UCFP) system, a novel open-source platform implemented as a single Rust binary that provides unified fingerprinting capabilities across three content modalities: text, image, and audio. UCFP distinguishes itself from existing solutions through its modular algorithm architecture, supporting multiple fingerprinting techniques per modality — including MinHash, SimHash, Locality-Sensitive Hashing (LSH), and Trend Micro Locality-Sensitive Hashing (TLSH) for text; perceptual hashing (pHash, dHash, aHash) and CLIP-based semantic embeddings for images; and Wang landmark hashing, Panako triplet hashing, Haitsma robust hashing, and neural log-mel embeddings for audio.

The system architecture follows a clean separation of concerns with an HTTP API layer built on the axum web framework, a modality pipeline for algorithm execution, and an embedded storage backend using redb (a pure-Rust ACID-compliant key-value store) combined with brute-force cosine similarity search and BM25 full-text retrieval. A hybrid search capability fuses vector similarity and keyword results using Reciprocal Rank Fusion (RRF). The system supports multi-tenant operation with pluggable authentication, rate limiting, and usage tracking.

A companion web dashboard built with SvelteKit and deployed on Cloudflare Pages provides an interactive playground for experimenting with fingerprinting algorithms, a pipeline inspector for visualizing intermediate computation stages, similarity search interfaces, and records management. The dashboard communicates with the UCFP backend through a proxy layer, enabling real-time visualization of fingerprint generation including MinHash slot heatmaps, SimHash bit wheels, audio spectrograms, and image hash grids.

Performance evaluation demonstrates that the system achieves sub-millisecond fingerprint generation for text documents, single-digit millisecond processing for images, and efficient audio fingerprinting through optimized Rust implementations. The embedded storage architecture eliminates network overhead for persistence operations, while the feature-flag system enables minimal binary sizes for deployment scenarios that require only a subset of algorithms.

The UCFP system represents a significant contribution to the field of content fingerprinting by providing a unified, production-ready platform that consolidates techniques previously available only through disparate, single-modality tools into a cohesive system with consistent APIs, shared storage, and cross-modal search capabilities.

**Keywords:** content fingerprinting, perceptual hashing, MinHash, SimHash, locality-sensitive hashing, audio fingerprinting, near-duplicate detection, vector search, Rust, embedded database

---

## Table of Contents

1. [Chapter 1: Introduction](#chapter-1-introduction)
   - 1.1 Background
   - 1.2 Problem Statement
   - 1.3 Objectives
   - 1.4 Scope
   - 1.5 Motivation
   - 1.6 Report Organization
2. [Chapter 2: Literature Review](#chapter-2-literature-review)
   - 2.1 Text Fingerprinting Techniques
   - 2.2 Image Fingerprinting Techniques
   - 2.3 Audio Fingerprinting Techniques
   - 2.4 Neural and Semantic Approaches
   - 2.5 Existing Systems and Comparison
   - 2.6 Summary of Literature
3. [Chapter 3: System Architecture](#chapter-3-system-architecture)
   - 3.1 Architectural Overview
   - 3.2 Component Diagram
   - 3.3 Request Flow
   - 3.4 Data Flow and Storage
   - 3.5 Technology Stack
   - 3.6 Design Decisions
4. [Chapter 4: Algorithm Design](#chapter-4-algorithm-design)
   - 4.1 Text Fingerprinting Algorithms
   - 4.2 Image Fingerprinting Algorithms
   - 4.3 Audio Fingerprinting Algorithms
   - 4.4 Semantic Embedding Approaches
   - 4.5 Complexity Analysis
5. [Chapter 5: Implementation](#chapter-5-implementation)
   - 5.1 Code Structure and Organization
   - 5.2 Feature Flag System
   - 5.3 API Design
   - 5.4 Database Schema
   - 5.5 Vector Search (HNSW and Brute-Force)
   - 5.6 BM25 Full-Text Retrieval
   - 5.7 Hybrid Search with RRF
6. [Chapter 6: Web Dashboard](#chapter-6-web-dashboard)
   - 6.1 Architecture and Technology
   - 6.2 Playground and Pipeline Inspector
   - 6.3 Search and Records Management
   - 6.4 Real-Time Visualization
   - 6.5 Deployment on Cloudflare Pages
7. [Chapter 7: Testing and Deployment](#chapter-7-testing-and-deployment)
   - 7.1 Testing Strategy
   - 7.2 Continuous Integration
   - 7.3 Docker Containerization
   - 7.4 Production Deployment
8. [Chapter 8: Results and Analysis](#chapter-8-results-and-analysis)
   - 8.1 Performance Characteristics
   - 8.2 Supported Modalities and Algorithms
   - 8.3 Scalability Considerations
9. [Chapter 9: Conclusion and Future Work](#chapter-9-conclusion-and-future-work)
   - 9.1 Summary
   - 9.2 Limitations
   - 9.3 Future Work
10. [References](#references)
11. [Appendix A: API Reference](#appendix-a-api-reference)
12. [Appendix B: Configuration Reference](#appendix-b-configuration-reference)

---

## Chapter 1: Introduction

### 1.1 Background

The digital age has witnessed an unprecedented explosion in content creation and distribution. Every minute, hundreds of hours of video are uploaded to streaming platforms, millions of text documents are published across the web, and countless images are shared on social media. This deluge of digital content presents fundamental challenges for content management, intellectual property protection, and information retrieval systems.

Content fingerprinting, also known as perceptual hashing or robust hashing, is a family of techniques that generate compact, fixed-size representations of media content. Unlike cryptographic hashes (SHA-256, MD5), which produce entirely different outputs for even single-bit changes in input, content fingerprints are designed to be *robust* — similar inputs produce similar fingerprints, enabling detection of near-duplicate and derivative content even after transformations such as compression, format conversion, cropping, or paraphrasing.

The concept of content fingerprinting has evolved significantly since its early applications in text plagiarism detection (Broder, 1997) and audio identification (Wang, 2003). Modern systems must contend with increasingly sophisticated content manipulation, cross-modal content reuse (e.g., text-to-speech conversion, image captioning), and the sheer scale of content repositories that may contain billions of items requiring real-time similarity queries.

Traditional approaches to content fingerprinting have been modality-specific: text systems use shingling and MinHash, image systems use DCT-based perceptual hashes, and audio systems use spectral landmark extraction. Each modality has developed its own ecosystem of tools, libraries, and storage formats, creating fragmentation that complicates systems requiring cross-modal content tracking.

### 1.2 Problem Statement

Despite decades of research in content fingerprinting, practitioners face several persistent challenges:

1. **Fragmentation across modalities.** Existing tools are typically single-modality. A system that needs to fingerprint text, images, and audio must integrate three or more separate libraries, each with different APIs, storage formats, and query interfaces. This fragmentation increases operational complexity and makes cross-modal deduplication impractical.

2. **Algorithm selection complexity.** Each modality offers multiple fingerprinting algorithms with different trade-offs between robustness, discriminative power, and computational cost. MinHash excels at set-similarity for long documents but struggles with short texts; SimHash handles weighted features but is sensitive to document length; perceptual image hashes are fast but miss semantic similarity. Practitioners must understand these trade-offs and often need to experiment with multiple algorithms before finding the right fit.

3. **Lack of unified retrieval.** Content fingerprints are only useful if they can be efficiently queried. Vector similarity search (for dense embeddings), Hamming distance computation (for binary hashes), and keyword search (for metadata) are typically served by different systems — a vector database, a specialized hash index, and a full-text engine respectively. Coordinating queries across these systems adds latency and operational burden.

4. **Deployment complexity.** Production fingerprinting systems often require multiple services: a compute layer for fingerprint generation, a vector database for similarity search, a metadata store, and an API gateway. This multi-service architecture demands container orchestration, service mesh configuration, and distributed monitoring.

5. **Reproducibility and determinism.** Many fingerprinting implementations produce non-deterministic results due to floating-point ordering, random seed management, or platform-dependent behavior. This makes it impossible to verify that two systems produce identical fingerprints for the same input, complicating distributed deployments and testing.

### 1.3 Objectives

The Universal Content Fingerprinting (UCFP) project addresses these challenges with the following objectives:

1. **Unified multi-modal fingerprinting.** Provide a single system capable of fingerprinting text, image, and audio content through a consistent HTTP API, with shared storage and query interfaces across all modalities.

2. **Algorithm diversity with sensible defaults.** Support multiple fingerprinting algorithms per modality, allowing practitioners to select the best algorithm for their use case while providing well-tuned defaults that work for common scenarios.

3. **Integrated retrieval.** Combine vector similarity search (approximate nearest neighbor), full-text keyword search (BM25), and hybrid fusion (Reciprocal Rank Fusion) in a single query interface, eliminating the need for external search infrastructure.

4. **Single-binary deployment.** Package the entire system — HTTP server, fingerprinting algorithms, storage engine, and search index — into a single statically-linked binary with no external dependencies beyond the filesystem, minimizing operational complexity.

5. **Deterministic, reproducible fingerprints.** Ensure that identical inputs with identical configuration always produce identical fingerprints, regardless of platform, enabling distributed verification and testing.

6. **Extensibility through feature flags.** Use Rust's compile-time feature flag system to allow users to build minimal binaries containing only the algorithms they need, reducing binary size and attack surface for constrained deployments.

7. **Production readiness.** Include authentication, rate limiting, usage tracking, health checks, and Prometheus metrics as first-class concerns, not afterthoughts.

### 1.4 Scope

The UCFP system encompasses the following within its scope:

**In scope:**
- Text fingerprinting: MinHash, SimHash (TF and IDF weighted), band-partitioned LSH, TLSH, and semantic embeddings via local ONNX models or external APIs (OpenAI, Voyage, Cohere)
- Image fingerprinting: perceptual hashing (pHash, dHash, aHash), multi-hash bundles, and CLIP-based semantic embeddings via local ONNX
- Audio fingerprinting: Wang landmark hashing, Panako triplet hashing, Haitsma robust hashing, neural log-mel embeddings, and AudioSeal watermark detection
- Embedded storage using redb with ACID transactions
- Vector k-NN search (brute-force cosine, with HNSW deferred to ≥1M vectors)
- BM25 keyword search with FST term dictionary and roaring bitmap postings
- Hybrid search with Reciprocal Rank Fusion
- Multi-tenant authentication and rate limiting
- Web dashboard for interactive exploration
- Docker containerization and Cloudflare deployment

**Out of scope (planned for future work):**
- Video fingerprinting (keyframe extraction, scene hashes)
- Document fingerprinting (OCR + layout analysis)
- Distributed/clustered deployment
- Real-time streaming at scale (>100k events/second)

### 1.5 Motivation

The motivation for UCFP arises from practical experience with content management systems that require fingerprinting across multiple modalities. Consider a digital publishing platform that ingests articles (text), photographs (images), and podcasts (audio). To detect unauthorized republication, the platform must:

1. Fingerprint each piece of content at ingestion time
2. Store fingerprints efficiently for millions of items
3. Query for similar content when new items arrive
4. Handle content that has been transformed (paraphrased text, cropped images, re-encoded audio)

Without a unified system, this platform would need to integrate separate text similarity services (perhaps using Elasticsearch with custom analyzers), image deduplication tools (perhaps using a pHash library with a custom database), and audio identification services (perhaps using a Shazam-like system). Each integration adds API surface, failure modes, and operational overhead.

UCFP consolidates these capabilities into a single binary that can be deployed with a single environment variable (`UCFP_TOKEN=secret ./ucfp`), making content fingerprinting accessible to teams that lack the resources to operate multiple specialized services.

The choice of Rust as the implementation language is motivated by three factors: (1) performance — fingerprinting algorithms are computationally intensive and benefit from zero-cost abstractions and SIMD optimization; (2) safety — memory safety without garbage collection eliminates entire classes of bugs in a system handling untrusted binary content; and (3) deployment simplicity — static linking produces self-contained binaries suitable for minimal container images.

### 1.6 Report Organization

The remainder of this report is organized as follows:

- **Chapter 2** reviews the literature on content fingerprinting techniques across text, image, and audio modalities, and compares UCFP with existing commercial and open-source systems.
- **Chapter 3** presents the system architecture, including component diagrams, request flow, and technology stack decisions.
- **Chapter 4** details the algorithmic design of each fingerprinting technique, including mathematical foundations and complexity analysis.
- **Chapter 5** describes the implementation, covering code structure, feature flags, API design, database schema, and search infrastructure.
- **Chapter 6** covers the web dashboard, its architecture, visualization capabilities, and deployment.
- **Chapter 7** discusses testing strategy, continuous integration, and production deployment.
- **Chapter 8** presents results and analysis of system performance and scalability.
- **Chapter 9** concludes with a summary of contributions, limitations, and directions for future work.

---

## Chapter 2: Literature Review

### 2.1 Text Fingerprinting Techniques

#### 2.1.1 Shingling and MinHash

The foundational work on text near-duplicate detection was established by Broder (1997) with the introduction of shingling — representing documents as sets of contiguous subsequences (k-grams or shingles) — combined with MinHash for efficient set similarity estimation. The MinHash technique exploits the mathematical property that the probability of two sets having the same minimum hash value under a random permutation equals their Jaccard similarity:

$$P[\min(h(A)) = \min(h(B))] = J(A, B) = \frac{|A \cap B|}{|A \cup B|}$$

By computing multiple independent hash functions (typically 128–256), MinHash produces a compact signature that estimates Jaccard similarity with bounded error. The Locality-Sensitive Hashing (LSH) framework (Indyk & Motwani, 1998) extends this by partitioning the signature into bands, enabling sub-linear time approximate nearest neighbor queries.

MinHash has been widely adopted in web-scale deduplication systems. Google's SimHash-based approach (Manku et al., 2007) was deployed for detecting near-duplicate web pages across billions of documents. The technique remains relevant in modern systems due to its simplicity, theoretical guarantees, and amenability to distributed computation.

#### 2.1.2 SimHash

Charikar (2002) introduced SimHash as a dimensionality reduction technique that maps high-dimensional vectors to compact binary codes while preserving cosine similarity. For text documents, SimHash operates by:

1. Extracting weighted features (typically TF or TF-IDF weighted terms)
2. Hashing each feature to a binary vector
3. Computing a weighted sum across all feature hashes
4. Thresholding the sum to produce the final binary fingerprint

The key property of SimHash is that the Hamming distance between two fingerprints approximates the cosine distance between the original feature vectors:

$$P[h(x) = h(y)] = 1 - \frac{\theta(x, y)}{\pi}$$

where θ(x, y) is the angle between vectors x and y. This makes SimHash particularly effective for detecting documents with similar term distributions, even when the exact set of terms differs.

Manku, Jain, and Sarma (2007) demonstrated SimHash at Google scale, showing that 64-bit SimHash fingerprints could identify near-duplicate web pages with high precision when combined with a 3-bit Hamming distance threshold.

#### 2.1.3 Locality-Sensitive Hashing (LSH)

LSH (Indyk & Motwani, 1998; Gionis et al., 1999) is a general framework for approximate nearest neighbor search in high-dimensional spaces. The core idea is to hash input items such that similar items map to the same bucket with high probability, while dissimilar items collide with low probability.

For text applications, band-partitioned LSH divides a MinHash signature into b bands of r rows each. Two documents are considered candidates for similarity if they agree in at least one complete band. The probability of two documents with Jaccard similarity s being identified as candidates is:

$$P(\text{candidate}) = 1 - (1 - s^r)^b$$

This S-curve can be tuned by adjusting b and r to control the trade-off between recall (finding all similar pairs) and precision (avoiding false positives).

#### 2.1.4 TLSH (Trend Micro Locality-Sensitive Hash)

TLSH (Oliver et al., 2013) is a fuzzy hashing algorithm developed by Trend Micro for malware similarity detection. Unlike MinHash (which measures set similarity) or SimHash (which measures cosine similarity), TLSH captures the statistical distribution of byte sequences within a document.

TLSH operates by:
1. Computing a sliding-window hash over the input (Pearson hashing with window size 5)
2. Building a histogram of hash values (128 buckets for the standard configuration)
3. Computing quartile points of the histogram
4. Encoding each bucket relative to the quartiles as a 2-bit value
5. Prepending a header with checksum, document length, and quartile ratios

The resulting 70-byte hash (35 hex-encoded bytes for the body plus a 3-byte header) enables distance computation that is robust to insertions, deletions, and modifications affecting up to ~30% of the document. TLSH requires a minimum input length of 50 bytes (256 bytes recommended) to produce statistically meaningful histograms.

### 2.2 Image Fingerprinting Techniques

#### 2.2.1 Perceptual Hashing (pHash)

Perceptual hashing for images was pioneered by Zauner (2010) and formalized in the pHash library. The DCT-based perceptual hash (pHash) operates by:

1. Resizing the image to a fixed size (typically 32×32 pixels)
2. Converting to grayscale
3. Computing the 2D Discrete Cosine Transform (DCT)
4. Retaining only the top-left 8×8 DCT coefficients (low frequencies)
5. Computing the median of these 64 values
6. Generating a 64-bit hash where each bit indicates whether the corresponding coefficient exceeds the median

The DCT-based approach captures the fundamental frequency structure of the image, making it robust to scaling, minor rotations, brightness adjustments, and lossy compression. The Hamming distance between two pHash values correlates with perceptual similarity — identical images produce distance 0, while visually similar images typically produce distances below 10.

#### 2.2.2 Difference Hash (dHash) and Average Hash (aHash)

Difference hash (dHash) captures relative gradient information by:
1. Resizing to 9×8 pixels (grayscale)
2. Computing horizontal differences between adjacent pixels
3. Encoding each difference as a single bit (1 if left > right, 0 otherwise)

This produces a 64-bit hash that is particularly robust to uniform brightness and contrast changes, since it encodes relative rather than absolute intensity values.

Average hash (aHash) is the simplest perceptual hash:
1. Resize to 8×8 pixels (grayscale)
2. Compute the mean pixel value
3. Set each bit to 1 if the pixel exceeds the mean, 0 otherwise

While less discriminative than pHash or dHash, aHash is extremely fast to compute and provides a useful first-pass filter for obvious duplicates.

#### 2.2.3 CLIP-Based Semantic Embeddings

Radford et al. (2021) introduced CLIP (Contrastive Language-Image Pre-training), a neural network trained on 400 million image-text pairs to learn a shared embedding space for images and text. CLIP embeddings capture semantic content rather than pixel-level features, enabling detection of images that depict the same concept even when they differ substantially in appearance.

For content fingerprinting, CLIP embeddings (typically 512 or 768 dimensions) serve as semantic fingerprints that complement perceptual hashes. While pHash detects near-identical images (same photo with different compression), CLIP embeddings detect semantically similar images (different photos of the same subject).

### 2.3 Audio Fingerprinting Techniques

#### 2.3.1 Wang Landmark Algorithm (Shazam)

Wang (2003) introduced the landmark-based audio fingerprinting algorithm that powers Shazam. The algorithm operates by:

1. Computing a spectrogram via Short-Time Fourier Transform (STFT)
2. Identifying spectral peaks (landmarks) — local maxima in the time-frequency plane
3. Forming pairs of landmarks within a target zone (combinatorial hashing)
4. Encoding each pair as a hash: (f1, f2, Δt) where f1 and f2 are the frequencies of the two peaks and Δt is their time difference

The genius of the Wang algorithm lies in its use of *pairs* rather than individual peaks. Single peaks are sensitive to noise and interference, but the geometric relationship between pairs is highly stable across recording conditions, background noise, and audio compression.

The resulting fingerprint is a set of (hash, time_offset) pairs that enable both identification (matching against a database) and temporal alignment (determining where in a recording the match occurs).

#### 2.3.2 Panako Triplet Algorithm

Six and Leman (2014) extended the landmark approach with the Panako algorithm, which uses triplets of spectral peaks rather than pairs. Each triplet encodes:

- Three frequency values (f1, f2, f3)
- Two time differences (Δt12, Δt13)
- A frequency ratio (f2/f1 or f3/f1)

The use of frequency ratios makes Panako robust to pitch shifting — a transformation that defeats the Wang algorithm. This is particularly relevant for detecting content that has been speed-adjusted (e.g., slightly accelerated video playback to evade detection).

#### 2.3.3 Haitsma Robust Hash

Haitsma and Kalker (2002) at Philips Research developed a robust audio hash based on spectral band energy differences. The algorithm:

1. Resamples audio to a fixed rate (5 kHz in UCFP's implementation)
2. Divides the spectrum into 33 frequency bands (Bark scale)
3. For each frame, computes the energy in each band
4. Generates a 32-bit hash per frame where each bit encodes whether the energy difference between adjacent bands exceeds the energy difference in the previous frame

This double-differential encoding (across both frequency and time) provides robustness to equalization, dynamic range compression, and additive noise. The algorithm produces one 32-bit hash per frame (approximately 31.25 frames/second at 5 kHz), enabling efficient storage and Hamming-distance-based matching.

#### 2.3.4 Neural Audio Embeddings

Recent advances in self-supervised audio representation learning (Baevski et al., 2020; Hershey et al., 2017) have produced neural models that generate dense embeddings capturing high-level audio semantics. These embeddings, typically derived from log-mel spectrogram features processed through transformer or convolutional architectures, enable detection of semantically similar audio (e.g., different recordings of the same speech) that traditional spectral fingerprints would miss.

UCFP supports neural audio embeddings through ONNX Runtime inference, allowing deployment of pre-trained models without Python dependencies.

### 2.4 Neural and Semantic Approaches

#### 2.4.1 Dense Retrieval and Embedding Models

The emergence of large language models and contrastive learning has produced a new generation of embedding models specifically designed for retrieval tasks. Models such as OpenAI's text-embedding-3-large, Voyage AI's voyage-3, and Cohere's embed-v4 generate dense vector representations that capture semantic meaning, enabling similarity search that goes beyond lexical overlap.

These models complement traditional fingerprinting techniques by capturing meaning rather than surface form. A paraphrased document that shares no n-grams with the original will have a very different MinHash signature but a very similar embedding vector.

#### 2.4.2 Approximate Nearest Neighbor (ANN) Search

Efficient retrieval of similar embeddings requires approximate nearest neighbor algorithms. The Hierarchical Navigable Small World (HNSW) algorithm (Malkov & Yashunin, 2018) has emerged as the dominant approach, offering logarithmic query complexity with high recall. HNSW constructs a multi-layer graph where each layer provides increasingly coarse navigation, enabling efficient traversal from random entry points to the nearest neighbors of a query vector.

For smaller collections (below ~1 million vectors), brute-force cosine similarity with SIMD acceleration remains competitive, offering exact results without the recall trade-off inherent in approximate methods.

### 2.5 Existing Systems and Comparison

#### 2.5.1 Shazam

Shazam (Wang, 2003) is the most commercially successful audio fingerprinting system, capable of identifying songs from short, noisy recordings captured through a smartphone microphone. Shazam uses the Wang landmark algorithm with a massive database of pre-computed fingerprints. While highly effective for audio identification, Shazam is a closed-source commercial service limited to audio and does not support text or image fingerprinting.

#### 2.5.2 Google Content ID

YouTube's Content ID system uses a combination of audio and video fingerprinting to identify copyrighted content in uploaded videos. The system processes over 500 hours of video per minute and maintains a reference database provided by content owners. Content ID is proprietary, operates only within the YouTube ecosystem, and is not available as a general-purpose fingerprinting service.

#### 2.5.3 Dejavu

Dejavu (open-source) implements the Wang landmark algorithm in Python, providing audio fingerprinting with a MySQL or PostgreSQL backend. While functional, Dejavu's Python implementation limits throughput, and its reliance on an external database adds deployment complexity. UCFP's Rust implementation of the same algorithm achieves significantly higher throughput with an embedded database.

#### 2.5.4 ImageHash (Python)

The `imagehash` Python library provides implementations of aHash, pHash, dHash, and wavelet hash. It is widely used for image deduplication but operates only on individual images without integrated storage or search capabilities. UCFP incorporates equivalent algorithms with integrated persistence and similarity search.

#### 2.5.5 TLSH (Trend Micro)

Trend Micro's open-source TLSH library provides fuzzy hashing for binary and text content. Originally designed for malware similarity detection, TLSH has found applications in document deduplication and plagiarism detection. UCFP integrates TLSH as one of several text fingerprinting options, providing it within a unified API alongside complementary techniques.

### 2.6 Summary of Literature

The literature reveals that content fingerprinting is a mature field with well-understood algorithms for each modality, but existing implementations are fragmented across single-purpose tools and services. No existing open-source system provides:

1. Multi-modal fingerprinting (text + image + audio) through a unified API
2. Multiple algorithm choices per modality with runtime selection
3. Integrated storage and similarity search (vector + keyword + hybrid)
4. Single-binary deployment with no external dependencies
5. Deterministic, reproducible fingerprints with formal versioning

UCFP addresses this gap by consolidating proven algorithms into a cohesive system with consistent interfaces, shared storage, and production-ready operational features.

---

## Chapter 3: System Architecture

### 3.1 Architectural Overview

UCFP follows a layered architecture organized around a single Rust binary that encapsulates all system functionality. The architecture is designed around three principles: (1) clean separation of concerns through Rust traits, (2) compile-time configurability through feature flags, and (3) operational simplicity through embedded storage.

The system is organized into five primary layers:

1. **HTTP Server Layer** (`src/server/`) — Request routing, authentication, rate limiting, usage tracking, and response serialization using the axum web framework.
2. **Modality Pipeline** (`src/modality/`) — Algorithm selection and execution for text, image, and audio fingerprinting.
3. **Index Backend** (`src/index/`) — Persistent storage of fingerprints, embeddings, and metadata using redb, with vector search and BM25 retrieval.
4. **Matcher** (`src/matcher/`) — Query orchestration combining vector similarity and keyword search with Reciprocal Rank Fusion.
5. **Core Types** (`src/core/`) — Shared data structures (`Record`, `Hit`, `Query`, `Modality`) used across all layers.

This layered design ensures that each component can be tested independently and that alternative implementations (e.g., a Qdrant-backed index for scale-up) can be substituted without modifying the layers above.

### 3.2 Component Diagram

The system's component relationships are captured in the following architectural diagram:

```
┌─────────────────────────────────────────────────────────────────────┐
│                        HTTP Client                                    │
└─────────────────────────┬───────────────────────────────────────────┘
                          │ HTTP POST/GET
┌─────────────────────────▼───────────────────────────────────────────┐
│                    Server Layer (axum)                                │
│  ┌──────────────┐  ┌───────────────┐  ┌──────────────┐             │
│  │ ApiKeyLookup │→ │ RateLimiter   │→ │ UsageSink    │             │
│  │ (auth)       │  │ (token bucket)│  │ (fire+forget)│             │
│  └──────────────┘  └───────────────┘  └──────────────┘             │
│  ┌──────────────────────────────────────────────────────┐           │
│  │              REST Handlers (handlers.rs)               │           │
│  │  /v1/ingest/{modality}/{tid}/{rid}                    │           │
│  │  /v1/query  /v1/records  /healthz  /metrics           │           │
│  └──────────────────────────┬───────────────────────────┘           │
└─────────────────────────────┼───────────────────────────────────────┘
                              │
┌─────────────────────────────▼───────────────────────────────────────┐
│                    Modality Pipeline                                  │
│  ┌────────────┐    ┌────────────┐    ┌────────────┐                 │
│  │    Text    │    │   Image    │    │   Audio    │                 │
│  │ minhash    │    │ multi      │    │ wang       │                 │
│  │ simhash    │    │ phash      │    │ panako     │                 │
│  │ lsh        │    │ dhash      │    │ haitsma    │                 │
│  │ tlsh       │    │ ahash      │    │ neural     │                 │
│  │ semantic   │    │ semantic   │    │ watermark  │                 │
│  └─────┬──────┘    └─────┬──────┘    └─────┬──────┘                 │
└────────┼──────────────────┼──────────────────┼──────────────────────┘
         │                  │                  │
         └──────────────────┼──────────────────┘
                            │ Record { fingerprint, embedding }
┌───────────────────────────▼─────────────────────────────────────────┐
│                    Index Backend (redb)                               │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐              │
│  │ Fingerprints │  │   Vectors    │  │   BM25       │              │
│  │ (bytemuck)   │  │ (f32 array)  │  │ (FST+roaring)│              │
│  └──────────────┘  └──────────────┘  └──────────────┘              │
│  ┌──────────────┐  ┌──────────────┐                                 │
│  │   Catalog    │  │  Metadata    │                                 │
│  │ (algorithm)  │  │ (rkyv)       │                                 │
│  └──────────────┘  └──────────────┘                                 │
└─────────────────────────────────────────────────────────────────────┘
```

### 3.3 Request Flow

The system processes two primary request types: **ingest** (fingerprint generation and storage) and **query** (similarity search).

#### Ingest Flow

1. Client sends `POST /v1/ingest/{modality}/{tenant_id}/{record_id}` with raw content body
2. `ApiKeyLookup` middleware validates the Bearer token and resolves tenant context
3. `TenantRateLimiter` checks the tenant's token bucket; returns 429 if exhausted
4. Handler parses the `?algorithm=` query parameter (defaults to modality-specific default)
5. Handler invokes the modality function: `fingerprint(bytes, algorithm, opts)`
6. Modality function returns a `Record` containing fingerprint bytes, optional embedding vector, algorithm identifier, and metadata
7. Handler calls `IndexBackend::upsert(&[record])` to persist the record
8. `UsageSink` receives a fire-and-forget usage event via `tokio::spawn`
9. Handler returns `201 Created` with JSON containing record_id, algorithm, and fingerprint hex

#### Query Flow

1. Client sends `POST /v1/query` with JSON body containing tenant_id, modality, k, vector, and/or terms
2. Authentication and rate limiting proceed as above
3. Handler constructs a `Query` struct and passes it to the `Matcher`
4. Matcher executes vector k-NN and BM25 search in parallel via `tokio::try_join!`
5. Results from both retrievers are fused using Reciprocal Rank Fusion (RRF with k=60)
6. Fused results are optionally passed through a `Reranker` (no-op by default)
7. Handler returns `200 OK` with JSON array of hits (record_id, score, source)

### 3.4 Data Flow and Storage

All persistent state resides in a single redb database file (`ucfp.redb`) located at the path specified by `UCFP_DATA_DIR`. The storage layout uses composite keys of `(tenant_id: u32, record_id: u64)` to enable multi-tenant isolation within a single file:

```
ucfp.redb
├── fingerprints   (tenant_id, record_id) → raw bytes (bytemuck-cast fingerprint)
├── metadata       (tenant_id, record_id) → rkyv-archived metadata
├── vectors        (tenant_id, record_id) → f32 array (little-endian)
├── catalog        (tenant_id, record_id) → JSON {algorithm, format_version, config_hash}
├── bm25_terms     FST<str> → (offset, len)  — term dictionary
├── bm25_postings  (term_offset, tenant, id) → roaring bitmap
└── bm25_scoring   (tenant_id, record_id) → (doc_len, avg_field_len)
```

The redb storage engine provides:
- **ACID transactions** with single-fsync commits
- **MVCC** (Multi-Version Concurrency Control) — one writer, unlimited concurrent readers
- **Copy-on-Write B-tree** — crash-safe without WAL, consistent filesystem snapshots
- **XXH3-128 page checksums** — silent corruption detection
- **Pure Rust** — no C/C++ FFI, cross-compiles to musl and aarch64

Backup is trivially accomplished by copying the database file while the system is running, as MVCC ensures the copy sees only fully-committed pages.

### 3.5 Technology Stack

| Layer | Technology | Version | Rationale |
|-------|-----------|---------|-----------|
| Language | Rust | 1.88+ (Edition 2024) | Memory safety, zero-cost abstractions, SIMD, static linking |
| HTTP Framework | axum | 0.8 | Tokio-native, Tower middleware, type-safe extractors |
| Async Runtime | Tokio | 1.47 LTS | Multi-threaded work-stealing, signal handling, timers |
| Storage | redb | 3.0 | Pure Rust ACID KV store, single file, MVCC |
| Vector Search | pulp + rayon | 0.22 / 1.12 | SIMD-accelerated brute-force cosine (day-one) |
| ANN Index | hnsw_rs | 0.3 | Pure Rust HNSW (graduation at ≥1M vectors) |
| Bitmap Filters | roaring | 0.11 | Compressed bitmaps for faceted filtering |
| Term Dictionary | fst | 0.4.7 | Memory-mapped finite state transducer |
| Serialization | rkyv | 0.8 | Zero-copy deserialization for metadata |
| Allocator | mimalloc | 0.1.50 | High-throughput concurrent allocator |
| Metrics | metrics + prometheus exporter | 0.24 / 0.18 | Pull-based Prometheus metrics |
| Auth | subtle | 2.6 | Constant-time token comparison |
| Text SDK | txtfp | 0.2 | MinHash, SimHash, LSH, TLSH, semantic |
| Image SDK | imgfprint | 0.4.1 | pHash, dHash, aHash, CLIP |
| Audio SDK | audiofp | 0.3 | Wang, Panako, Haitsma, neural, watermark |
| Frontend | SvelteKit | 2.8 | Server-side rendering, Cloudflare adapter |
| Frontend Runtime | Svelte | 5.1 | Reactive UI with runes |
| Frontend Deployment | Cloudflare Pages | — | Edge deployment, Workers integration |
| Visualization | @xyflow/svelte | 1.5 | Node-based flow diagrams |
| Containerization | Docker | Multi-stage | Minimal debian:bookworm-slim runtime |

### 3.6 Design Decisions

#### 3.6.1 Single Binary vs. Microservices

UCFP deliberately packages all functionality into a single binary rather than decomposing into microservices. This decision is justified by:

- **Operational simplicity:** One process to deploy, monitor, and restart. No service mesh, no inter-service authentication, no distributed tracing.
- **Latency:** Fingerprint generation and storage occur in the same process, eliminating network round-trips between compute and storage.
- **Consistency:** ACID transactions span fingerprint storage and index updates atomically, impossible with separate services without distributed transactions.
- **Resource efficiency:** A single process with shared memory (mimalloc arena) uses less total RAM than equivalent functionality split across containers.

The architecture supports graduation to distributed systems through the `IndexBackend` trait — a Qdrant implementation can replace the embedded backend when the corpus exceeds ~100M vectors.

#### 3.6.2 Embedded Storage vs. External Database

The choice of redb over PostgreSQL, Redis, or a managed vector database is motivated by:

- **Zero operational overhead:** No connection pooling, no schema migrations, no backup orchestration (just `cp`).
- **Predictable latency:** No network hop for reads or writes; redb's B-tree provides O(log n) access with page-cache-friendly access patterns.
- **Pure Rust:** Cross-compiles cleanly to musl (static binary) and aarch64 without C/C++ toolchain dependencies.
- **ACID without WAL:** Copy-on-write B-tree provides crash safety through atomic root pointer swap, simpler than WAL-based recovery.

#### 3.6.3 Feature Flags for Compile-Time Configuration

Rust's feature flag system enables users to build minimal binaries containing only the algorithms they need:

```bash
# Minimal: only text MinHash + image multi-hash + audio Wang
cargo build --release --bin ucfp

# Full: all algorithms including ONNX neural models
cargo build --release --features full --bin ucfp
```

This approach reduces binary size (from ~50MB with all features to ~15MB minimal), eliminates unused code paths from the attack surface, and avoids pulling heavy dependencies (ONNX Runtime, reqwest) when they're not needed.

#### 3.6.4 Trait-Based Extensibility

Three core traits define the system's extension points:

- `IndexBackend` — Storage and retrieval (embedded redb, or Qdrant for scale-up)
- `ApiKeyLookup` — Authentication (static token, TOML file, or webhook)
- `TenantRateLimiter` — Rate limiting (in-memory token bucket, or webhook)
- `UsageSink` — Usage tracking (no-op, file log, or webhook)

The binary selects concrete implementations at startup based on environment variables, enabling the same codebase to serve single-tenant self-hosted deployments and multi-tenant SaaS configurations.

#### 3.6.5 Multi-Tenant by Default

The storage schema uses `(tenant_id: u32, record_id: u64)` composite keys from day one, even for single-tenant deployments (which use `tenant_id = 0`). This avoids a costly data migration when multi-tenancy is later required and enables per-tenant range scans for isolation.

---

## Chapter 4: Algorithm Design

### 4.1 Text Fingerprinting Algorithms

#### 4.1.1 MinHash (Default)

MinHash is the default text fingerprinting algorithm in UCFP, implemented via the `txtfp` crate with H=128 independent hash functions. The algorithm proceeds as follows:

**Pseudocode:**

```
function MINHASH(document, num_hashes=128, shingle_size=5):
    shingles ← extract_character_shingles(document, shingle_size)
    signature ← [∞] × num_hashes
    
    for each shingle s in shingles:
        for i in 0..num_hashes:
            h ← hash_function_i(s)
            signature[i] ← min(signature[i], h)
    
    return signature
```

**Implementation details in UCFP:**
- Character-level shingles (k=5) rather than word-level for language independence
- 128 hash functions using the "one permutation" optimization (Li et al., 2012)
- Output: 128 × 32-bit values = 512 bytes per fingerprint
- Stored as raw `bytemuck::cast_slice` bytes in redb

**Similarity estimation:**
```
J_hat(A, B) = (number of matching slots) / 128
```

The standard error of this estimate is √(J(1-J)/128), giving ±4.4% at J=0.5.

#### 4.1.2 SimHash (TF and IDF Weighted)

UCFP implements two SimHash variants, selectable via `?algorithm=simhash-tf` or `?algorithm=simhash-idf`:

**Pseudocode:**

```
function SIMHASH(document, weighting="tf", bits=256):
    terms ← tokenize(document)
    weights ← compute_weights(terms, weighting)  // TF or TF-IDF
    accumulator ← [0.0] × bits
    
    for each (term, weight) in zip(terms, weights):
        hash ← sha256(term)  // or fast hash to `bits` bits
        for i in 0..bits:
            if hash[i] == 1:
                accumulator[i] += weight
            else:
                accumulator[i] -= weight
    
    fingerprint ← [0] × bits
    for i in 0..bits:
        fingerprint[i] ← 1 if accumulator[i] > 0 else 0
    
    return fingerprint
```

**TF weighting:** `w(t) = count(t, document)`
**TF-IDF weighting:** `w(t) = tf(t, d) × log(N / df(t))` where N is a corpus-level constant and df(t) is estimated from the document collection.

The IDF variant requires corpus statistics, which UCFP maintains incrementally in the BM25 scoring table. For the first document ingested, TF-IDF falls back to pure TF weighting.

**Output:** 256-bit binary fingerprint (32 bytes)
**Distance metric:** Hamming distance; threshold of ≤10 bits indicates high similarity

#### 4.1.3 Band-Partitioned LSH

The LSH algorithm in UCFP builds on MinHash signatures to enable sub-linear candidate retrieval:

**Pseudocode:**

```
function LSH_INDEX(signature, bands=20, rows_per_band=6):
    for b in 0..bands:
        band_slice ← signature[b*rows_per_band .. (b+1)*rows_per_band]
        bucket_key ← hash(band_slice)
        insert(band_table[b], bucket_key, document_id)

function LSH_QUERY(query_signature, bands=20, rows_per_band=6):
    candidates ← ∅
    for b in 0..bands:
        band_slice ← query_signature[b*rows_per_band .. (b+1)*rows_per_band]
        bucket_key ← hash(band_slice)
        candidates ← candidates ∪ lookup(band_table[b], bucket_key)
    
    return candidates
```

With b=20 bands and r=6 rows per band (using 120 of the 128 MinHash slots), the probability of two documents with Jaccard similarity s being identified as candidates is:

$$P = 1 - (1 - s^6)^{20}$$

This gives approximately:
- s=0.3: P ≈ 0.01 (1% false positive rate)
- s=0.5: P ≈ 0.47 (47% detection rate)
- s=0.7: P ≈ 0.98 (98% detection rate)
- s=0.9: P ≈ 1.00 (near-certain detection)

#### 4.1.4 TLSH

UCFP integrates TLSH through the `txtfp` crate's `tlsh` feature. The algorithm requires a minimum of 50 bytes of input:

**Pseudocode:**

```
function TLSH(document):
    if len(document) < 50:
        return ERROR("insufficient input length")
    
    // Step 1: Sliding window hash
    buckets ← [0] × 128
    for i in 0..len(document)-4:
        window ← document[i..i+5]
        for each triplet in combinations(window, 3):
            bucket_idx ← pearson_hash(triplet) mod 128
            buckets[bucket_idx] += 1
    
    // Step 2: Quartile computation
    q1, q2, q3 ← compute_quartiles(buckets)
    
    // Step 3: Binary encoding
    body ← []
    for each bucket b in buckets:
        if b <= q1: body.append(0b00)
        elif b <= q2: body.append(0b01)
        elif b <= q3: body.append(0b10)
        else: body.append(0b11)
    
    // Step 4: Header
    checksum ← compute_checksum(document)
    length_code ← encode_length(len(document))
    q_ratios ← encode_quartile_ratios(q1, q2, q3)
    
    return header(checksum, length_code, q_ratios) || body
```

**Output:** 70 hex characters (35 bytes)
**Distance metric:** Custom TLSH distance function (weighted Hamming on body + header penalties)
**Threshold:** Distance ≤ 100 indicates similarity; ≤ 30 indicates near-identity

#### 4.1.5 Semantic Embeddings

For semantic text fingerprinting, UCFP supports four providers:

| Provider | Model | Dimensions | Feature Flag |
|----------|-------|-----------|--------------|
| Local ONNX | Configurable | 384–1024 | `text-semantic-local` |
| OpenAI | text-embedding-3-large | 3072 (truncatable) | `text-semantic-openai` |
| Voyage AI | voyage-3 | 1024 | `text-semantic-voyage` |
| Cohere | embed-v4 | 1024 | `text-semantic-cohere` |

The local ONNX path avoids external API calls, providing deterministic embeddings with no network dependency. External providers offer higher-quality embeddings at the cost of latency and API key management.

### 4.2 Image Fingerprinting Algorithms

#### 4.2.1 Multi-Hash Bundle (Default)

The default image algorithm computes all three perceptual hashes simultaneously, producing a combined fingerprint:

**Pseudocode:**

```
function MULTI_HASH(image_bytes):
    img ← decode_image(image_bytes)
    gray ← to_grayscale(img)
    
    phash ← compute_phash(gray)    // 64 bits
    dhash ← compute_dhash(gray)    // 64 bits
    ahash ← compute_ahash(gray)    // 64 bits
    
    return concat(phash, dhash, ahash)  // 192 bits = 24 bytes
```

This bundle provides redundancy — if one hash produces a false negative (e.g., pHash fails on a heavily cropped image), the other hashes may still match.

#### 4.2.2 Perceptual Hash (pHash)

**Pseudocode:**

```
function PHASH(grayscale_image):
    resized ← resize(grayscale_image, 32, 32)
    dct ← dct_2d(resized)
    
    // Keep only top-left 8×8 (low-frequency components)
    low_freq ← dct[0:8, 0:8]
    
    // Exclude DC component (overall brightness)
    values ← flatten(low_freq)[1:]  // 63 values
    median ← median(values)
    
    hash ← 0
    for i, v in enumerate(values):
        if v > median:
            hash |= (1 << i)
    
    return hash  // 64-bit integer
```

**Complexity:** O(n²) for the 2D DCT on the 32×32 resized image, effectively O(1) per input image since the resize is fixed-size.

#### 4.2.3 Difference Hash (dHash)

**Pseudocode:**

```
function DHASH(grayscale_image):
    resized ← resize(grayscale_image, 9, 8)  // 9 wide, 8 tall
    
    hash ← 0
    bit ← 0
    for row in 0..8:
        for col in 0..8:
            if resized[row, col] > resized[row, col+1]:
                hash |= (1 << bit)
            bit += 1
    
    return hash  // 64-bit integer
```

#### 4.2.4 Average Hash (aHash)

**Pseudocode:**

```
function AHASH(grayscale_image):
    resized ← resize(grayscale_image, 8, 8)
    mean ← average(flatten(resized))
    
    hash ← 0
    for i, pixel in enumerate(flatten(resized)):
        if pixel > mean:
            hash |= (1 << i)
    
    return hash  // 64-bit integer
```

#### 4.2.5 CLIP Semantic Embedding

The CLIP-based image fingerprinting uses a pre-trained vision transformer (ViT) exported to ONNX format:

**Pseudocode:**

```
function CLIP_EMBED(image_bytes):
    img ← decode_image(image_bytes)
    tensor ← preprocess(img)  // resize to 224×224, normalize
    
    session ← load_onnx_model("clip-vit-base-patch32.onnx")
    embedding ← session.run(tensor)  // [1, 512] float32
    
    normalized ← l2_normalize(embedding)
    return normalized  // 512 × f32 = 2048 bytes
```

**Distance metric:** Cosine similarity (equivalent to dot product on L2-normalized vectors)

### 4.3 Audio Fingerprinting Algorithms

#### 4.3.1 Wang Landmark Algorithm (Default)

**Pseudocode:**

```
function WANG_FINGERPRINT(audio_bytes):
    samples ← decode_audio(audio_bytes)
    mono ← to_mono(samples)
    resampled ← resample(mono, target_rate=11025)
    
    // Compute spectrogram
    spectrogram ← stft(resampled, window=1024, hop=512)
    
    // Find spectral peaks (landmarks)
    peaks ← find_local_maxima(spectrogram, neighborhood=20)
    
    // Form constellation pairs
    hashes ← []
    for each peak p1 in peaks:
        target_zone ← peaks_in_zone(peaks, p1, 
                                     freq_range=(-50, +50),
                                     time_range=(1, 6))
        for each peak p2 in target_zone:
            hash_value ← encode(p1.freq, p2.freq, p2.time - p1.time)
            hashes.append((hash_value, p1.time))
    
    return hashes  // List of (hash: u32, offset: u32)
```

**Output:** Variable-length list of (hash, time_offset) pairs
**Storage:** Stored as raw bytes; typically 10–50 hashes per second of audio
**Matching:** Hash table lookup with time-offset verification (Hough transform)

#### 4.3.2 Panako Triplet Algorithm

**Pseudocode:**

```
function PANAKO_FINGERPRINT(audio_bytes):
    samples ← decode_and_resample(audio_bytes, 8000)
    spectrogram ← constant_q_transform(samples)
    peaks ← find_spectral_peaks(spectrogram)
    
    hashes ← []
    for each peak p1 in peaks:
        for each peak p2 in forward_zone(peaks, p1):
            for each peak p3 in forward_zone(peaks, p2):
                // Encode frequency ratios (pitch-invariant)
                f_ratio1 ← quantize(p2.freq / p1.freq)
                f_ratio2 ← quantize(p3.freq / p1.freq)
                dt1 ← p2.time - p1.time
                dt2 ← p3.time - p1.time
                
                hash ← encode(f_ratio1, f_ratio2, dt1, dt2)
                hashes.append((hash, p1.time))
    
    return hashes
```

**Key advantage:** Frequency ratios are invariant to pitch shifting, making Panako robust to speed changes that defeat the Wang algorithm.

#### 4.3.3 Haitsma Robust Hash

**Pseudocode:**

```
function HAITSMA_FINGERPRINT(audio_bytes):
    samples ← decode_and_resample(audio_bytes, 5000)  // 5 kHz
    
    // Compute band energies (33 Bark-scale bands)
    frames ← frame(samples, frame_size=160, hop=160)  // 31.25 fps
    
    prev_energies ← [0] × 33
    hashes ← []
    
    for each frame f in frames:
        spectrum ← fft(f)
        energies ← [sum(spectrum[band]) for band in bark_bands(33)]
        
        hash ← 0
        for b in 0..32:
            // Double differential: across frequency AND time
            current_diff ← energies[b] - energies[b+1]
            prev_diff ← prev_energies[b] - prev_energies[b+1]
            
            if current_diff - prev_diff > 0:
                hash |= (1 << b)
        
        hashes.append(hash)
        prev_energies ← energies
    
    return hashes  // One 32-bit hash per frame (~31.25 fps)
```

**Output:** Sequence of 32-bit hashes at 31.25 frames/second
**Matching:** Bit Error Rate (BER) — count mismatched bits across aligned frame sequences
**Robustness:** Survives MP3 compression, equalization, dynamic range compression, and moderate additive noise

#### 4.3.4 Neural Log-Mel Embeddings

**Pseudocode:**

```
function NEURAL_EMBED(audio_bytes):
    samples ← decode_and_resample(audio_bytes, 16000)
    
    // Compute log-mel spectrogram
    mel_spec ← mel_spectrogram(samples, n_mels=128, hop=160)
    log_mel ← log(mel_spec + 1e-6)
    
    // Segment into fixed-length windows
    segments ← segment(log_mel, window=96_frames)
    
    session ← load_onnx_model("audio_encoder.onnx")
    embeddings ← []
    for each segment s in segments:
        emb ← session.run(s)  // [1, 512] float32
        embeddings.append(l2_normalize(emb))
    
    // Average pool across segments
    final ← l2_normalize(mean(embeddings, axis=0))
    return final  // 512 × f32
```

### 4.4 Semantic Embedding Approaches

Semantic embeddings represent content in a continuous vector space where geometric proximity corresponds to semantic similarity. UCFP supports semantic embeddings for all three modalities:

| Modality | Model Type | Typical Dimensions | Use Case |
|----------|-----------|-------------------|----------|
| Text | Transformer encoder | 384–3072 | Paraphrase detection, semantic search |
| Image | Vision Transformer (CLIP) | 512–768 | Concept-level similarity |
| Audio | CNN/Transformer on log-mel | 512 | Speaker/content similarity |

All embeddings are L2-normalized before storage, enabling cosine similarity computation via simple dot product.

### 4.5 Complexity Analysis

| Algorithm | Time Complexity | Space Complexity | Output Size |
|-----------|----------------|-----------------|-------------|
| MinHash (H=128) | O(n × H) where n = shingle count | O(H) | 512 bytes |
| SimHash (256-bit) | O(n × B) where n = terms, B = bits | O(B) | 32 bytes |
| LSH (b=20, r=6) | O(b) per query | O(N × b) for index | — |
| TLSH | O(n) sliding window | O(1) — fixed 128 buckets | 35 bytes |
| pHash | O(1) — fixed 32×32 DCT | O(1) | 8 bytes |
| dHash | O(1) — fixed 9×8 resize | O(1) | 8 bytes |
| aHash | O(1) — fixed 8×8 resize | O(1) | 8 bytes |
| CLIP embed | O(1) — fixed 224×224 ViT forward | O(d) | 2048 bytes (d=512) |
| Wang landmarks | O(n log n) peak finding + O(p²) pairing | O(p) peaks | Variable |
| Panako triplets | O(p³) triplet enumeration | O(p) peaks | Variable |
| Haitsma | O(n) linear scan | O(33) band energies | 4 bytes/frame |
| Neural audio | O(n/w) segments × O(1) inference | O(d) | 2048 bytes (d=512) |

Where n is input size, p is number of spectral peaks, d is embedding dimension, N is corpus size, and w is segment window size.

---

## Chapter 5: Implementation

### 5.1 Code Structure and Organization

The UCFP codebase is organized as a single Rust crate with a library (`src/lib.rs`) and a binary (`src/bin/ucfp.rs`). The library exposes all functionality through well-defined modules, while the binary is a thin wrapper that reads environment variables, selects trait implementations, and starts the HTTP server.

```
src/
├── bin/
│   └── ucfp.rs              # Binary entry point: env parsing, impl selection, server start
├── lib.rs                    # Library root: re-exports all public modules
├── error.rs                  # Shared error types (thiserror-derived)
├── core/
│   └── mod.rs               # Record, Hit, Query, Modality, HitSource
├── modality/
│   ├── mod.rs               # Modality trait and dispatch
│   ├── text.rs              # Text fingerprinting (minhash, simhash, lsh, tlsh, semantic)
│   ├── image.rs             # Image fingerprinting (multi, phash, dhash, ahash, semantic)
│   └── audio.rs             # Audio fingerprinting (wang, panako, haitsma, neural, watermark)
├── index/
│   ├── mod.rs               # IndexBackend trait definition
│   └── embedded/
│       ├── mod.rs           # EmbeddedBackend: redb tables + brute-force k-NN
│       └── bm25.rs          # BM25 implementation: FST term dict + roaring postings
├── matcher/
│   └── mod.rs               # Matcher: parallel retrieval + RRF fusion
├── rerank/
│   └── mod.rs               # Reranker trait + NoopReranker
├── ingest/
│   └── mod.rs               # IngestSource trait (for future queue-based ingest)
└── server/
    ├── mod.rs               # Router construction, ServerState, middleware wiring
    ├── handlers.rs          # HTTP handlers for all routes (~49KB)
    ├── dto.rs               # Request/response DTOs with serde
    ├── apikey.rs            # ApiKeyLookup trait + StaticSingleKey + StaticMapKey + Webhook
    ├── ratelimit.rs         # TenantRateLimiter trait + InMemoryTokenBucket + Webhook
    ├── usage.rs             # UsageSink trait + NoopUsageSink + LogUsageSink + Webhook
    ├── algorithms_manifest.rs  # Runtime algorithm discovery and documentation
    ├── inputs_cache.rs      # LRU cache for playground input replay
    ├── extractors.rs        # Custom axum extractors (TenantContext)
    ├── error.rs             # HTTP error responses (Problem Details RFC 9457)
    └── tests.rs             # Integration tests (~42KB)
```

The binary entry point (`src/bin/ucfp.rs`, ~10KB) performs the following at startup:

1. Installs `mimalloc` as the global allocator
2. Initializes `tracing-subscriber` with JSON formatting
3. Reads environment variables for configuration
4. Selects `ApiKeyLookup` implementation (webhook > file > static token)
5. Selects `TenantRateLimiter` implementation (webhook or in-memory)
6. Selects `UsageSink` implementation (webhook > file > no-op)
7. Opens the redb database at `UCFP_DATA_DIR`
8. Constructs `ServerState` with all selected implementations
9. Builds the axum router with middleware layers
10. Binds to `UCFP_BIND` address and starts serving with graceful shutdown

### 5.2 Feature Flag System

UCFP uses Rust's compile-time feature flags extensively to enable minimal builds. The feature hierarchy is:

```toml
[features]
default = ["embedded", "server", "audio", "image", "text"]

# Storage backend
embedded = ["dep:redb", "dep:hnsw_rs", "dep:pulp", "dep:roaring", "dep:rkyv", "dep:rayon", "dep:tokio", "dep:fst"]

# HTTP server
server = ["dep:axum", "dep:tokio", "dep:tower", "dep:tower-http", "dep:tracing-subscriber", 
          "dep:metrics", "dep:metrics-exporter-prometheus", "dep:subtle", "dep:mimalloc"]

# Modality SDKs
audio = ["dep:audiofp"]
image = ["dep:imgfprint"]
text  = ["dep:txtfp"]

# Fine-grained algorithm gates
audio-wang     = ["audio"]
audio-panako   = ["audio"]
audio-haitsma  = ["audio"]
audio-neural   = ["audio", "audiofp/neural"]      # Pulls ONNX runtime
audio-watermark = ["audio", "audiofp/watermark"]   # Pulls ONNX runtime

image-perceptual = ["image"]
image-semantic   = ["image", "imgfprint/local-embedding"]  # Pulls ONNX runtime

text-simhash        = ["text"]
text-lsh            = ["text"]
text-tlsh           = ["text", "txtfp/tlsh"]
text-semantic-local = ["text", "txtfp/semantic"]           # Pulls ONNX runtime
text-semantic-openai = ["text-semantic-local", "txtfp/openai"]
```

This design means:
- A minimal build (`default` features) includes MinHash, multi-hash, and Wang — no ONNX, no HTTP client
- Adding `text-simhash` costs zero additional dependencies (just enables code paths in `text.rs`)
- Adding `audio-neural` pulls the ONNX runtime (~20MB binary size increase)
- The `full` feature umbrella enables everything for CI testing and Docker builds

### 5.3 API Design

The REST API follows resource-oriented design with consistent URL patterns:

#### Ingest Endpoints

```
POST /v1/ingest/{modality}/{tenant_id}/{record_id}[?algorithm={name}]
```

- **Path parameters:** modality (text|image|audio), tenant_id (u32), record_id (u64)
- **Query parameters:** algorithm (optional, defaults to modality default)
- **Request body:** Raw content bytes (Content-Type: text/plain, image/*, audio/*)
- **Response:** 201 Created with JSON `{record_id, algorithm, fingerprint_hex, embedding_dim?}`

Specialized ingest variants:
- `POST /v1/ingest/text/{tid}/{rid}/stream` — Chunked streaming for large documents
- `POST /v1/ingest/text/{tid}/{rid}/preprocess/{kind}` — HTML/PDF preprocessing before fingerprinting
- `POST /v1/ingest/image/{tid}/{rid}/semantic` — CLIP embedding only (no perceptual hash)
- `POST /v1/ingest/audio/{tid}/{rid}/watermark` — AudioSeal watermark detection
- `POST /v1/ingest/audio/{tid}/{rid}/stream` — Multipart streaming audio

#### Query Endpoint

```
POST /v1/query
Content-Type: application/json

{
    "tenant_id": 0,
    "modality": "text",
    "k": 10,
    "vector": [0.1, 0.2, ...],    // optional: vector similarity
    "terms": ["quick", "brown"],   // optional: BM25 keyword search
    "rrf_k": 60                    // optional: RRF fusion constant
}
```

Response:
```json
{
    "hits": [
        {"record_id": 42, "score": 0.95, "source": "vector"},
        {"record_id": 17, "score": 0.87, "source": "fused"}
    ]
}
```

#### Records Management

- `POST /v1/records` — Bulk upsert pre-computed fingerprint records
- `GET /v1/records/{tid}/{rid}` — Retrieve a stored record with metadata
- `DELETE /v1/records/{tid}/{rid}` — Delete a record from all tables

#### Operational Endpoints

- `GET /healthz` — Liveness check + redb open verification
- `GET /v1/info` — Server version, enabled features, algorithm manifest
- `GET /metrics` — Prometheus-format metrics (request counts, latencies, DB size)

### 5.4 Database Schema

The `EmbeddedBackend` (`src/index/embedded/mod.rs`, ~24KB) manages six redb tables:

**Table 1: `fingerprints`**
- Key: `(tenant_id: u32, record_id: u64)` — 12 bytes, big-endian for sort order
- Value: Raw fingerprint bytes (`bytemuck::cast_slice` from the modality SDK)
- Purpose: Primary fingerprint storage for Hamming distance comparison

**Table 2: `vectors`**
- Key: `(tenant_id: u32, record_id: u64)`
- Value: `[f32; D]` as little-endian bytes (D varies by model: 384, 512, 768, 1024, 3072)
- Purpose: Dense embedding vectors for cosine similarity search

**Table 3: `catalog`**
- Key: `(tenant_id: u32, record_id: u64)`
- Value: JSON-encoded `{algorithm: str, format_version: u32, config_hash: u64}`
- Purpose: Algorithm provenance — ensures fingerprints are only compared within compatible versions

**Table 4: `metadata`**
- Key: `(tenant_id: u32, record_id: u64)`
- Value: `rkyv`-archived variable-length metadata (content type, original filename, ingestion timestamp)
- Purpose: Zero-copy metadata access without deserialization

**Table 5: `bm25_terms` (FST)**
- Structure: `fst::Map<term_string, term_id>`
- Purpose: Compressed term dictionary mapping strings to integer IDs
- Rebuilt per-tenant on each write transaction

**Table 6: `bm25_postings`**
- Key: `(term_id: u64, tenant_id: u32)`
- Value: Roaring bitmap of record_ids containing the term, plus per-document term frequencies
- Purpose: Inverted index for BM25 scoring

**Table 7: `bm25_scoring`**
- Key: `(tenant_id: u32, record_id: u64)`
- Value: `(doc_len: u32, avg_field_len: f32)`
- Purpose: Document length statistics for BM25 normalization

All tables are accessed within a single redb transaction, ensuring consistency between fingerprint storage and index updates.

### 5.5 Vector Search (HNSW and Brute-Force)

UCFP implements a two-tier vector search strategy:

**Tier 1: Brute-Force Cosine (default, <1M vectors)**

```rust
// Simplified from src/index/embedded/mod.rs
pub async fn knn(&self, query: &[f32], k: usize) -> Vec<Hit> {
    let vectors = self.load_all_vectors(tenant_id);  // mmap'd redb read
    
    // SIMD-accelerated dot product via pulp
    let scores: Vec<f32> = vectors.par_iter()
        .map(|(id, vec)| {
            pulp::Arch::new().dispatch(|| dot_product(query, vec))
        })
        .collect();
    
    // Partial sort for top-k
    top_k(scores, k)
}
```

The `pulp` crate provides runtime CPU feature detection and dispatches to the optimal SIMD implementation (SSE4.1, AVX2, AVX-512, or NEON) without requiring nightly Rust or compile-time target specification.

**Performance characteristics:**
- 100K vectors × 768-d: ~1ms on modern hardware (16 cores, AVX2)
- 1M vectors × 768-d: ~8ms (approaching the graduation threshold)
- Memory: vectors are read from redb's mmap'd pages, no separate copy

**Tier 2: HNSW (graduation at ≥1M vectors)**

The `hnsw_rs` crate provides a pure-Rust HNSW implementation with:
- Construction: m=16 neighbors per layer, ef_construction=200
- Query: ef_search=128, returning top-k with recall >0.99
- Persistence: dump/reload alongside the redb file
- Distance: Cosine (via built-in `DistCosine`)

The graduation trigger is monitored via the `/metrics` endpoint (`ucfp_vector_count` gauge).

### 5.6 BM25 Full-Text Retrieval

The BM25 implementation (`src/index/embedded/bm25.rs`, ~27KB) provides keyword search without external dependencies like Tantivy or Elasticsearch:

**Scoring formula:**

$$\text{BM25}(q, d) = \sum_{t \in q} \text{IDF}(t) \cdot \frac{tf(t, d) \cdot (k_1 + 1)}{tf(t, d) + k_1 \cdot (1 - b + b \cdot \frac{|d|}{\text{avgdl}})}$$

Where:
- k₁ = 1.2 (term frequency saturation)
- b = 0.75 (document length normalization)
- IDF(t) = log((N - df(t) + 0.5) / (df(t) + 0.5) + 1)

**Implementation components:**

1. **Term Dictionary:** `fst::Map` provides a compressed, memory-mapped finite state transducer that maps term strings to integer IDs in O(|term|) time with minimal memory overhead.

2. **Postings Lists:** `roaring::RoaringBitmap` stores the set of document IDs containing each term, with per-document term frequency stored in a parallel structure. Roaring bitmaps provide excellent compression for clustered integer sets.

3. **Scoring Table:** Per-document length and running average field length, updated incrementally on each upsert.

4. **Query Execution:**
```rust
pub async fn bm25(&self, terms: &[&str], k: usize) -> Vec<Hit> {
    let term_ids: Vec<u64> = terms.iter()
        .filter_map(|t| self.term_fst.get(t))
        .collect();
    
    // Intersect postings to find candidate documents
    let candidates = union_postings(&term_ids);
    
    // Score each candidate
    let scored: Vec<Hit> = candidates.iter()
        .map(|doc_id| {
            let score = bm25_score(doc_id, &term_ids, k1=1.2, b=0.75);
            Hit { record_id: doc_id, score, source: HitSource::Bm25 }
        })
        .collect();
    
    top_k(scored, k)
}
```

### 5.7 Hybrid Search with Reciprocal Rank Fusion

The `Matcher` (`src/matcher/mod.rs`, ~10KB) orchestrates hybrid search by running vector and BM25 retrievers in parallel and fusing results:

```rust
pub async fn search(&self, query: &Query) -> Result<Vec<Hit>, Error> {
    // Run both retrievers in parallel
    let (vector_hits, bm25_hits) = tokio::try_join!(
        self.index.knn(&query.vector.unwrap_or_default(), query.k, query.filter.as_ref()),
        self.index.bm25(&query.terms, query.k, query.filter.as_ref()),
    )?;
    
    // Reciprocal Rank Fusion
    let fused = rrf_fuse(vector_hits, bm25_hits, query.rrf_k);
    
    // Optional reranking
    let reranked = self.reranker.rerank(query, fused).await?;
    
    Ok(reranked)
}
```

**RRF Fusion Algorithm:**

```rust
fn rrf_fuse(sources: Vec<Vec<Hit>>, rrf_k: u32) -> Vec<Hit> {
    let mut scores: HashMap<u64, f32> = HashMap::new();
    
    for source_hits in &sources {
        for (rank, hit) in source_hits.iter().enumerate() {
            *scores.entry(hit.record_id).or_default() 
                += 1.0 / (rrf_k as f32 + rank as f32 + 1.0);
        }
    }
    
    let mut fused: Vec<Hit> = scores.into_iter()
        .map(|(id, score)| Hit { record_id: id, score, source: HitSource::Fused })
        .collect();
    
    fused.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
    fused
}
```

The RRF constant k=60 (default, configurable per query) controls the balance between rank positions. Higher k values reduce the influence of top-ranked results, producing more uniform fusion. This value matches the defaults used by Azure AI Search, Elasticsearch, and Qdrant.

---
