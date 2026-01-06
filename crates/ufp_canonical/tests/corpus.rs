use ufp_canonical::{canonicalize, CanonicalizeConfig};

struct Case {
    name: &'static str,
    input: &'static str,
    cfg: CanonicalizeConfig,
    expected_text: &'static str,
    expected_tokens: &'static [(&'static str, usize, usize)],
}

#[test]
fn golden_corpus_regression() {
    let cases = [
        Case {
            name: "ascii_whitespace_collapse",
            input: "  Hello   world  ",
            cfg: CanonicalizeConfig::default(),
            expected_text: "hello world",
            expected_tokens: &[("hello", 0, 5), ("world", 6, 11)],
        },
        Case {
            name: "unicode_combining_marks",
            input: "Caf\u{00E9} cafe\u{0301}",
            cfg: CanonicalizeConfig::default(),
            expected_text: "café café",
            expected_tokens: &[
                // "café" is 5 bytes in UTF-8; "café " is 6; "café café" is 11.
                ("café", 0, 5),
                ("café", 6, 11),
            ],
        },
        Case {
            name: "non_bmp_codepoint",
            input: " a\u{10348}b  c ",
            cfg: CanonicalizeConfig::default(),
            expected_text: "a\u{10348}b c",
            expected_tokens: &[
                // "a\u{10348}b" is 6 bytes; "a\u{10348}b " is 7; "a\u{10348}b c" is 8.
                ("a\u{10348}b", 0, 6),
                ("c", 7, 8),
            ],
        },
    ];

    for case in cases {
        let doc = canonicalize(case.name, case.input, &case.cfg)
            .unwrap_or_else(|e| panic!("case {} failed: {e}", case.name));

        assert_eq!(
            doc.canonical_text, case.expected_text,
            "text mismatch for {}",
            case.name
        );

        let tokens: Vec<(String, usize, usize)> = doc
            .tokens
            .iter()
            .map(|t| (t.text.clone(), t.start, t.end))
            .collect();
        let expected: Vec<(String, usize, usize)> = case
            .expected_tokens
            .iter()
            .map(|(text, s, e)| (text.to_string(), *s, *e))
            .collect();
        assert_eq!(tokens, expected, "tokens mismatch for {}", case.name);

        // Sanity: token_hashes must align 1:1 with tokens.
        assert_eq!(
            doc.tokens.len(),
            doc.token_hashes.len(),
            "hash alignment for {}",
            case.name
        );
    }
}
