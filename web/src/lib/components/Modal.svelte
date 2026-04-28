<script lang="ts">
  import { onMount, type Snippet } from 'svelte';

  interface Props {
    open: boolean;
    title?: string;
    onclose?: () => void;
    children?: Snippet;
  }

  let { open = $bindable(), title, onclose, children }: Props = $props();

  let cardEl = $state<HTMLDivElement | null>(null);
  let lastFocus: Element | null = null;

  function focusables(root: HTMLElement): HTMLElement[] {
    const sel =
      'a[href], button:not([disabled]), textarea:not([disabled]), input:not([disabled]), select:not([disabled]), [tabindex]:not([tabindex="-1"])';
    return Array.from(root.querySelectorAll<HTMLElement>(sel));
  }

  function close() {
    open = false;
    onclose?.();
  }

  function onKeydown(e: KeyboardEvent) {
    if (!open) return;
    if (e.key === 'Escape') {
      e.preventDefault();
      close();
      return;
    }
    if (e.key === 'Tab' && cardEl) {
      const els = focusables(cardEl);
      if (els.length === 0) {
        e.preventDefault();
        cardEl.focus();
        return;
      }
      const first = els[0];
      const last = els[els.length - 1];
      const active = document.activeElement as HTMLElement | null;
      if (e.shiftKey && active === first) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && active === last) {
        e.preventDefault();
        first.focus();
      }
    }
  }

  $effect(() => {
    if (typeof document === 'undefined') return;
    if (open) {
      lastFocus = document.activeElement;
      // Focus the first interactive element next tick.
      queueMicrotask(() => {
        if (!cardEl) return;
        const els = focusables(cardEl);
        (els[0] ?? cardEl).focus();
      });
    } else if (lastFocus instanceof HTMLElement) {
      lastFocus.focus();
      lastFocus = null;
    }
  });

  onMount(() => {
    document.addEventListener('keydown', onKeydown);
    return () => document.removeEventListener('keydown', onKeydown);
  });
</script>

{#if open}
  <div
    class="modal-backdrop"
    role="presentation"
    onclick={(e) => {
      if (e.target === e.currentTarget) close();
    }}
  >
    <div
      class="modal-card"
      role="dialog"
      aria-modal="true"
      aria-label={title ?? 'Dialog'}
      tabindex="-1"
      bind:this={cardEl}
    >
      {#if title}
        <header class="modal-head">
          <h2>{title}</h2>
          <button type="button" class="close" aria-label="Close dialog" onclick={close}>×</button>
        </header>
      {/if}
      <div class="modal-body">
        {#if children}{@render children()}{/if}
      </div>
    </div>
  </div>
{/if}
