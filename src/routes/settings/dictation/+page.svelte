<script lang="ts">
  import { settings } from "$lib/settings-store.svelte";
  import { flash } from "$lib/settings-toast.svelte";
  import HotkeyCapture from "$lib/HotkeyCapture.svelte";
  import { prettyHotkey, isMac } from "$lib/hotkey-display";

  async function saveHotkeys() {
    await settings.replace({ ...settings.s });
    flash("Restart wispr-fox to apply new hotkeys");
  }

  const NOISE_OPTIONS = [
    {
      id: "off",
      label: "Off",
      desc: "Audio goes to the transcriber exactly as recorded.",
    },
    {
      id: "on",
      label: "On",
      desc: "Filters low-frequency fan rumble. Zero effect on speech — safe to leave on.",
    },
    {
      id: "aggressive",
      label: "Aggressive",
      desc: "Adds an AI noise gate (RNNoise) that strips fan whir across the whole spectrum. Best for loud fans; turn back if transcripts get worse.",
    },
  ] as const;
</script>

<section>
  <h2>Dictation</h2>
  <p class="lede">Bind the keys that start and stop dictation.</p>
  {#if isMac()}
    <p class="lede tight" title="Bare F8/F9 can be swallowed by media-key behavior unless macOS is set for standard function keys.">
      <strong>macOS defaults to Option+Space and Option+Enter.</strong>
    </p>
  {/if}
  <p class="lede tight">Main is push-to-talk; sticky toggles on the next press.</p>

  <div class="hotkey-block">
    <div class="hotkey-head">
      <div>
        <div class="hk-label">Transcribe <span class="hk-tag">{isMac() ? "Option+Space" : "F8"} default</span></div>
        <div class="hk-desc">Voice to text. The sidebar Clean toggle decides whether this also runs LLM cleanup.</div>
      </div>
    </div>
    <div class="hk-pair">
      <div class="hk-pair-col">
        <div class="hk-pair-label">Main</div>
        <HotkeyCapture label="" bind:value={settings.s.light_hotkey} />
      </div>
      <div class="hk-pair-col">
        <div class="hk-pair-label">Sticky</div>
        <HotkeyCapture label="" bind:value={settings.s.light_sticky_hotkey} />
      </div>
    </div>
    <label class="check-row small">
      <input
        type="checkbox"
        checked={settings.s.sticky_light}
        onchange={(e) => settings.set("sticky_light", (e.currentTarget as HTMLInputElement).checked)}
      />
      <span>Make the main Transcribe hotkey sticky by default</span>
    </label>
  </div>

  <div class="hotkey-block">
    <div class="hotkey-head">
      <div>
        <div class="hk-label">Transcribe + force-clean <span class="hk-tag">{isMac() ? "Shift+Option+Space" : "Shift+F8"} default</span></div>
        <div class="hk-desc">Runs Transcribe with cleanup on for this one dictation without changing your saved preference.</div>
      </div>
    </div>
    <div class="hk-pair">
      <div class="hk-pair-col">
        <div class="hk-pair-label">Main</div>
        <HotkeyCapture label="" bind:value={settings.s.force_clean_hotkey} />
      </div>
      <div class="hk-pair-col">
        <div class="hk-pair-label">Sticky</div>
        <HotkeyCapture label="" bind:value={settings.s.force_clean_sticky_hotkey} />
      </div>
    </div>
  </div>

  <div class="hotkey-block">
    <div class="hotkey-head">
      <div>
        <div class="hk-label">Draft <span class="hk-tag">{isMac() ? "Option+Enter" : "F9"} default</span></div>
        <div class="hk-desc">Turns a spoken brief into polished output for email, chat, docs, or social posts.</div>
      </div>
    </div>
    <div class="hk-pair">
      <div class="hk-pair-col">
        <div class="hk-pair-label">Main</div>
        <HotkeyCapture label="" bind:value={settings.s.drafting_hotkey} />
      </div>
      <div class="hk-pair-col">
        <div class="hk-pair-label">Sticky</div>
        <HotkeyCapture label="" bind:value={settings.s.drafting_sticky_hotkey} />
      </div>
    </div>
    <label class="check-row small">
      <input
        type="checkbox"
        checked={settings.s.sticky_drafting}
        onchange={(e) => settings.set("sticky_drafting", (e.currentTarget as HTMLInputElement).checked)}
      />
      <span>Make the main Draft hotkey sticky by default</span>
    </label>
  </div>

  <details class="hotkey-block-collapsed">
    <summary>
      <span class="hk-label">Advanced cleanup</span>
      <span class="hk-desc-inline">Optional legacy cleanup-only binding. Most users can leave this unbound.</span>
    </summary>
    <div class="hk-pair">
      <div class="hk-pair-col">
        <div class="hk-pair-label">Main</div>
        <HotkeyCapture label="" bind:value={settings.s.advanced_hotkey} />
      </div>
      <div class="hk-pair-col">
        <div class="hk-pair-label">Sticky</div>
        <HotkeyCapture label="" bind:value={settings.s.advanced_sticky_hotkey} />
      </div>
    </div>
    <label class="check-row small">
      <input
        type="checkbox"
        checked={settings.s.sticky_advanced}
        onchange={(e) => settings.set("sticky_advanced", (e.currentTarget as HTMLInputElement).checked)}
      />
      <span>Make Advanced sticky by default</span>
    </label>
  </details>

  <button class="btn-primary" onclick={saveHotkeys}>Save hotkeys</button>
  <p class="hint">Press <strong>Esc</strong> during a recording to stop without sending.</p>

  <h3>Delivery</h3>
  <p class="lede">What wispr-fox does after transcription or cleanup finishes.</p>

  <div class="behavior-block">
    <label class="check-row" title="This replaces whatever you previously copied.">
      <input
        type="checkbox"
        checked={settings.s.keep_in_clipboard}
        onchange={(e) => settings.set("keep_in_clipboard", (e.currentTarget as HTMLInputElement).checked)}
      />
      <span><strong>Keep result on clipboard</strong> as a Ctrl+V backup.</span>
    </label>
  </div>

  <div class="behavior-block">
    <label class="check-row" title="When off and you've moved away, wispr-fox copies the result to the clipboard instead of pasting into the wrong app.">
      <input
        type="checkbox"
        checked={settings.s.pull_back_on_navigation}
        onchange={(e) => settings.set("pull_back_on_navigation", (e.currentTarget as HTMLInputElement).checked)}
      />
      <span><strong>Pull focus back to the original app</strong> when the result is ready.</span>
    </label>
  </div>

  <div class="behavior-block">
    <label class="check-row" title="Affects Draft only — a coarse bucket like email, chat, doc, or social. Transcribe preserves your words.">
      <input
        type="checkbox"
        checked={settings.s.adapt_to_app}
        onchange={(e) => settings.set("adapt_to_app", (e.currentTarget as HTMLInputElement).checked)}
      />
      <span><strong>Adapt Draft tone to the active app</strong>.</span>
    </label>
  </div>

  <h3>Noise reduction</h3>
  <p class="lede">
    Cleans laptop-fan hum and whir out of the audio sent for transcription.
    Runs locally in a few milliseconds after you release the hotkey — the
    floater shows a quick "clearing noise" beat while it works. The saved
    recording in History is always the untouched original.
  </p>

  <div class="radio-grid">
    {#each NOISE_OPTIONS as opt (opt.id)}
      <button
        class="radio-card"
        class:active={settings.s.noise_reduction === opt.id}
        onclick={() => settings.set("noise_reduction", opt.id)}
      >
        <div class="radio-card-head">
          <span class="radio-dot">{settings.s.noise_reduction === opt.id ? "●" : "○"}</span>
          <span class="radio-label">{opt.label}</span>
        </div>
        <div class="radio-desc">{opt.desc}</div>
      </button>
    {/each}
  </div>
</section>
