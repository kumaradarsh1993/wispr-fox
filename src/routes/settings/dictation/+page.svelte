<script lang="ts">
  // Dictation — cross-cutting hotkeys + behaviour toggles.
  // Per-mode hotkeys live in /settings/providers under Models > Modes;
  // this page is for the force-clean override + advanced legacy binding
  // + post-dictation behaviour (clipboard, pull-back, per-app tone).
  import { settings } from "$lib/settings-store.svelte";
  import { flash } from "$lib/settings-toast.svelte";
  import HotkeyCapture from "$lib/HotkeyCapture.svelte";

  async function saveHotkeys() {
    await settings.replace({ ...settings.s });
    flash("Saved — restart wispr-fox to apply new hotkeys");
  }
</script>

<section>
  <h2>Dictation</h2>
  <p class="lede">
    Per-mode hotkeys (F8 Transcribe, F9 Draft, Advanced cleanup) live
    inside <strong>Providers & Models → Modes</strong> alongside their cleanup
    toggle and system prompt — that's the recommended place to edit them.
    This page is kept for cross-cutting bindings (the force-clean override)
    and post-dictation behaviour.
  </p>
  <p class="lede" style="margin-top: -10px;">
    Every mode has TWO hotkeys: a <strong>main</strong> (push-to-talk by default —
    hold to record) and a <strong>sticky-invoke</strong> (press once to start,
    press again to stop). Optionally check <strong>"Default to sticky"</strong>
    to make the main hotkey also behave as a toggle.
  </p>

  <div class="hotkey-block">
    <div class="hotkey-head">
      <div>
        <div class="hk-label">Transcribe <span class="hk-tag">F8 default · most used</span></div>
        <div class="hk-desc">Voice → text. LLM cleanup toggle is in Providers & Models.</div>
      </div>
    </div>
    <div class="hk-pair">
      <div class="hk-pair-col">
        <div class="hk-pair-label">Main (push-to-talk)</div>
        <HotkeyCapture label="" bind:value={settings.s.light_hotkey} />
      </div>
      <div class="hk-pair-col">
        <div class="hk-pair-label">Sticky-invoke (toggle)</div>
        <HotkeyCapture label="" bind:value={settings.s.light_sticky_hotkey} />
      </div>
    </div>
    <label class="check-row small">
      <input
        type="checkbox"
        checked={settings.s.sticky_light}
        onchange={(e) => settings.set("sticky_light", (e.currentTarget as HTMLInputElement).checked)}
      />
      <span>Default to sticky — make the MAIN hotkey behave as toggle too</span>
    </label>
  </div>

  <div class="hotkey-block">
    <div class="hotkey-head">
      <div>
        <div class="hk-label">Transcribe + force-clean <span class="hk-tag">Shift+F8 default</span></div>
        <div class="hk-desc">Same as Transcribe, but forces LLM cleanup ON for this one press — useful when you normally want raw and occasionally want cleaned. Doesn't change the persistent toggle.</div>
      </div>
    </div>
    <div class="hk-pair">
      <div class="hk-pair-col">
        <div class="hk-pair-label">Main (push-to-talk)</div>
        <HotkeyCapture label="" bind:value={settings.s.force_clean_hotkey} />
      </div>
      <div class="hk-pair-col">
        <div class="hk-pair-label">Sticky-invoke (toggle)</div>
        <HotkeyCapture label="" bind:value={settings.s.force_clean_sticky_hotkey} />
      </div>
    </div>
  </div>

  <div class="hotkey-block">
    <div class="hotkey-head">
      <div>
        <div class="hk-label">Draft <span class="hk-tag">F9 default</span></div>
        <div class="hk-desc">Drafts polished output from your brief. Best for emails, Slack, docs.</div>
      </div>
    </div>
    <div class="hk-pair">
      <div class="hk-pair-col">
        <div class="hk-pair-label">Main (push-to-talk)</div>
        <HotkeyCapture label="" bind:value={settings.s.drafting_hotkey} />
      </div>
      <div class="hk-pair-col">
        <div class="hk-pair-label">Sticky-invoke (toggle)</div>
        <HotkeyCapture label="" bind:value={settings.s.drafting_sticky_hotkey} />
      </div>
    </div>
    <label class="check-row small">
      <input
        type="checkbox"
        checked={settings.s.sticky_drafting}
        onchange={(e) => settings.set("sticky_drafting", (e.currentTarget as HTMLInputElement).checked)}
      />
      <span>Default to sticky</span>
    </label>
  </div>

  <details class="hotkey-block-collapsed">
    <summary>
      <span class="hk-label">Advanced cleanup (legacy)</span>
      <span class="hk-desc-inline">— optional dedicated hotkey for the cleanup-only pipeline. Most people don't need this; the F8 LLM toggle covers it.</span>
    </summary>
    <div class="hk-pair">
      <div class="hk-pair-col">
        <div class="hk-pair-label">Main (push-to-talk)</div>
        <HotkeyCapture label="" bind:value={settings.s.advanced_hotkey} />
      </div>
      <div class="hk-pair-col">
        <div class="hk-pair-label">Sticky-invoke (toggle)</div>
        <HotkeyCapture label="" bind:value={settings.s.advanced_sticky_hotkey} />
      </div>
    </div>
    <label class="check-row small">
      <input
        type="checkbox"
        checked={settings.s.sticky_advanced}
        onchange={(e) => settings.set("sticky_advanced", (e.currentTarget as HTMLInputElement).checked)}
      />
      <span>Default to sticky</span>
    </label>
  </details>

  <button class="btn-primary" onclick={saveHotkeys}>Save hotkeys</button>
  <p class="hint">⚠ Hotkey changes take effect after restarting wispr-fox. F10 is retired by default — it activates the menu bar in Outlook, which steals focus from your text field.</p>

  <h3 style="margin-top: 32px;">Behaviour</h3>
  <p class="lede">
    How wispr-fox delivers the result when you've moved on during the LLM gap.
  </p>

  <div class="behavior-block">
    <label class="check-row">
      <input
        type="checkbox"
        checked={settings.s.keep_in_clipboard}
        onchange={(e) => settings.set("keep_in_clipboard", (e.currentTarget as HTMLInputElement).checked)}
      />
      <span><strong>Keep dictation on clipboard</strong> — after every dictation, the cleaned text stays on your clipboard. Press Ctrl+V anywhere as a backup delivery.</span>
    </label>
    <p class="hint">Sacrifices whatever you previously copied. Turn off if you rely on clipboard history flows.</p>
  </div>

  <div class="behavior-block">
    <label class="check-row">
      <input
        type="checkbox"
        checked={settings.s.pull_back_on_navigation}
        onchange={(e) => settings.set("pull_back_on_navigation", (e.currentTarget as HTMLInputElement).checked)}
      />
      <span><strong>Pull focus back to original app</strong> when result is ready — yank the window you started speaking in back to the foreground so the paste lands there.</span>
    </label>
    <p class="hint">Off by default (don't disrupt you if you've moved on). When off and you've navigated away, wispr-fox copies to clipboard and Clippy shows "Copied to clipboard" instead.</p>
  </div>

  <div class="behavior-block">
    <label class="check-row">
      <input
        type="checkbox"
        checked={settings.s.adapt_to_app}
        onchange={(e) => settings.set("adapt_to_app", (e.currentTarget as HTMLInputElement).checked)}
      />
      <span><strong>Adapt tone to active app</strong> — when you press F9, look at which app you're dictating into and hint the LLM at the right register (formal for Outlook, casual for WhatsApp, polished for LinkedIn, etc).</span>
    </label>
    <p class="hint">Only the bucket name (e.g. "email", "chat") is sent to the LLM — never your window title or process name. Affects F9 drafting only; F8 always preserves your voice exactly. Off = let the LLM infer register from the brief alone.</p>
  </div>
</section>
