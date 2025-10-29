# 🧠 Universal Content Fingerprinting (UCFP)

**UCFP** is a **high-performance, multimodal content fingerprinting framework** built in **Rust**, designed to identify, deduplicate, and semantically match **text, images, audio, video, and documents** — even after transformation, re-encoding, or paraphrasing.

It creates **multi-layer “universal fingerprints”** that combine:
- 🔒 **Exact hashes (SHA-256)** — byte-level deduplication  
- 🧩 **Perceptual hashes** — robust to edits, compression, or noise  
- 🧠 **Semantic embeddings** — deep encoders (CLIP, SBERT, OpenL3) for content-aware similarity  
- 🧱 **Structural & metadata signatures** — document layout, ASTs, media stats  

---

## ✨ Key Features

| Layer | Purpose | Example Implementation |
|--------|----------|------------------------|
| **Exact** | Detect byte-identical content | SHA-256 of canonical bytes |
| **Perceptual** | Detect near-duplicates | pHash/dHash for images, landmarks for audio, MinHash for text |
| **Semantic** | Detect paraphrased or cross-modal content | CLIP, SBERT, or OpenL3 embeddings |
| **Structural/Metadata** | Contextual verification | Document layout, AST, duration, dominant colors |




