import adapter from '@sveltejs/adapter-cloudflare';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

/** @type {import('@sveltejs/kit').Config} */
const config = {
  preprocess: vitePreprocess(),
  kit: {
    adapter: adapter({
      routes: { include: ['/*'], exclude: ['<all>'] }
    }),
    // /api/* is a developer-facing REST API consumed by curl, SDKs, and the
    // playground — not same-origin HTML forms. API key + session auth already
    // protect every sensitive route; CSRF origin-check adds no value here and
    // blocks legitimate cross-origin POST requests with text/plain bodies.
    csrf: { trustedOrigins: ['*'] },
    alias: {
      $lib: 'src/lib',
      $components: 'src/lib/components'
    },
    prerender: {
      // Routes are added incrementally across waves; warn on dangling
      // links (e.g. footer points at /docs before W4 ships) instead of
      // failing the build.
      handleHttpError: 'warn',
      handleMissingId: 'warn',
      handleUnseenRoutes: 'warn'
    }
  }
};

export default config;
