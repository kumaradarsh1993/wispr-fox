<script lang="ts">
  // Settings layout — left sub-nav + outlet for sub-route content.
  // Each setting category gets its own URL (deep-linkable from
  // onboarding, tray menu, etc.). Shared toast lives here; sub-pages
  // import flash() from $lib/settings-toast to trigger it.
  import { onMount } from "svelte";
  import { page } from "$app/state";
  import { settings } from "$lib/settings-store.svelte";
  import { toast } from "$lib/settings-toast.svelte";
  import "./settings.css";

  let { children } = $props();

  type NavItem = { href: string; label: string; icon: string };
  const NAV: NavItem[] = [
    { href: "/settings/providers",  label: "Providers & Models", icon: "🔑" },
    { href: "/settings/dictation",  label: "Dictation",          icon: "⌨" },
    { href: "/settings/general",    label: "General",            icon: "⚙" },
    { href: "/settings/appearance", label: "Appearance",         icon: "🎨" },
    { href: "/settings/data",       label: "Data",               icon: "🗃" },
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
      <div class="toast">✓ {toast.msg}</div>
    {/if}

    {@render children?.()}
  </main>
</div>
