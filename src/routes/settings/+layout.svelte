<script lang="ts">
  // Settings layout: compact sub-nav + outlet for category pages.
  import { onMount } from "svelte";
  import { page } from "$app/state";
  import { settings } from "$lib/settings-store.svelte";
  import { toast } from "$lib/settings-toast.svelte";
  import "./settings.css";

  let { children } = $props();

  // Icons are inline stroke SVGs (see the navIcon snippet) rather than letter
  // glyphs — the old K/M/D/A/G/! read like keyboard shortcuts. Same 15px /
  // currentColor / 1.6-stroke style as the main sidebar nav.
  type IconName = "providers" | "modes" | "dictation" | "appearance" | "general" | "security" | "account";
  type NavItem = { href: string; label: string; icon: IconName };
  // Canonical section order, shared across desktop / web / android:
  // Transcription → Cleanup & modes → API keys → Appearance → Delivery/
  // behaviour → Storage/Data → Account (with Purge at its bottom) → About.
  // Desktop mapping: Providers folds Transcription + API keys; Modes = Cleanup
  // & modes; Dictation = delivery/behaviour; Security (key-storage) sits with
  // the data section; Account is second-to-last; General (data retention +
  // updates/version) is the closest thing to "About" and stays last.
  const NAV: NavItem[] = [
    { href: "/settings/providers", label: "Providers", icon: "providers" },
    { href: "/settings/modes", label: "Modes", icon: "modes" },
    { href: "/settings/appearance", label: "Appearance", icon: "appearance" },
    { href: "/settings/dictation", label: "Dictation", icon: "dictation" },
    { href: "/settings/security", label: "Security", icon: "security" },
    { href: "/settings/account", label: "Account", icon: "account" },
    { href: "/settings/general", label: "General", icon: "general" },
  ];

  function isActive(href: string): boolean {
    const path = page.url?.pathname ?? "/";
    return path === href || path.startsWith(href + "/");
  }

  onMount(async () => {
    await settings.init();
  });
</script>

{#snippet navIcon(icon: IconName)}
  {#if icon === "providers"}
    <!-- plug -->
    <svg viewBox="0 0 16 16" width="15" height="15" aria-hidden="true">
      <path d="M 6 2 V 5 M 10 2 V 5" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" />
      <path d="M 4.5 5 H 11.5 V 7.5 A 3.5 3.5 0 0 1 4.5 7.5 Z" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round" />
      <path d="M 8 11 V 14" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" />
    </svg>
  {:else if icon === "modes"}
    <!-- wand -->
    <svg viewBox="0 0 16 16" width="15" height="15" aria-hidden="true">
      <path d="M 3 13 L 11 5" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" />
      <path d="M 11 2.5 L 11.6 4 L 13 4.6 L 11.6 5.2 L 11 6.7 L 10.4 5.2 L 9 4.6 L 10.4 4 Z" fill="currentColor" stroke="none" />
    </svg>
  {:else if icon === "dictation"}
    <!-- keyboard key -->
    <svg viewBox="0 0 16 16" width="15" height="15" aria-hidden="true">
      <rect x="2.5" y="4" width="11" height="8" rx="1.6" fill="none" stroke="currentColor" stroke-width="1.6" />
      <path d="M 6 8.5 H 10" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" />
    </svg>
  {:else if icon === "appearance"}
    <!-- sparkle -->
    <svg viewBox="0 0 16 16" width="15" height="15" aria-hidden="true">
      <path d="M 8 2 L 9.2 6.8 L 14 8 L 9.2 9.2 L 8 14 L 6.8 9.2 L 2 8 L 6.8 6.8 Z" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round" />
    </svg>
  {:else if icon === "general"}
    <!-- gear -->
    <svg viewBox="0 0 16 16" width="15" height="15" aria-hidden="true">
      <circle cx="8" cy="8" r="2.2" fill="none" stroke="currentColor" stroke-width="1.6" />
      <path d="M 8 1.5 V 3 M 8 13 V 14.5 M 1.5 8 H 3 M 13 8 H 14.5 M 3.4 3.4 L 4.5 4.5 M 11.5 11.5 L 12.6 12.6 M 12.6 3.4 L 11.5 4.5 M 4.5 11.5 L 3.4 12.6" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" />
    </svg>
  {:else if icon === "account"}
    <!-- person -->
    <svg viewBox="0 0 16 16" width="15" height="15" aria-hidden="true">
      <circle cx="8" cy="5.5" r="2.6" fill="none" stroke="currentColor" stroke-width="1.6" />
      <path d="M 3 13.5 C 3 10.8 5.2 9.5 8 9.5 C 10.8 9.5 13 10.8 13 13.5" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" />
    </svg>
  {:else}
    <!-- shield -->
    <svg viewBox="0 0 16 16" width="15" height="15" aria-hidden="true">
      <path d="M 8 2 L 13 4 V 8 C 13 11 10.8 13 8 14 C 5.2 13 3 11 3 8 V 4 Z" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round" />
    </svg>
  {/if}
{/snippet}

<div class="settings">
  <aside class="section-nav">
    <h1>Settings</h1>
    <nav>
      {#each NAV as item (item.href)}
        <a
          class="nav-btn"
          class:active={isActive(item.href)}
          href={item.href}
          data-sveltekit-preload-data="hover"
        >
          <span class="nav-icon">{@render navIcon(item.icon)}</span>
          <span>{item.label}</span>
        </a>
      {/each}
    </nav>
  </aside>

  <main class="section-body">
    {#if toast.msg}
      <div class="toast">Saved: {toast.msg}</div>
    {/if}

    {@render children?.()}
  </main>
</div>
