<script lang="ts">
  import { onMount } from "svelte";
  import { api } from "$lib/api";
  import { settings } from "$lib/settings-store.svelte";
  import HotkeyCapture from "$lib/HotkeyCapture.svelte";

  type Tab = "keys" | "hotkeys" | "clippy" | "retention";
  let tab = $state<Tab>("keys");

  let secretCheck = $state({ stt: false, llm: false });
  let groqStt = $state("");
  let groqLlm = $state("");
  let saving = $state(false);
  let savedToast = $state<string | null>(null);

  onMount(async () => {
    await settings.init();
    secretCheck = await api.checkSecrets();
  });

  async function saveStt() {
    if (!groqStt.trim()) return;
    saving = true;
    try {
      await api.saveSecret("groq_stt", groqStt.trim());
      groqStt = "";
      secretCheck = await api.checkSecrets();
      flash("STT key saved");
    } finally {
      saving = false;
    }
  }

  async function saveLlm() {
    if (!groqLlm.trim()) return;
    saving = true;
    try {
      await api.saveSecret("groq_llm", groqLlm.trim());
      groqLlm = "";
      secretCheck = await api.checkSecrets();
      flash("LLM key saved");
    } finally {
      saving = false;
    }
  }

  async function deleteKey(name: "groq_stt" | "groq_llm") {
    if (!confirm(`Delete ${name} from Credential Manager?`)) return;
    await api.deleteSecret(name);
    secretCheck = await api.checkSecrets();
    flash("Deleted");
  }

  function flash(msg: string) {
    savedToast = msg;
    setTimeout(() => (savedToast = null), 1500);
  }

  // Hotkey changes need a restart to re-register globally; flag this on edit.
  let hotkeyDirty = $state(false);
  async function saveHotkeys() {
    await settings.replace({ ...settings.s });
    hotkeyDirty = false;
    flash("Saved — restart wispr-fox to apply new hotkeys");
  }

  async function setNumber<K extends keyof typeof settings.s>(key: K, value: number) {
    await settings.set(key, value as (typeof settings.s)[K]);
  }
</script>

