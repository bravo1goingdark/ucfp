// Markdown-driven docs.
//
// Each `src/lib/docs/*.md` file is a Markdown body preceded by a tiny
// frontmatter block:
//
//   ---
//   title: …
//   order: 1
//   description: …
//   ---
//
// We parse frontmatter with a hand-rolled 30-line parser (no gray-matter
// dep), render Markdown with `marked`, and syntax-highlight code blocks
// with `shiki` using both `github-light` and `github-dark` themes. The
// dual themes are emitted as <pre class="shiki shiki-themes …"> blocks
// whose contents flip based on the document `[data-theme]` attribute.
//
// IMPORTANT: this module lives in `$lib/server/`, so it is server-only
// and never reaches the client bundle. Both `+page.server.ts` files in
// `routes/(marketing)/docs` import from here.

import { Marked, Renderer } from 'marked';
import {
  createHighlighter,
  type Highlighter,
  type BundledLanguage,
  type BundledTheme
} from 'shiki';

export type Doc = {
  slug: string;
  title: string;
  order: number;
  description: string;
  category: string;
  body: string;       // raw markdown (frontmatter stripped)
  html: string;       // rendered + highlighted html
  headings: { id: string; text: string }[]; // <h2> only, for the right-rail TOC
};

type Frontmatter = {
  title: string;
  order: number;
  description: string;
  category: string;
};

// ── Frontmatter ──────────────────────────────────────────────────────────
//
// Minimal YAML-ish parser. Accepts:
//   ---
//   key: value
//   key: "value with spaces"
//   key: 3
//   ---
// One key per line, no nesting, no arrays. That is all our docs use.
function parseFrontmatter(raw: string): { fm: Frontmatter; body: string } {
  const match = raw.match(/^---\r?\n([\s\S]*?)\r?\n---\r?\n([\s\S]*)$/);
  if (!match) {
    return {
      fm: { title: 'Untitled', order: 999, description: '', category: 'Docs' },
      body: raw
    };
  }
  const [, head, body] = match;
  const fm: Record<string, string | number> = {};
  for (const line of head.split(/\r?\n/)) {
    const m = line.match(/^([A-Za-z_][A-Za-z0-9_-]*)\s*:\s*(.*)$/);
    if (!m) continue;
    let value: string | number = m[2].trim();
    // strip wrapping quotes
    if (
      (value.startsWith('"') && value.endsWith('"')) ||
      (value.startsWith("'") && value.endsWith("'"))
    ) {
      value = value.slice(1, -1);
    }
    // numeric coercion for `order`
    if (/^-?\d+(\.\d+)?$/.test(value as string)) value = Number(value);
    fm[m[1]] = value;
  }
  return {
    fm: {
      title: String(fm.title ?? 'Untitled'),
      order: typeof fm.order === 'number' ? fm.order : 999,
      description: String(fm.description ?? ''),
      category: String(fm.category ?? 'Docs')
    },
    body
  };
}

// ── Highlighter (cached at module load) ──────────────────────────────────
//
// `createHighlighter` is async and ~megabytes; we make sure exactly one
// instance ever exists per Worker process by caching the promise.

const LANGS: BundledLanguage[] = [
  'bash',
  'json',
  'typescript',
  'javascript',
  'python',
  'rust',
  'http',
  'sql'
];

const THEMES: BundledTheme[] = ['github-light', 'github-dark'];

let highlighterPromise: Promise<Highlighter> | null = null;

function getHighlighter(): Promise<Highlighter> {
  if (!highlighterPromise) {
    highlighterPromise = createHighlighter({
      themes: THEMES,
      langs: LANGS
    });
  }
  return highlighterPromise;
}

// Map language aliases the docs might use to a Shiki language we loaded.
function normalizeLang(lang: string | undefined): BundledLanguage {
  const v = (lang ?? '').toLowerCase().trim();
  if (v === 'ts') return 'typescript';
  if (v === 'js') return 'javascript';
  if (v === 'sh' || v === 'shell' || v === 'console') return 'bash';
  if (v === 'py') return 'python';
  if ((LANGS as string[]).includes(v)) return v as BundledLanguage;
  return 'bash'; // safe fallback for plain ``` blocks
}

// ── Markdown renderer ────────────────────────────────────────────────────
//
// Marked v14 supports an async pipeline via `walkTokens`. We mutate code
// tokens to carry their highlighted HTML, then return that HTML directly
// from `renderer.code` (with `escaped = true`, so marked won't re-escape).

