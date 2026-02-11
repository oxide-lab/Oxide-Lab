<script lang="ts">
  import { t } from '$lib/i18n';
  import GithubLogo from 'phosphor-svelte/lib/GithubLogo';

  interface Props {
    open: boolean;
    appVersion: string;
    onClose: () => void;
  }

  let { open, appVersion, onClose }: Props = $props();
  let modalElement = $state<HTMLDivElement | null>(null);

  function handleBackdropKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape' || event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      onClose();
    }
  }

  // Focus trap for About modal
  $effect(() => {
    if (!open || !modalElement) return;

    const node = modalElement;
    const focusableElements = Array.from(
      node.querySelectorAll<HTMLElement>(
        'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])',
      ),
    );
    const firstElement = focusableElements[0];
    const lastElement = focusableElements[focusableElements.length - 1];

    const handleKeydown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        event.preventDefault();
        onClose();
        return;
      }

      if (event.key !== 'Tab' || focusableElements.length === 0) return;

      const activeElement = document.activeElement as HTMLElement | null;

      if (event.shiftKey) {
        if (activeElement === firstElement) {
          event.preventDefault();
          lastElement?.focus();
        }
      } else if (activeElement === lastElement) {
        event.preventDefault();
        firstElement?.focus();
      }
    };

    node.addEventListener('keydown', handleKeydown);
    firstElement?.focus() ?? node.focus();

    return () => node.removeEventListener('keydown', handleKeydown);
  });
</script>

{#if open}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    bind:this={modalElement}
    class="about-modal"
    role="dialog"
    aria-modal="true"
    aria-labelledby="about-title"
    tabindex="-1"
    onclick={(event) => {
      if (event.target === event.currentTarget) onClose();
    }}
    onkeydown={handleBackdropKeydown}
  >
    <div class="about-content" role="document">
      <h2 id="about-title">{$t('about.title') || 'About'}</h2>
      <div class="about-info">
        <p>
          <strong>Oxide Lab</strong> — {$t('about.description') || 'Local AI inference application'}
        </p>
        <p>
          <strong>{$t('about.technologies') || 'Technologies'}:</strong> Tauri, Svelte, llama.cpp process-host
        </p>
        <p><strong>{$t('about.version') || 'Version'}:</strong> {appVersion}</p>
      </div>
      <div class="about-actions">
        <button
          class="github-btn"
          onclick={() => window.open('https://github.com/FerrisMind/Oxide-Lab', '_blank')}
          aria-label="GitHub"
        >
          <GithubLogo size={16} />
          GitHub
        </button>
        <button
          class="close-btn"
          onclick={(e) => {
            e.stopPropagation();
            onClose();
          }}
          aria-label={$t('common.buttons.close') || 'Close'}
        >
          {$t('common.buttons.close') || 'Close'}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .about-modal {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 2000;
  }

  .about-content {
    background: var(--popover);
    border: 1px solid var(--border);
    border-radius: 1rem;
    padding: 2rem;
    max-width: 28rem;
    width: 90%;
    box-shadow: var(--shadow-2xl);
  }

  h2 {
    font-size: 1.5rem;
    font-weight: 700;
    margin-bottom: 1.5rem;
    color: var(--foreground);
  }

  .about-info {
    margin-bottom: 2rem;
    line-height: 1.6;
    color: var(--muted-foreground);
  }

  .about-info p {
    margin-bottom: 0.75rem;
  }

  .about-info strong {
    color: var(--foreground);
    font-weight: 600;
  }

  .about-actions {
    display: flex;
    justify-content: flex-end;
    gap: 1rem;
  }

  .github-btn {
    display: inline-flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.6rem 1.25rem;
    background: var(--secondary);
    color: var(--secondary-foreground);
    border-radius: 0.75rem;
    font-weight: 600;
    transition: all 0.2s ease;
  }

  .github-btn:hover {
    background: var(--secondary-hover);
  }

  .close-btn {
    padding: 0.6rem 1.5rem;
    background: var(--primary);
    color: var(--primary-foreground);
    border-radius: 0.75rem;
    font-weight: 600;
    transition: all 0.2s ease;
  }

  .close-btn:hover {
    opacity: 0.9;
  }
</style>