<main class="flex flex-col min-h-screen p-5 gap-4 max-w-2xl mx-auto">
  <header class="flex items-center justify-between">
    <a href="/" class="text-sm text-blue-600 hover:underline">← Back</a>
    <h1 class="text-base font-semibold">Settings</h1>
    {#if savedToast}
      <span class="text-xs text-green-700">✓ {savedToast}</span>
    {:else}
      <span></span>
    {/if}
  </header>

  <nav class="flex gap-2 text-sm border-b">
    {#each [["keys", "API keys"], ["hotkeys", "Hotkeys"], ["clippy", "Clippy"], ["retention", "Retention"]] as [id, label] (id)}
      <button
        class="px-3 py-1.5 -mb-px border-b-2 {tab === id ? 'border-blue-600 text-blue-700' : 'border-transparent text-gray-600 hover:text-gray-900'}"
        onclick={() => (tab = id as Tab)}
      >
        {label}
      </button>
    {/each}
  </nav>

  {#if tab === "keys"}
    <section class="space-y-4 text-sm">
      <div class="space-y-1">
        <div class="font-medium text-gray-700">Groq STT key (Whisper)</div>
        <div class="flex gap-2 items-center">
          <input
            type="password"
            placeholder={secretCheck.stt ? "•••••••• (saved)" : "gsk_..."}
            class="flex-1 px-2 py-1 rounded border text-xs font-mono"
            bind:value={groqStt}
          />
          <button class="px-3 py-1 rounded border text-xs" onclick={saveStt} disabled={saving}>
            Save
          </button>
          {#if secretCheck.stt}
            <button class="px-2 py-1 text-xs text-red-600 hover:underline" onclick={() => deleteKey("groq_stt")}>
              Delete
            </button>
          {/if}
        </div>
      </div>

      <div class="space-y-1">
        <div class="font-medium text-gray-700">Groq LLM key (Clippy)</div>
        <div class="flex gap-2 items-center">
          <input
            type="password"
            placeholder={secretCheck.llm ? "•••••••• (saved)" : "gsk_... (or reuse STT key)"}
            class="flex-1 px-2 py-1 rounded border text-xs font-mono"
            bind:value={groqLlm}
          />
          <button class="px-3 py-1 rounded border text-xs" onclick={saveLlm} disabled={saving}>
            Save
          </button>
          {#if secretCheck.llm}
            <button class="px-2 py-1 text-xs text-red-600 hover:underline" onclick={() => deleteKey("groq_llm")}>
              Delete
            </button>
          {/if}
        </div>
        <p class="text-xs text-gray-500">If unset, Clippy uses the STT key.</p>
      </div>

      <p class="text-xs text-gray-500">
        Stored in Windows Credential Manager. Service: <code class="bg-gray-100 px-1">wispr-fox</code>.
      </p>
    </section>
  {:else if tab === "hotkeys"}
    <section class="space-y-4 text-sm">
      <HotkeyCapture
        label="Light flow (default — auto-clean transcription)"
        bind:value={settings.s.light_hotkey}
      />
      <HotkeyCapture
        label="Advanced flow (treats dictation as instructions)"
        bind:value={settings.s.advanced_hotkey}
      />
      <button class="px-3 py-1.5 rounded bg-blue-600 text-white text-sm" onclick={saveHotkeys}>
        Save hotkeys
      </button>
      <p class="text-xs text-gray-500">
        Hotkey changes take effect after restarting wispr-fox.
      </p>
    </section>
  {:else if tab === "clippy"}
    <section class="space-y-3 text-sm">
      <label class="flex items-center gap-2">
        <input
          type="checkbox"
          checked={settings.s.auto_clean_in_light}
          onchange={(e) => settings.set("auto_clean_in_light", (e.currentTarget as HTMLInputElement).checked)}
        />
        <span>Auto-clean transcripts in Light flow</span>
      </label>
      <label class="block">
        <span class="font-medium text-gray-700">Light model</span>
        <input
          type="text"
          class="mt-1 w-full px-2 py-1 rounded border text-xs font-mono"
          value={settings.s.clippy_light_model}
          onchange={(e) => settings.set("clippy_light_model", (e.currentTarget as HTMLInputElement).value)}
        />
      </label>
      <label class="block">
        <span class="font-medium text-gray-700">Advanced model</span>
        <input
          type="text"
          class="mt-1 w-full px-2 py-1 rounded border text-xs font-mono"
          value={settings.s.clippy_advanced_model}
          onchange={(e) => settings.set("clippy_advanced_model", (e.currentTarget as HTMLInputElement).value)}
        />
      </label>
      <label class="block">
        <span class="font-medium text-gray-700">STT model</span>
        <input
          type="text"
          class="mt-1 w-full px-2 py-1 rounded border text-xs font-mono"
          value={settings.s.stt_model}
          onchange={(e) => settings.set("stt_model", (e.currentTarget as HTMLInputElement).value)}
        />
      </label>
      <label class="block">
        <span class="font-medium text-gray-700">Language hint</span>
        <input
          type="text"
          class="mt-1 w-full px-2 py-1 rounded border text-xs font-mono"
          placeholder="auto (leave blank)"
          value={settings.s.language_hint ?? ""}
          onchange={(e) => {
            const v = (e.currentTarget as HTMLInputElement).value.trim();
            settings.set("language_hint", v.length ? v : null);
          }}
        />
        <span class="text-xs text-gray-500">Blank = auto-detect. Use ISO codes like <code>en</code> or <code>hi</code>.</span>
      </label>
    </section>
  {:else if tab === "retention"}
    <section class="space-y-3 text-sm">
      <label class="block">
        <span class="font-medium text-gray-700">
          Retention: {settings.s.retention_days} day{settings.s.retention_days === 1 ? "" : "s"}
        </span>
        <input
          type="range"
          min="1"
          max="30"
          value={settings.s.retention_days}
          oninput={(e) => setNumber("retention_days", Number((e.currentTarget as HTMLInputElement).value))}
          class="mt-1 w-full"
        />
        <span class="text-xs text-gray-500">Recordings older than this are auto-deleted hourly.</span>
      </label>
      <label class="block">
        <span class="font-medium text-gray-700">Storage cap (MB): {settings.s.retention_max_mb}</span>
        <input
          type="number"
          min="50"
          step="50"
          value={settings.s.retention_max_mb}
          onchange={(e) => setNumber("retention_max_mb", Number((e.currentTarget as HTMLInputElement).value))}
          class="mt-1 w-32 px-2 py-1 rounded border text-xs"
        />
      </label>
    </section>
  {/if}
</main>
