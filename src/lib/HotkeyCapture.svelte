<script lang="ts">
  // Capture a keybind by clicking "Record" → pressing the desired combo.
  // Output format matches Tauri's Shortcut::from_str: e.g.
  // "CommandOrControl+Super+F8".
  //
  // ── Why capture suspends the global hotkeys ────────────────────────────
  // A registered global shortcut is consumed by the OS before the focused
  // window ever sees the key. So with F8 still registered, pressing F8 into
  // this dialog started a DICTATION and the keydown never reached the listener
  // below — the picker appeared to do nothing for exactly the keys people
  // actually want to bind (F8, F9, and anything already in use). No amount of
  // preventDefault/stopPropagation in the webview can fix that; the event is
  // never delivered. Every app with a rebind UI (Discord, Raycast, Alfred,
  // OBS) handles it the same way: unregister the shortcuts for the duration of
  // the capture, re-register when it ends. `api.suspendHotkeys()` /
  // `api.applyHotkeys()` are that pair, and they are paired in a `finally` so
  // a thrown error, an Escape, or an unmount can never strand the user with no
  // working hotkeys.
  //
  // Note: the Windows key (Super) is special on Windows — Win alone opens
  // the Start menu (the OS intercepts before the app sees it), and Win+Space
  // is the IME picker. Combinations like Win+F8/F9/F10 work fine.

  import { onDestroy } from "svelte";
  import { api } from "$lib/api";

  let {
    value = $bindable(""),
    label = "Hotkey",
    // Called with the newly captured combo. The parent persists it and makes
    // it live; returning a rejected promise (or a string) reports a problem —
    // e.g. the combo already belongs to another binding.
    oncommit = undefined,
  } = $props<{
    value: string;
    label?: string;
    oncommit?: (combo: string) => void | Promise<void>;
  }>();

  let capturing = $state(false);
  let captured = $state("");
  let error = $state("");
  let overlayEl: HTMLDivElement | null = $state(null);
  /** Set while the global shortcuts are torn down, so we only ever restore
   *  what we actually suspended. */
  let suspended = false;

  async function startCapture() {
    capturing = true;
    captured = "";
    error = "";
    // Defer to next frame so the overlay element exists, then focus it.
    queueMicrotask(() => {
      if (overlayEl) overlayEl.focus();
    });
    window.addEventListener("keydown", onKey, true);
    window.addEventListener("keyup", onKeyUp, true);
    try {
      await api.suspendHotkeys();
      suspended = true;
    } catch (e) {
      // Capture still half-works (non-conflicting combos come through), so
      // don't block the user — but tell them why F8 might misbehave.
      console.warn("suspendHotkeys failed", e);
      error = "Couldn't pause the current hotkeys — pressing one may start a recording instead.";
    }
  }

  /** Always restore the global hotkeys. Safe to call when nothing is suspended. */
  async function resumeHotkeys() {
    if (!suspended) return;
    suspended = false;
    try {
      await api.applyHotkeys();
    } catch (e) {
      console.error("applyHotkeys failed — hotkeys may be inactive until restart", e);
    }
  }

  function stopCapture() {
    capturing = false;
    window.removeEventListener("keydown", onKey, true);
    window.removeEventListener("keyup", onKeyUp, true);
    void resumeHotkeys();
  }

  // A navigation or a parent re-render mid-capture must not leave the app with
  // its hotkeys torn down.
  onDestroy(() => {
    window.removeEventListener("keydown", onKey, true);
    window.removeEventListener("keyup", onKeyUp, true);
    void resumeHotkeys();
  });

  async function commit(combo: string) {
    const previous = value;
    value = combo;
    if (!oncommit) return true;
    try {
      await oncommit(combo);
      return true;
    } catch (e) {
      // Rejected — the parent refused this combo (duplicate binding, etc.).
      // Put the old value back so the row doesn't show a binding that isn't real.
      value = previous;
      captured = "";
      error = e instanceof Error ? e.message : String(e);
      return false;
    }
  }

  function onKey(e: KeyboardEvent) {
    e.preventDefault();
    e.stopPropagation();
    e.stopImmediatePropagation();

    if (e.key === "Escape") {
      stopCapture();
      return;
    }

    const mods: string[] = [];
    if (e.ctrlKey) mods.push("CommandOrControl");
    if (e.altKey) mods.push("Alt");
    if (e.shiftKey) mods.push("Shift");
    if (e.metaKey) mods.push("Super");

    const k = e.key;
    // Show pending modifiers while user is still holding them.
    if (k === "Control" || k === "Alt" || k === "Shift" || k === "Meta" || k === "OS") {
      captured = mods.length ? mods.join(" + ") + " + …" : "Press a key…";
      return;
    }

    const keyName = normalizeKey(k);
    if (!keyName) return;

    const combo = [...mods, keyName].join("+");
    captured = combo;
    error = "";
    // Persist first; only close the dialog if the parent accepted it. A
    // rejected combo keeps the overlay open with the reason showing, so the
    // user can immediately try a different key.
    void commit(combo).then((ok) => {
      // Brief delay so the user sees what they captured.
      if (ok) setTimeout(stopCapture, 250);
    });
  }

  function onKeyUp(e: KeyboardEvent) {
    e.preventDefault();
    e.stopPropagation();
  }

  function normalizeKey(k: string): string | null {
    if (k.length === 1) {
      // a..z, 0..9, etc.
      return k.toUpperCase();
    }
    const map: Record<string, string> = {
      " ": "Space",
      Enter: "Enter",
      Tab: "Tab",
      Backspace: "Backspace",
      ArrowUp: "Up",
      ArrowDown: "Down",
      ArrowLeft: "Left",
      ArrowRight: "Right",
      Home: "Home",
      End: "End",
      PageUp: "PageUp",
      PageDown: "PageDown",
      Insert: "Insert",
      Delete: "Delete",
    };
    if (map[k]) return map[k];
    if (/^F\d{1,2}$/.test(k)) return k;
    return null;
  }

  function display(combo: string) {
    if (!combo) return "(none)";
    return combo
      .replace(/CommandOrControl/g, "Ctrl")
      .replace(/Super/g, "Win")
      .replace(/\+/g, " + ");
  }