async function renderMarkdown(md: string): Promise<{ html: string; headings: { id: string; text: string }[] }> {
  const hl = await getHighlighter();
  const headings: { id: string; text: string }[] = [];

  // Use a fresh Marked instance each call — marked.use() is additive on the
  // global instance, which causes walkTokens to fire N times on the Nth doc
  // (producing exponentially bloated HTML).
  const renderer = new Renderer();
  // walkTokens has replaced `token.text` with the shiki HTML; we wrap it
  // here with a header bar carrying the language label + a copy button.
  // The pre.shiki block stays inside so existing CSS still applies.
  renderer.code = function ({ text, lang }: { text: string; lang?: string }) {
    const display = displayLang(lang);
    return (
      `<div class="code-block" data-lang="${escapeAttr(display)}">` +
      `<div class="code-block-header">` +
      `<span class="code-block-lang">${escapeHtml(display)}</span>` +
      `<button class="copy-btn" type="button" aria-label="Copy code">Copy</button>` +
      `</div>` +
      text +
      `</div>\n`
    );
  };
  renderer.heading = function (this: { parser: { parseInline: (tokens: unknown[]) => string } }, token: { depth: number; text: string; tokens: unknown[] }) {
    const inner = this.parser.parseInline(token.tokens);
    if (token.depth === 2) {
      const id = slugify(token.text);
      headings.push({ id, text: token.text });
      return `<h2 id="${id}">${inner}</h2>\n`;
    }
    return `<h${token.depth}>${inner}</h${token.depth}>\n`;
  };
  // GFM-style admonitions: a blockquote whose first line is `[!NOTE]`,
  // `[!WARNING]`, `[!TIP]`, `[!INFO]`, `[!IMPORTANT]`, or `[!CAUTION]`
  // gets rendered as a callout div instead of a <blockquote>.
  renderer.blockquote = function (this: { parser: { parse: (tokens: unknown[]) => string } }, token: { tokens: unknown[] }) {
    const inner = this.parser.parse(token.tokens);
    const m = inner.match(/^<p>\s*\[!(NOTE|TIP|WARNING|INFO|IMPORTANT|CAUTION)\]\s*([\s\S]*?)<\/p>\n?([\s\S]*)$/);
    if (m) {
      const kind = m[1].toLowerCase();
      const firstLine = m[2].trim();
      const rest = m[3];
      const body = (firstLine ? `<p>${firstLine}</p>\n` : '') + rest;
      return (
        `<div class="callout callout-${kind}">` +
        `<div class="callout-title">${calloutLabel(kind)}</div>` +
        `<div class="callout-body">${body}</div>` +
        `</div>\n`
      );
    }
    return `<blockquote>${inner}</blockquote>\n`;
  };

  const instance = new Marked({
    async: true,
    gfm: true,
    breaks: false,
    renderer,
    walkTokens: async (token) => {
      if (token.type === 'code') {
        const lang = normalizeLang(token.lang);
        try {
          const html = hl.codeToHtml(token.text, {
            lang,
            themes: { light: 'github-light', dark: 'github-dark' },
            defaultColor: false
          });
          token.text = html;
          (token as unknown as { escaped: boolean }).escaped = true;
        } catch {
          token.text = `<pre class="code"><code>${escapeHtml(token.text)}</code></pre>`;
          (token as unknown as { escaped: boolean }).escaped = true;
        }
      }
    }
  });

  const html = await instance.parse(md);
  return { html: html as string, headings };
}

function slugify(s: string): string {
  return s
    .toLowerCase()
    .replace(/[^a-z0-9\s-]/g, '')
    .trim()
    .replace(/\s+/g, '-')
    .slice(0, 80);
}

function escapeHtml(s: string): string {
  return s
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}

function escapeAttr(s: string): string {
  return s.replace(/&/g, '&amp;').replace(/"/g, '&quot;');
}

function displayLang(lang: string | undefined): string {
  const v = (lang ?? '').toLowerCase().trim();
  if (!v) return 'text';
  if (v === 'sh' || v === 'shell' || v === 'console') return 'bash';
  if (v === 'ts') return 'typescript';
  if (v === 'js') return 'javascript';
  if (v === 'py') return 'python';
  return v;
}

function calloutLabel(kind: string): string {
  switch (kind) {
    case 'note':
      return 'Note';
    case 'tip':
      return 'Tip';
    case 'warning':
      return 'Warning';
    case 'info':
      return 'Info';
    case 'important':
      return 'Important';
    case 'caution':
      return 'Caution';
    default:
      return kind;
  }
}

// ── Public API ───────────────────────────────────────────────────────────
//
// `import.meta.glob` with `eager: true` + `query: '?raw'` + `import: 'default'`
// gives us a synchronous { '../docs/foo.md': string } map at build time.
// SvelteKit + Vite resolves this for both prerender and runtime.

const files = import.meta.glob('../docs/*.md', {
  eager: true,
  query: '?raw',
  import: 'default'
}) as Record<string, string>;

// In-memory cache of fully rendered docs. Rebuilt once per Worker
// instance — prerender invokes `loadDocs()` per page and we don't want
// to re-run shiki for every call.
let docsCache: Promise<Doc[]> | null = null;

export function loadDocs(): Promise<Doc[]> {
  if (!docsCache) docsCache = buildAll();
  return docsCache;
}

async function buildAll(): Promise<Doc[]> {
  const out: Doc[] = [];
  for (const [path, raw] of Object.entries(files)) {
    const slug = slugFromPath(path);
    const { fm, body } = parseFrontmatter(raw);
    const { html, headings } = await renderMarkdown(body);
    out.push({
      slug,
      title: fm.title,
      order: fm.order,
      description: fm.description,
      category: fm.category,
      body,
      html,
      headings
    });
  }
  out.sort((a, b) => a.order - b.order || a.title.localeCompare(b.title));
  return out;
}

function slugFromPath(path: string): string {
  const m = path.match(/([^/]+)\.md$/);
  return m ? m[1] : path;
}

export async function getDoc(slug: string): Promise<Doc | null> {
  const docs = await loadDocs();
  return docs.find((d) => d.slug === slug) ?? null;
}

/** Used by [slug]/+page.server.ts `entries()` for prerender enumeration. */
export function listSlugs(): string[] {
  return Object.keys(files).map(slugFromPath).sort();
}
