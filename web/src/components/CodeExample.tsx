import { motion } from 'framer-motion'
import { Copy, Check, Terminal } from 'lucide-react'
import { useState } from 'react'
import './CodeExample.css'

const codeExample = `use ucfp::{
    CanonicalizeConfig, IngestConfig, IngestPayload, 
    IngestSource, PerceptualConfig, RawIngestRecord,
    SemanticConfig, process_record_with_perceptual,
    semanticize_document,
};
use ucfp_index::{BackendConfig, IndexConfig, IndexRecord, UfpIndex};
use ucfp_matcher::{DefaultMatcher, MatchConfig, MatchRequest, Matcher};

// Configure pipeline stages
let ingest_cfg = IngestConfig::default();
let canonical_cfg = CanonicalizeConfig::default();
let perceptual_cfg = PerceptualConfig::default();
let semantic_cfg = SemanticConfig::default();

// Create index
let index_cfg = IndexConfig::new()
    .with_backend(BackendConfig::InMemory);
let index = UfpIndex::new(index_cfg).unwrap();

// Ingest document
let record = RawIngestRecord {
    id: "doc-001".into(),
    source: IngestSource::RawText,
    payload: Some(IngestPayload::Text(
        "Rust memory safety features".into()
    )),
    ..Default::default()
};

// Process through pipeline
let (doc, fingerprint) = process_record_with_perceptual(
    record, &canonical_cfg, &perceptual_cfg
)?;

// Generate embedding
let embedding = semanticize_document(&doc, &semantic_cfg)?;

// Store in index
let record = IndexRecord {
    doc_id: doc.doc_id.clone(),
    canonical_hash: doc.canonical_hash.clone(),
    perceptual_fingerprint: Some(fingerprint),
    semantic_embedding: Some(embedding),
    ..Default::default()
};
index.upsert(record)?;

// Search with matcher
let matcher = DefaultMatcher::new(
    index, ingest_cfg, canonical_cfg,
    perceptual_cfg, semantic_cfg,
);

let req = MatchRequest {
    query_text: "Rust safety".to_string(),
    config: MatchConfig::default(),
    ..Default::default()
};

let hits = matcher.match_document(&req)?;
println!("Found {} matches", hits.len());`

export default function CodeExample() {
  const [copied, setCopied] = useState(false)

  const handleCopy = () => {
    navigator.clipboard.writeText(codeExample)
    setCopied(true)
    setTimeout(() => setCopied(false), 2000)
  }

  return (
    <section id="code" className="code-section">
      <motion.div
        className="section-header"
        initial={{ opacity: 0, y: 30 }}
        whileInView={{ opacity: 1, y: 0 }}
        viewport={{ once: true }}
        transition={{ duration: 0.6 }}
      >
        <h2 className="section-title">
          Simple, <span className="gradient-text">Powerful</span> API
        </h2>
        <p className="section-subtitle">
          Get started in minutes with our intuitive Rust API. Full pipeline
          from ingest to matching in just a few lines of code.
        </p>
      </motion.div>

      <motion.div
        className="code-container"
        initial={{ opacity: 0, y: 30 }}
        whileInView={{ opacity: 1, y: 0 }}
        viewport={{ once: true }}
        transition={{ duration: 0.6, delay: 0.2 }}
      >
        <div className="code-header">
          <div className="code-header-left">
            <Terminal size={18} />
            <span>main.rs</span>
          </div>
          <button className="copy-btn" onClick={handleCopy}>
            {copied ? <Check size={18} /> : <Copy size={18} />}
            <span>{copied ? 'Copied!' : 'Copy'}</span>
          </button>
        </div>
        <div className="code-content">
          <pre>
            <code>{codeExample}</code>
          </pre>
        </div>
      </motion.div>

      <motion.div
        className="cta-section"
        initial={{ opacity: 0, y: 30 }}
        whileInView={{ opacity: 1, y: 0 }}
        viewport={{ once: true }}
        transition={{ duration: 0.6, delay: 0.3 }}
      >
        <h3 className="cta-title">Ready to get started?</h3>
        <p className="cta-subtitle">
          Join the community and start building with UCFP today.
        </p>
        <div className="cta-buttons">
          <a
            href="https://github.com/bravo1goingdark/ucfp"
            target="_blank"
            rel="noopener noreferrer"
            className="btn btn-primary"
          >
            View on GitHub
          </a>
          <a href="#features" className="btn btn-secondary">
            Read Documentation
          </a>
        </div>
      </motion.div>
    </section>
  )
}
