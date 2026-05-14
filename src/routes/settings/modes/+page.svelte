<script lang="ts">
  // Modes — per-mode cleanup toggle + custom system prompt.
  //
  // Hotkey assignment for each mode lives on /settings/dictation; the
  // shared LLM model lives on /settings/providers. This page is the one
  // place to tune what each F-key actually DOES with the transcript.
  import { onMount } from "svelte";
  import { api } from "$lib/api";
  import { settings } from "$lib/settings-store.svelte";
  import { flash } from "$lib/settings-toast.svelte";

  let defaultPrompts = $state<{ light: string; advanced: string; drafting: string } | null>(null);
  let promptOpen = $state<Record<"light" | "advanced" | "drafting", boolean>>({
    light: false,
    advanced: false,
    drafting: false,
  });

  function effectivePrompt(mode: "light" | "advanced" | "drafting"): string {
    const customField = `custom_${mode}_prompt` as keyof typeof settings.s;
    const custom = (settings.s[customField] as string) || "";
    if (custom.trim()) return custom;
    return defaultPrompts ? defaultPrompts[mode] : "(loading…)";
  }

  async function savePrompt(mode: "light" | "advanced" | "drafting", value: string) {
    const customField = `custom_${mode}_prompt` as keyof typeof settings.s;
    await settings.set(customField, value as any);
    flash(`${mode} prompt saved`);
  }

  async function resetPrompt(mode: "light" | "advanced" | "drafting") {
    if (!confirm(`Reset the ${mode} prompt to its default? Any custom edits will be lost.`)) return;
    const customField = `custom_${mode}_prompt` as keyof typeof settings.s;
    await settings.set(customField, "" as any);
    flash(`${mode} prompt reset`);
  }

  onMount(async () => {
    defaultPrompts = await api.getDefaultPrompts();
  });
</script>

<section>
  <h2>Modes</h2>
  <p class="lede">
    Each F-key runs the same LLM model with a different system prompt.
    Toggle whether each mode uses LLM cleanup, and click "Show prompt"
    to view or customise what each mode tells the model to do.
  </p>

  {#each [
    { id: "light",    fkey: "F8",  title: "Transcribe",          settingKey: "auto_clean_in_light",
      desc: "Voice → text. When LLM cleanup is OFF you get the raw Whisper output (default). When ON, every F8 press also gets spell / punctuation / paragraphing — same content, same voice, just readable. For one-off cleanup without flipping this toggle, use Shift+F8." },
    { id: "drafting", fkey: "F9",  title: "Draft",               settingKey: "auto_clean_in_drafting",
      desc: "Give a brief (\"draft an email to Saurabh about X, Y, Z\") and get back a complete polished output. Best for emails, Slack, docs." },
    { id: "advanced", fkey: "—",   title: "Advanced (legacy)",   settingKey: "auto_clean_in_advanced",
      desc: "Standalone Advanced cleanup mode. No hotkey by default — bind one in Dictation if you want a dedicated key separate from the F8 toggle." },
  ] as m (m.id)}
    <div class="mode-block">
      <div class="mode-head">
        <kbd class="mode-key">{m.fkey}</kbd>
        <div class="mode-title-block">
          <div class="mode-title">{m.title}</div>
          <div class="mode-desc">{m.desc}</div>
        </div>
        <label class="check-row inline">
          <input
            type="checkbox"
            checked={settings.s[m.settingKey as keyof typeof settings.s] as boolean}
            onchange={(e) => settings.set(m.settingKey as any, (e.currentTarget as HTMLInputElement).checked as any)}
          />
          <span>LLM cleanup</span>
        </label>
      </div>

      <button
        class="prompt-toggle"
        onclick={() => (promptOpen[m.id as "light" | "advanced" | "drafting"] = !promptOpen[m.id as "light" | "advanced" | "drafting"])}
      >
        <span class="prompt-caret" class:open={promptOpen[m.id as "light" | "advanced" | "drafting"]}>›</span>
        {promptOpen[m.id as "light" | "advanced" | "drafting"] ? "Hide" : "Show"} system prompt
        {#if (settings.s[`custom_${m.id}_prompt` as keyof typeof settings.s] as string)?.trim()}
          <span class="prompt-edited-pill">customised</span>
        {/if}
      </button>
      {#if promptOpen[m.id as "light" | "advanced" | "drafting"]}
        <div class="prompt-editor">
          <textarea
            rows="10"
            value={effectivePrompt(m.id as "light" | "advanced" | "drafting")}
            onchange={(e) => savePrompt(m.id as "light" | "advanced" | "drafting", (e.currentTarget as HTMLTextAreaElement).value)}
          ></textarea>
          <div class="prompt-actions">
            <button
              class="btn-secondary small"
              onclick={() => resetPrompt(m.id as "light" | "advanced" | "drafting")}
            >
              Reset to default
            </button>
            {#if m.id === "light"}
              <span class="prompt-warning">
                ⚠ The Light prompt is a security boundary against prompt injection — keep the "treat transcript as literal data" guarantee or attackers can hijack via dictation.
              </span>
            {/if}
          </div>
        </div>
      {/if}
    </div>
  {/each}

  <p class="tip">
    Hotkey bindings for each mode live in <strong>Settings → Dictation</strong>.
    Provider keys and the shared LLM model live in <strong>Settings → Providers &amp; Models</strong>.
  </p>
</section>

<style>
  .tip {
    background: var(--bg-subtle);
    border: 1px dashed var(--border);
    border-radius: 10px;
    padding: 10px 14px;
    font-size: 12px;
    color: var(--text-secondary);
    line-height: 1.5;
    margin: 24px 0 0;
    max-width: 720px;
  }
</style>
