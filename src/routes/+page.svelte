<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { api, onFlowError } from "$lib/api";
  import { settings } from "$lib/settings-store.svelte";
  import StatusBadge from "$lib/StatusBadge.svelte";

  let lastError = $state<string | null>(null);
  let bootChecked = $state(false);

  onMount(() => {
    let unlisten: (() => void) | undefined;
    onFlowError((msg) => (lastError = msg)).then((u) => (unlisten = u));

    (async () => {
      await settings.init();
      // First-run gate: if no STT key in keychain, route to onboarding.
      const secrets = await api.checkSecrets();
      console.log("[wispr-fox] main-shell checkSecrets:", secrets);
      if (!secrets.stt) {
        goto("/onboarding");
      } else {
        bootChecked = true;
      }
    })();

    return () => unlisten?.();
  });

  function display(combo: string): string {
    return combo
      .replace(/CommandOrControl/g, "Ctrl")
      .replace(/Super/g, "Win")
      .replace(/\+/g, " + ");
  }
</script>

<main class="flex flex-col h-screen p-6 gap-5 bg-[#fafafa]">
  <header class="flex items-center justify-between">
    <h1 class="text-base font-semibold tracking-tight">wispr-fox</h1>
    <StatusBadge />
  </header>

  {#if !bootChecked}
    <p class="text-sm text-gray-500">Loading…</p>
  {:else}
    <section class="text-sm space-y-2">
      <p class="text-gray-700">Press your hotkey anywhere on your machine to dictate.</p>
      <ul class="text-xs text-gray-600 font-mono space-y-0.5 pl-2">
        <li><strong>Light:</strong> {display(settings.s.light_hotkey)}</li>
        <li><strong>Advanced:</strong> {display(settings.s.advanced_hotkey)}</li>
      </ul>
    </section>
  {/if}

  {#if lastError}
    <div class="text-xs px-2 py-1.5 rounded bg-red-50 text-red-700 border border-red-100">
      ⚠ {lastError}
      <button class="ml-2 underline" onclick={() => (lastError = null)}>dismiss</button>
    </div>
  {/if}

  <nav class="mt-auto flex gap-2 text-sm">
    <a href="/settings" class="px-3 py-1.5 rounded border hover:bg-gray-50">Settings</a>
    <a href="/history" class="px-3 py-1.5 rounded border hover:bg-gray-50">History</a>
  </nav>
</main>
