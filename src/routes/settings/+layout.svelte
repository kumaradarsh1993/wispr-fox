<script lang="ts">
  // Settings layout: compact sub-nav + outlet for category pages.
  import { onMount } from "svelte";
  import { page } from "$app/state";
  import { settings } from "$lib/settings-store.svelte";
  import { toast } from "$lib/settings-toast.svelte";
  import "./settings.css";

  let { children } = $props();

  type NavItem = { href: string; label: string; icon: string };
  const NAV: NavItem[] = [
    { href: "/settings/providers", label: "Providers", icon: "K" },
    { href: "/settings/modes", label: "Modes", icon: "M" },
    { href: "/settings/dictation", label: "Dictation", icon: "D" },
    { href: "/settings/appearance", label: "Avatar", icon: "A" },
    { href: "/settings/general", label: "General", icon: "G" },
    { href: "/settings/security", label: "Security", icon: "!" },
  ];

  function isActive(href: string): boolean {
    const path = page.url?.pathname ?? "/";
    return path === href || path.startsWith(href + "/");
  }

  onMount(async () => {
    await settings.init();
  });
</script>

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
          <span class="nav-icon">{item.icon}</span>
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
