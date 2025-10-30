# UCFP Canonicalizer — Usage Guide

## Overview

This document explains what the **UCFP canonicalizer** does, how it works, and how to run and test it with your own text files.

The canonicalizer is responsible for transforming any given text into a **deterministic, normalized form**, splitting it into tokens, and generating a **SHA-256 hash fingerprint**. This ensures that two texts which look slightly different but mean the same thing (e.g., punctuation or case differences) yield the same canonical representation.

* * *

## 1\. What It Does

The canonicalizer performs the following steps:

1.  **Reads text input** — from a file or a string.

2.  **Normalizes Unicode (NFKC)** — ensures consistent encoding for special characters.

3.  **Lowercases** all text (configurable).

4.  **Removes punctuation** (configurable).

5.  **Collapses multiple whitespaces/newlines** into single spaces.

6.  **Splits into tokens** — each word is extracted with its byte offsets.

7.  **Computes SHA-256 checksum** of the final canonical text.


This creates a stable, comparable representation for text fingerprinting.

* * *

## 2\. Example Code

```rust
use std::fs;
    use ufp_canonical::{canonicalize, CanonicalizeConfig};
    
    fn main() {
        // Read input file
        let file_path = "test.txt";
        let content = fs::read_to_string(file_path).expect("Failed to read file");
    
        // Configure canonicalization
        let cfg = CanonicalizeConfig { strip_punctuation: true, lowercase: true };
    
        // Run canonicalizer
        let doc = canonicalize(&content, &cfg);
    
        // Print results
        println!("canonical: {}", doc.canonical_text);
        println!("tokens: {:?}", doc.tokens);
        println!("sha256: {}", doc.sha256_hex);
    }
```


* * *

## 3\. Example Input

**File: `test.txt`**

    Hello, WORLD!  This   is a Test.


* * *

## 4\. Example Output

    canonical: hello world this is a test
    tokens: ["hello", "world", "this", "is", "a", "test"]
    sha256: e7b8b50b4c4c90a58e9e4f6d87ac...


* * *

## 5\. How It Works Internally

| Step | Operation | Example Before | Example After |
| --- | --- | --- | --- |
| 1 | Unicode normalize (NFKC) | “é” + combining | "é" (single composed) |
| 2 | Lowercase | HELLO | hello |
| 3 | Strip punctuation | Hello, world! | Hello world |
| 4 | Collapse whitespace | Hello world | Hello world |
| 5 | Tokenize | Hello world | [hello, world] |
| 6 | Checksum | (whole text) | sha256 = ... |

* * *

## 6\. Why It’s Important

The canonicalizer guarantees that different representations of the same text yield **identical canonical outputs** — this is crucial for:

*   **Deduplication** — detect duplicates even if formatting differs.

*   **Plagiarism detection** — normalize before comparison.

*   **Search and indexing** — ensure consistent fingerprints.

*   **Perceptual fingerprinting** — provides the clean input for shingling + winnowing.


* * *

## 7\. Running It

### Option A — From a file

    echo "Hello, WORLD! This is a test." > test.txt
    cargo run -- test.txt


### Option B — Inline text (for debugging)

    let doc = canonicalize("Hello, WORLD! This is a test.", &cfg);


* * *

## 8\. Expected Results

    canonical: hello world this is a test
    tokens: ["hello", "world", "this", "is", "a", "test"]
    sha256: <deterministic hash>


* * *

## 9\. Integration in UCFP

    Raw Input (text / file)
            │
            ▼
    [ ufp_ingest ]  →  CanonicalIngestRecord
            │
            ▼
    [ ufp_canonical ]  → CanonicalizedDocument (text + tokens + checksum)
            │
            ▼
    [ ufp_perceptual ] → Shingles + Winnowing + MinHash


* * *

## 10\. Next Step

Continue by integrating this canonical output into the **Perceptual Fingerprinting Layer** (`ufp_perceptual`), where shingles and hashes are generated for fuzzy matching.