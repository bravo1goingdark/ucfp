<script lang="ts">
  import { tweaks, setTweak } from '$lib/stores/tweaks.svelte';
  import { onMount } from 'svelte';

  let open = $state(false);

  // Apply tweaks to <html> as effects so SSR + hydration agree.
  $effect(() => {
    if (typeof document === 'undefined') return;
    document.documentElement.setAttribute('data-theme', tweaks.theme);
  });
  $effect(() => {
    if (typeof document === 'undefined') return;
    document.documentElement.setAttribute('data-density', tweaks.density);
  });
  $effect(() => {
    if (typeof document === 'undefined') return;
    document.documentElement.style.setProperty(
      '--accent',
      `oklch(0.62 0.08 ${tweaks.accentHue})`
    );
    document.documentElement.style.setProperty(
      '--accent-ink',
      `oklch(0.32 0.06 ${tweaks.accentHue})`
    );
  });

  onMount(() => {
    // Already applied via $effect; nothing to do, but keeps the file
    // ready for any keyboard-shortcut wiring later.
  });
</script>

{#if open}
  <aside class="tweaks-panel" aria-label="Tweaks panel">
    <header>
      <span>TWEAKS</span>
      <button onclick={() => (open = false)} aria-label="Close">×</button>
    </header>

    <div class="tweaks-section">
      <div class="title">Theme</div>
      <div class="label">Surface</div>
      <div class="tw-radio">
        <button class:on={tweaks.theme === 'paper'} onclick={() => setTweak('theme', 'paper')}>Paper</button>
        <button class:on={tweaks.theme === 'snow'} onclick={() => setTweak('theme', 'snow')}>Snow</button>
        <button class:on={tweaks.theme === 'ink'} onclick={() => setTweak('theme', 'ink')}>Ink</button>
      </div>
    </div>

    <div class="tweaks-section">
      <div class="title">Accent</div>
      <div class="label">Hue · oklch</div>
      <div class="tw-slider">
        <input
          type="range"
          min="0"
          max="360"
          step="1"
          value={tweaks.accentHue}
          oninput={(e) => setTweak('accentHue', Number((e.target as HTMLInputElement).value))}
          aria-label="Accent hue"
        />
        <span class="val">{tweaks.accentHue}°</span>
      </div>
    </div>

    <div class="tweaks-section">
      <div class="title">Layout</div>
      <div class="label">Density</div>
      <div class="tw-radio">
        <button class:on={tweaks.density === 'cozy'} onclick={() => setTweak('density', 'cozy')}>Cozy</button>
        <button class:on={tweaks.density === 'default'} onclick={() => setTweak('density', 'default')}>Default</button>
        <button class:on={tweaks.density === 'airy'} onclick={() => setTweak('density', 'airy')}>Airy</button>
      </div>
    </div>
  </aside>
{/if}

<button
  class="tweaks-fab"
  onclick={() => (open = !open)}
  aria-label="Open tweaks panel"
  aria-expanded={open}
>
  ◐ TWEAKS
</button>
