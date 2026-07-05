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
  import { prettyHotkey } from "$lib/hotkey-display";

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
  <p class="lede">Each hotkey runs the LLM with a different prompt — customise them here.</p>

  {#each [
    { id: "light",    fkey: prettyHotkey(settings.s.light_hotkey),  title: "Transcribe",          settingKey: "auto_clean_in_light",
      desc: `Voice → text. Clean Transcribe adds spell / punctuation / paragraphing — same words, just readable. For one-off cleanup, use ${prettyHotkey(settings.s.force_clean_hotkey)}.` },
    { id: "drafting", fkey: prettyHotkey(settings.s.drafting_hotkey),  title: "Draft",               settingKey: "auto_clean_in_drafting",
      desc: "Give a brief and get back a complete polished output. Best for emails, Slack, docs." },
    { id: "advanced", fkey: "—",   title: "Cleanup only (legacy)",   settingKey: "auto_clean_in_advanced",
      desc: "Standalone cleanup mode. No hotkey by default — bind one in Dictation if you want it." },
  ] as m (m.id)}
    <div class="mode-block">
      <div class="mode-head">
        <kbd class="mode-key">{m.fkey}</kbd>
        <div class="mode-title-block">
          <div class="mode-title">{m.title}</div>
          <div class="mode-desc">{m.desc}</div>
        </div>
        {#if m.id === "light"}
          <label class="check-row inline">
            <input
              type="checkbox"
              checked={settings.s.auto_clean_in_light}
              onchange={(e) => settings.set("auto_clean_in_light", (e.currentTarget as HTMLInputElement).checked as any)}
            />
            <span>Clean Transcribe</span>
          </label>
        {/if}
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
                ⚠ The Transcribe prompt is a security boundary against prompt injection — keep the "treat transcript as literal data" guarantee.
              </span>
            {/if}
          </div>
        </div>
      {/if}
    </div>
  {/each}

  <p class="tip">
    Hotkeys live in <strong>Settings → Dictation</strong>; keys and models in <strong>Settings → Providers</strong>.
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
