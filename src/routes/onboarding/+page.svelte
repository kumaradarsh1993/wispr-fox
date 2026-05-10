<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { api } from "$lib/api";
  import { settings } from "$lib/settings-store.svelte";

  let step = $state<1 | 2 | 3>(1);
  let groqKey = $state("");
  let useSameKeyForLlm = $state(true);
  let groqLlmKey = $state("");
  let saving = $state(false);
  let error = $state<string | null>(null);

  onMount(async () => {
    await settings.init();
    // If a key already exists, skip onboarding.
    const s = await api.checkSecrets();
    if (s.stt) goto("/");
  });

  async function saveKeys() {
    error = null;
    if (!groqKey.trim()) {
      error = "Paste your Groq API key first.";
      return;
    }
    saving = true;
    try {
      console.log("[wispr-fox] saving STT key…");
      await api.saveSecret("groq_stt", groqKey.trim());
      console.log("[wispr-fox] saving LLM key…");
      await api.saveSecret(
        "groq_llm",
        useSameKeyForLlm ? groqKey.trim() : groqLlmKey.trim() || groqKey.trim(),
      );
      // Verify the write actually persisted before advancing.
      const verify = await api.checkSecrets();
      console.log("[wispr-fox] post-save checkSecrets:", verify);
      if (!verify.stt) {
        error =
          "Saved without error, but the key isn't readable back.\n\nThis usually means Windows Credential Manager has a permission issue:\n• Make sure you're NOT running wispr-fox as Administrator.\n• Open Start → search \"Credential Manager\" → check it opens normally.\n• Try restarting the Credential Manager service (Win+R → services.msc → find \"Credential Manager\" → Restart).\n• If none of that works, try logging out and back into Windows.";
        return;
      }
      step = 3;
    } catch (e) {
      console.error("[wispr-fox] saveKeys failed", e);
      const raw = String(e);
      if (raw.toLowerCase().includes("credential") || raw.toLowerCase().includes("keyring")) {
        error =
          `Windows Credential Manager error: ${raw}\n\nTroubleshooting:\n• Make sure you're NOT running wispr-fox as Administrator — run it as your normal user.\n• Open Start → search "Credential Manager" → confirm it opens.\n• If it still fails, try restarting Windows Credential Manager: open Services (Win+R → services.msc) → find "Credential Manager" → Restart.`;
      } else {
        error = `Failed to save key: ${raw}`;
      }
    } finally {
      saving = false;
    }
  }

  function display(combo: string): string {
    return combo
      .replace(/CommandOrControl/g, "Ctrl")
      .replace(/Super/g, "Win")
      .replace(/\+/g, " + ");
  }
</script>

<main class="flex flex-col min-h-screen p-6 gap-4 max-w-lg mx-auto">
  <header class="flex items-center justify-between">
    <h1 class="text-lg font-semibold">Welcome to wispr-fox</h1>
    <span class="text-xs text-gray-500">Step {step} / 3</span>
  </header>

  {#if step === 1}
    <section class="space-y-3 text-sm text-gray-700">
      <p>This is a hotkey-based dictation app. Hold your hotkey anywhere on Windows, speak, release — your words appear in whatever text field is focused.</p>
      <p>You'll need a free <strong>Groq API key</strong> for transcription. Free tier (May 2026) covers ~2 hours of dictation per day with Whisper-large-v3-turbo.</p>
      <p>Get one at <a class="text-blue-600 underline" href="https://console.groq.com/keys" target="_blank" rel="noopener">console.groq.com/keys</a> — it's free and takes about a minute.</p>
      <button class="mt-2 px-4 py-2 rounded bg-blue-600 text-white hover:bg-blue-700" onclick={() => (step = 2)}>
        Got my key →
      </button>
    </section>
  {:else if step === 2}
    <section class="space-y-3 text-sm">
      <label class="block">
        <span class="font-medium text-gray-700">Groq API key</span>
        <input
          type="password"
          class="mt-1 w-full px-2 py-1.5 rounded border font-mono text-xs"
          placeholder="gsk_..."
          bind:value={groqKey}
          autocomplete="off"
        />
      </label>

      <label class="flex items-center gap-2 text-gray-700">
        <input type="checkbox" bind:checked={useSameKeyForLlm} />
        Use the same key for the Clippy LLM cleanup layer
      </label>

      {#if !useSameKeyForLlm}
        <label class="block">
          <span class="font-medium text-gray-700">Groq key for Clippy (LLM)</span>
          <input
            type="password"
            class="mt-1 w-full px-2 py-1.5 rounded border font-mono text-xs"
            placeholder="gsk_..."
            bind:value={groqLlmKey}
          />
        </label>
      {/if}

      {#if error}
        <div class="rounded border border-red-300 bg-red-50 p-3 text-red-700 text-xs whitespace-pre-line">
          {error}
        </div>
      {/if}

      <div class="flex gap-2">
        <button class="px-3 py-1.5 rounded border" onclick={() => (step = 1)}>← Back</button>
        <button
          class="px-4 py-1.5 rounded bg-blue-600 text-white hover:bg-blue-700 disabled:opacity-50"
          onclick={saveKeys}
          disabled={saving}
        >
          {saving ? "Saving…" : "Save & continue"}
        </button>
      </div>
      <p class="text-xs text-gray-500">Your keys are stored in Windows Credential Manager — never written to disk in plaintext.</p>
    </section>
  {:else}
    <section class="space-y-3 text-sm">
      <p class="text-green-700">✓ Keys saved.</p>
      <div>
        <p class="font-medium text-gray-700">Your hotkeys are:</p>
        <ul class="mt-1 text-xs font-mono text-gray-700 space-y-0.5 pl-2">
          <li><strong>Light</strong> (auto-clean): {display(settings.s.light_hotkey)}</li>
          <li><strong>Advanced</strong> (treats dictation as instructions): {display(settings.s.advanced_hotkey)}</li>
        </ul>
      </div>
      <p class="text-xs text-gray-500">Try it: open Notepad, hold Light, say a sentence, release.</p>
      <button class="px-4 py-2 rounded bg-blue-600 text-white" onclick={() => goto("/")}>
        Done
      </button>
      <a href="/settings" class="ml-2 text-sm text-gray-600 underline">Customize hotkeys first</a>
    </section>
  {/if}
</main>
