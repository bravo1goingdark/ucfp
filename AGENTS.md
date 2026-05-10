<!-- recon:start -->
## recon MCP tools — strict policy

For all code exploration in this repository, you MUST use the `code_*` tools provided by the recon MCP server:

- Reading code: `code_outline`, `code_skeleton`, `code_read_symbol` (instead of `Read`)
- Searching: `code_find_symbol`, `code_find_refs`, `code_search`, `code_find_strings`, `code_multi_find` (instead of `Grep`)
- Listing / orientation: `code_list`, `code_repo_map` (instead of `Glob`)
- Index health: `code_reindex`

Do **not** use `Read`, `Grep`, or `Glob` on source files by default.

**Exception.** If no `code_*` tool can answer the question — for example a non-source file (JSON config, Markdown doc, generated asset), a freshly created file the index has not picked up yet, or the recon index is unavailable — you MAY use `Read`, `Grep`, or `Glob`, but only after:

1. Stopping and asking the user for explicit permission, and
2. Explaining which `code_*` tool you tried and the specific reason it could not answer.

Do not silently fall back. The whole point of recon is the 15–30× token reduction; defaulting to `Read`/`Grep`/`Glob` defeats it.
<!-- recon:end -->
