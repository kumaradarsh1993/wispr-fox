<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { settings } from "$lib/settings-store.svelte";
  import { flash } from "$lib/settings-toast.svelte";
  import HotkeyCapture from "$lib/HotkeyCapture.svelte";
  import { api, type InputDeviceInfo, type MicReadyEvent } from "$lib/api";
  import { isMac } from "$lib/hotkey-display";
  import MicMeter from "$lib/MicMeter.svelte";

  // ── Hotkeys ──────────────────────────────────────────────────────────────
  // Bindings apply the moment they're captured. The old model was
  // bind:value → a "Save hotkeys" button → "Restart wispr-fox to apply", which
  // had two problems: the restart was never actually necessary (registration is
  // live now), and `bind:value` mutated the in-memory settings without
  // persisting, so an unsaved rebind would ride along on the NEXT unrelated
  // settings write. Commit-on-capture fixes both.

  type HotkeyField =
    | "light_hotkey"
    | "force_clean_hotkey"
    | "drafting_hotkey"
    | "advanced_hotkey";

  const HOTKEY_LABELS: Record<HotkeyField, string> = {
    light_hotkey: "Transcribe",
    force_clean_hotkey: "Transcribe + force-clean",
    drafting_hotkey: "Draft",
    advanced_hotkey: "Advanced cleanup",
  };

  const HOTKEY_FIELDS = Object.keys(HOTKEY_LABELS) as HotkeyField[];

  /** Persist a captured binding and make it live immediately.
   *  Throws (which HotkeyCapture surfaces inline and reverts) when the combo
   *  is already taken — two identical registrations mean the second silently
   *  loses, which looks like "the app ignored my hotkey". */
  async function commitHotkey(field: HotkeyField, combo: string) {
    const clash = HOTKEY_FIELDS.find(
      (f) => f !== field && settings.s[f] && settings.s[f] === combo,
    );
    if (clash) {
      throw new Error(`${combo.replace(/CommandOrControl/g, "Ctrl").replace(/Super/g, "Win")} is already used by "${HOTKEY_LABELS[clash]}". Pick a different key.`);
    }
    // set_settings refreshes live registrations, but deliberately leaves a
    // capture-suspended registrar alone. HotkeyCapture resumes exactly once
    // after this commit resolves, using the newly stored binding.
    await settings.set(field, combo);
    flash(`${HOTKEY_LABELS[field]} is now ${combo.replace(/CommandOrControl/g, "Ctrl").replace(/Super/g, "Win")}`);
  }

  async function clearHotkey(field: HotkeyField) {
    await settings.set(field, "");
    flash(`${HOTKEY_LABELS[field]} unbound`);
  }

  // ── Microphone ───────────────────────────────────────────────────────────
  let devices = $state<InputDeviceInfo[]>([]);
  let devicesError = $state("");
  /** "" means system default — the value the settings field stores as null. */
  let selectedDevice = $derived(settings.s.input_device ?? "");
  /** The saved mic isn't in the live list: it's off, unplugged, or unpaired.
   *  Say so plainly instead of silently showing something else selected. */
  let savedDeviceMissing = $derived(
    Boolean(selectedDevice) && devices.length > 0 && !devices.some((d) => d.name === selectedDevice),
  );

  async function refreshDevices() {
    try {
      devices = await api.listInputDevices();
      devicesError = "";
    } catch (e) {
      devicesError = String(e);
      devices = [];
    }
  }

  async function pickDevice(name: string) {
    await settings.set("input_device", name || null);
    flash(name ? `Recording from ${name}` : "Using the system default microphone");
    // Keep a running test pointed at whatever is now selected.
    if (testing) await restartTest();
  }

  // ── Mic test ─────────────────────────────────────────────────────────────
  // Enumeration alone can't tell you a device works. After a sleep/wake cycle
  // a Bluetooth transmitter can keep its "connected" indicator and stay listed
  // by Windows while delivering no audio whatsoever — recoverable only by
  // power-cycling the mic. A live meter catches that in two seconds; a device
  // list never will.
  let testing = $state(false);
  let testDevice = $state("");
  let testError = $state("");
  let rmsDb = $state(-120);
  let peakDb = $state(-120);
  /** Head-gap for the test stream: how long the mic took to deliver its first
   *  audio after we asked. This is the number that explains "it cut off my
   *  first few words", and the mic test is the right place to see it BEFORE a
   *  real recording is damaged. */
  let readyMs = $state<number | null>(null);
  let unlistenMeter: UnlistenFn | null = null;
  let unlistenLive: UnlistenFn | null = null;

  async function startTest() {
    testError = "";
    readyMs = null;
    rmsDb = -120;
    peakDb = -120;
    try {
      testDevice = await api.startMicTest(selectedDevice || null);
      testing = true;
    } catch (e) {
      testError = String(e);
      testing = false;
    }
  }

  async function stopTest() {
    testing = false;
    readyMs = null;
    rmsDb = -120;
    peakDb = -120;
    try {
      await api.stopMicTest();
    } catch (e) {
      console.warn("stopMicTest failed", e);
    }
  }

  async function restartTest() {
    await stopTest();
    await startTest();
  }

  onMount(() => {
    refreshDevices();
    listen<{ rms_dbfs: number; peak_dbfs: number }>("wispr:mic_meter", (e) => {
      if (!testing) return;
      rmsDb = e.payload.rms_dbfs;
      peakDb = e.payload.peak_dbfs;
    }).then((u) => (unlistenMeter = u));
    listen<MicReadyEvent>("wispr:mic_ready", (e) => {
      if (testing && e.payload.source === "preview") readyMs = e.payload.ready_ms;
    }).then((u) => (unlistenLive = u));
  });

  // Leaving the page must release the mic — a preview stream left open holds
  // the OS "mic in use" indicator on and can block other apps from the device.
  onDestroy(() => {
    unlistenMeter?.();
    unlistenLive?.();
    if (testing) api.stopMicTest().catch(() => {});
  });

  /** Is the measured head-gap bad enough that opening words are being lost? */
  let slowHandover = $derived(readyMs !== null && readyMs > 2500);
  /** Present in the enumeration, but no audio is arriving at all. */
  let noAudio = $derived(testing && readyMs === null);

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
  <h2>Voice &amp; shortcuts</h2>
  <p class="lede">
    Which mic wispr-fox records from, and a way to confirm it actually works
    before it costs you a dictation.
  </p>

  <div class="mic-block">
    <div class="mic-row">
      <label class="mic-label" for="mic-select">Input device</label>
      <div class="mic-controls">
        <select
          id="mic-select"
          value={selectedDevice}
          onchange={(e) => pickDevice((e.currentTarget as HTMLSelectElement).value)}
        >
          <option value="">System default</option>
          {#each devices as d (d.name)}
            <option value={d.name}>{d.name}{d.is_default ? " (system default)" : ""}</option>
          {/each}
          {#if savedDeviceMissing}
            <!-- Keep the saved pick visible and selected even though it's gone,
                 so the dropdown doesn't silently appear to show a different mic. -->
            <option value={selectedDevice}>{selectedDevice} — not connected</option>
          {/if}
        </select>
        <button class="btn-ghost" onclick={refreshDevices}>Refresh</button>
      </div>
    </div>

    {#if devicesError}
      <p class="warn-line">Couldn't list microphones: {devicesError}</p>
    {:else if savedDeviceMissing}
      <p class="warn-line">
        <strong>{selectedDevice}</strong> isn't connected right now. Recordings will use the
        system default until it's back — you won't lose a dictation over it.
      </p>
    {/if}

    <div class="test-row">
      <button class="btn-primary" onclick={() => (testing ? stopTest() : startTest())}>
        {testing ? "Stop test" : "Test microphone"}
      </button>
      {#if testing}
        <span class="test-device">Listening on {testDevice}</span>
      {/if}
    </div>

    {#if testError}
      <p class="warn-line">{testError}</p>
    {/if}

    {#if testing}
      <MicMeter {rmsDb} {peakDb} />

      <div class="handover">
        {#if noAudio}
          <p class="warn-line">
            No audio yet. If this doesn't clear in a few seconds the device is listed but not
            actually sending anything — power-cycle the mic (a Bluetooth mic can keep its
            "connected" light after your laptop sleeps and still be dead).
          </p>
        {:else if slowHandover}
          <p class="warn-line">
            Mic took <strong>{(readyMs! / 1000).toFixed(1)}s</strong> to start. Anything you say
            in that window is lost. See the fixes below.
          </p>
        {:else if readyMs !== null}
          <p class="ok-line">
            Mic started in <strong>{(readyMs / 1000).toFixed(2)}s</strong>. Speak normally and
            aim for the green band.
          </p>
        {/if}
      </div>
    {/if}
  </div>

  <details class="guidance">
    <summary>
      <span class="hk-label">Mic starts slowly, or cuts off my first words</span>
      <span class="hk-desc-inline">Two different causes, two different fixes.</span>
    </summary>
    <div class="guidance-body">
      <p>
        wispr-fox measures how long your mic takes to deliver its first audio after you press
        the hotkey. Anything spoken before that never reaches the recording — no software can
        recover it, so the fix has to happen at the device.
      </p>
      <h4>Built-in or wired mic</h4>
      <p>
        Almost always Windows processing the input before handing it over. Open
        <strong>Sound settings → your microphone → Properties</strong>, then:
      </p>
      <ul>
        <li>turn <strong>off</strong> all audio enhancements</li>
        <li>
          turn <strong>off</strong> "allow applications to take exclusive control of this device"
        </li>
      </ul>
      <p class="tight">
        This typically takes a ~5 second wake-up down to effectively instant.
      </p>
      <h4>Bluetooth mic</h4>
      <p>
        A different mechanism — the enhancements fix above will not help. The delay is the
        headset audio link coming up, plus, on transmitters with onboard noise cancellation,
        the mic negotiating <em>itself</em> out of NC mode (which it must, because NC can't run
        while streaming over Bluetooth). That alone can be 7–8 seconds.
      </p>
      <ul>
        <li>
          Turn <strong>noise cancellation off on the mic</strong> before connecting it. This is
          usually most of the delay.
        </li>
        <li>Reconnecting leaves 1–2 seconds that is inherent to Bluetooth and can't be removed.</li>
        <li>
          After your laptop sleeps, a Bluetooth mic can show as connected and still send
          nothing. Power-cycle the mic; the test above will confirm it.
        </li>
      </ul>
      <p class="tight">
        While you wait, the avatar shows a "hold on" state and only switches to recording once
        audio is genuinely flowing — so you can see the gap instead of talking into a void.
      </p>
    </div>
  </details>

  <h3>Quiet recordings</h3>
  <p class="lede">
    Quiet audio doesn't fail loudly — the transcript comes back looking fine with words
    silently missing. wispr-fox measures every recording's level and can boost a copy before
    sending it. The saved recording is never modified.
  </p>
  <div class="behavior-block">
    <label class="check-row" title="Peak-normalises the copy sent for transcription. Costs nothing and prevents silently dropped words.">
      <input
        type="checkbox"
        checked={settings.s.auto_gain}
        onchange={(e) => settings.set("auto_gain", (e.currentTarget as HTMLInputElement).checked)}
      />
      <span><strong>Boost audio that came in too quiet</strong> before transcribing.</span>
    </label>
  </div>

  <h2 class="section-gap">Dictation keys</h2>
  <p class="lede">Bind the keys that start and stop dictation. Changes apply immediately.</p>
  {#if isMac()}
    <p class="lede tight" title="Bare F8/F9 can be swallowed by media-key behavior unless macOS is set for standard function keys.">
      <strong>macOS defaults to Option+Space and Option+Enter.</strong>
    </p>
  {/if}
  <p class="lede tight">
    Tap for less than 700 ms to latch recording; press any dictation key or Esc to stop and send.
    Hold for 700 ms or longer to use hold-to-talk and send on release.
  </p>

  <div class="hotkey-block">
    <div class="hotkey-head">
      <div>
        <div class="hk-label">Transcribe <span class="hk-tag">{isMac() ? "Option+Space" : "F8"} default</span></div>
        <div class="hk-desc">Voice to text. The sidebar Clean toggle decides whether this also runs LLM cleanup.</div>
      </div>
    </div>
    <HotkeyCapture
      label=""
      bind:value={settings.s.light_hotkey}
      oncommit={(c) => commitHotkey("light_hotkey", c)}
    />
  </div>

  <div class="hotkey-block">
    <div class="hotkey-head">
      <div>
        <div class="hk-label">Transcribe + force-clean <span class="hk-tag">{isMac() ? "Shift+Option+Space" : "Shift+F8"} default</span></div>
        <div class="hk-desc">Runs Transcribe with cleanup on for this one dictation without changing your saved preference.</div>
      </div>
    </div>
    <HotkeyCapture
      label=""
      bind:value={settings.s.force_clean_hotkey}
      oncommit={(c) => commitHotkey("force_clean_hotkey", c)}
    />
  </div>

  <div class="hotkey-block">
    <div class="hotkey-head">
      <div>
        <div class="hk-label">Draft <span class="hk-tag">{isMac() ? "Option+Enter" : "F9"} default</span></div>
        <div class="hk-desc">Turns a spoken brief into polished output for email, chat, docs, or social posts.</div>
      </div>
    </div>
    <HotkeyCapture
      label=""
      bind:value={settings.s.drafting_hotkey}
      oncommit={(c) => commitHotkey("drafting_hotkey", c)}
    />
  </div>

  <details class="hotkey-block-collapsed">
    <summary>
      <span class="hk-label">Advanced cleanup</span>
      <span class="hk-desc-inline">Optional legacy cleanup-only binding. Most users can leave this unbound.</span>
    </summary>
    <HotkeyCapture
      label=""
      bind:value={settings.s.advanced_hotkey}
      oncommit={(c) => commitHotkey("advanced_hotkey", c)}
    />
    {#if settings.s.advanced_hotkey}
      <button class="btn-unbind" onclick={() => clearHotkey("advanced_hotkey")}>Unbind</button>
    {/if}
  </details>

  <p class="hint">Press <strong>Esc</strong> during a recording to stop and send.</p>

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

<style>
  .section-gap {
    margin-top: 34px;
  }

  .mic-block {
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: 12px;
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    margin-bottom: 14px;
  }
  .mic-row {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .mic-label {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-secondary);
  }
  .mic-controls {
    display: flex;
    gap: 8px;
  }
  .mic-controls select {
    flex: 1;
    min-width: 0;
    background: var(--bg-subtle);
    border: 1px solid var(--border);
    border-radius: 9px;
    padding: 8px 10px;
    font-size: 13px;
    color: var(--text-primary);
    font-family: inherit;
    cursor: pointer;
  }
  .mic-controls select:focus {
    outline: none;
    border-color: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-fade);
  }

  .test-row {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
  }
  .test-device {
    font-size: 12px;
    color: var(--text-secondary);
  }
  .handover {
    margin-top: 2px;
  }

  .warn-line,
  .ok-line {
    font-size: 12.5px;
    line-height: 1.5;
    margin: 0;
    padding: 8px 11px;
    border-radius: 8px;
  }
  .warn-line {
    color: var(--danger);
    background: var(--danger-fade);
  }
  .ok-line {
    color: var(--text-secondary);
    background: var(--bg-subtle);
  }

  .btn-ghost {
    background: transparent;
    border: 1px solid var(--border);
    color: var(--text-primary);
    border-radius: 9px;
    padding: 8px 14px;
    font-size: 12.5px;
    cursor: pointer;
    font-family: inherit;
    flex-shrink: 0;
  }
  .btn-ghost:hover {
    background: var(--bg-subtle);
    border-color: var(--accent);
  }
  .btn-unbind {
    align-self: flex-start;
    margin-top: 6px;
    background: transparent;
    border: none;
    color: var(--text-secondary);
    font-size: 11.5px;
    cursor: pointer;
    font-family: inherit;
    text-decoration: underline;
    padding: 0;
  }
  .btn-unbind:hover {
    color: var(--danger);
  }

  .guidance {
    border: 1px solid var(--border);
    border-radius: 12px;
    padding: 12px 16px;
    margin-bottom: 8px;
    background: var(--bg-card);
  }
  .guidance summary {
    cursor: pointer;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .guidance-body {
    margin-top: 12px;
    font-size: 12.5px;
    line-height: 1.6;
    color: var(--text-secondary);
  }
  .guidance-body h4 {
    font-size: 12.5px;
    font-weight: 700;
    color: var(--text-primary);
    margin: 14px 0 4px;
  }
  .guidance-body p {
    margin: 0 0 6px;
  }
  .guidance-body p.tight {
    margin-bottom: 0;
  }
  .guidance-body ul {
    margin: 0 0 6px;
    padding-left: 20px;
  }
  .guidance-body li {
    margin-bottom: 3px;
  }
  .guidance-body strong {
    color: var(--text-primary);
  }
</style>