</script>

<div class="hk-row">
  {#if label}<span class="hk-label">{label}</span>{/if}
  <div class="hk-controls">
    <div class="hk-current" title="Current binding">
      {display(value)}
    </div>
    <button type="button" class="record-btn" onclick={startCapture}>
      <svg viewBox="0 0 16 16" width="12" height="12" aria-hidden="true">
        <circle cx="8" cy="8" r="4" fill="currentColor"/>
      </svg>
      Record
    </button>
  </div>
</div>

{#if capturing}
  <!-- Modal overlay — covers the page so keystrokes never leak to other
       inputs. Gets focus so the keydown listener (capture phase) sees
       every event first. -->
  <div
    class="capture-overlay"
    bind:this={overlayEl}
    tabindex="-1"
    role="dialog"
    aria-modal="true"
    aria-label="Recording hotkey"
  >
    <div class="capture-card">
      <div class="capture-icon">
        <svg viewBox="0 0 32 32" width="36" height="36" aria-hidden="true">
          <circle cx="16" cy="16" r="6" fill="currentColor"/>
          <circle cx="16" cy="16" r="13" fill="none" stroke="currentColor" stroke-width="2" opacity="0.3">
            <animate attributeName="r" values="13;15;13" dur="1.4s" repeatCount="indefinite"/>
            <animate attributeName="opacity" values="0.3;0.6;0.3" dur="1.4s" repeatCount="indefinite"/>
          </circle>
        </svg>
      </div>
      <div class="capture-title">Press your hotkey…</div>
      <div class="capture-shown">{captured || "Waiting…"}</div>
      {#if error}
        <div class="capture-error">{error}</div>
      {/if}
      <div class="capture-hint">
        Hold modifiers (Ctrl / Alt / Shift / Win) then press the key.
        <br />
        Your current hotkeys are paused, so <em>F8</em> and <em>F9</em> won't start a
        recording while this is open.
        <br />
        <em>Esc</em> to cancel. Win+F8/F9/F10 work great; avoid Win+Space (Windows reserves it for IME).
      </div>
      <button type="button" class="capture-cancel" onclick={stopCapture}>Cancel</button>
    </div>
  </div>
{/if}

<style>
  .hk-row {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .hk-label {
    font-size: 12px;
    font-weight: 500;
    color: var(--text-primary);
  }
  .hk-controls {
    display: flex;
    gap: 8px;
    align-items: stretch;
  }
  .hk-current {
    flex: 1;
    background: var(--bg-subtle);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 7px 12px;
    font-family: ui-monospace, "SF Mono", Cascadia, monospace;
    font-size: 12px;
    color: var(--text-primary);
    display: flex;
    align-items: center;
  }
  .record-btn {
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 6px 14px;
    cursor: pointer;
    color: var(--text-primary);
    font-size: 12px;
    font-weight: 500;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    transition: all 120ms ease;
  }
  .record-btn:hover {
    background: var(--bg-subtle);
    border-color: var(--accent);
    color: var(--accent);
  }

  /* Modal overlay for capture session */
  .capture-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.55);
    backdrop-filter: blur(4px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 9999;
    outline: none;
  }
  .capture-card {
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: 16px;
    padding: 32px 36px;
    min-width: 380px;
    max-width: 460px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.3);
    color: var(--text-primary);
  }
  .capture-icon {
    color: var(--danger);
    margin-bottom: 4px;
  }
  .capture-title {
    font-size: 16px;
    font-weight: 600;
    color: var(--text-primary);
  }
  .capture-shown {
    background: var(--bg-subtle);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 10px 18px;
    font-family: ui-monospace, "SF Mono", Cascadia, monospace;
    font-size: 14px;
    color: var(--text-primary);
    min-width: 200px;
    text-align: center;
  }
  .capture-error {
    font-size: 12px;
    color: var(--danger);
    background: var(--danger-fade);
    border-radius: 8px;
    padding: 7px 12px;
    text-align: center;
    max-width: 360px;
    line-height: 1.45;
  }
  .capture-hint {
    font-size: 12px;
    color: var(--text-secondary);
    line-height: 1.6;
    text-align: center;
    max-width: 360px;
  }
  .capture-hint em {
    background: var(--bg-subtle);
    padding: 1px 6px;
    border-radius: 4px;
    font-style: normal;
    color: var(--text-primary);
    font-family: ui-monospace, monospace;
    font-size: 11px;
  }
  .capture-cancel {
    background: transparent;
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 6px 18px;
    cursor: pointer;
    color: var(--text-secondary);
    font-size: 12px;
    margin-top: 4px;
  }
  .capture-cancel:hover {
    color: var(--text-primary);
    border-color: var(--text-secondary);
  }
</style>
