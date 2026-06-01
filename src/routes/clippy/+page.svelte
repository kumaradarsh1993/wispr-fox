<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { LogicalPosition } from "@tauri-apps/api/window";
  import { skinStore } from "$lib/skin-store.svelte";
  import clippyJs from "$lib/clippyjs-vendor/clippy.js";

  type ClippyState = "idle" | "listening" | "thinking" | "writing" | "pasting";
  type Mode = "light" | "advanced";

  // `state` is the *actual* flow state from Rust (changes fast during pipeline).
  // `displayState` is what Clippy is currently animating — it lags `state`
  // through a queue so each post-listening animation gets at least MIN_DWELL_MS
  // of airtime regardless of how fast the backend finishes transcribing /
  // cleaning / injecting.
  // Frontend watchdog — last-ditch defense if the backend ever fails to
  // emit a terminal state. If Clippy has been in any non-idle state for
  // longer than WATCHDOG_MS without progressing, force-reset to idle and
  // show a generic error toast. The Rust wrapper in flow.rs handles every
  // failure I know of; this catches the unknown unknowns (frontend lost
  // the wispr:state event, Tauri IPC stutter, etc.).
  const WATCHDOG_MS = 90_000;
  let watchdogTimer: ReturnType<typeof setTimeout> | null = null;
  function armWatchdog() {
    if (watchdogTimer) clearTimeout(watchdogTimer);
    watchdogTimer = setTimeout(() => {
      console.warn("[clippy] watchdog fired — forcing state back to idle");
      state = "idle";
      displayState = "idle";
      displayQueue = [];
      if (displayTimer) {
        clearTimeout(displayTimer);
        displayTimer = null;
      }
      showToast("Took too long — try again", "error", 5000);
    }, WATCHDOG_MS);
  }
  function disarmWatchdog() {
    if (watchdogTimer) {
      clearTimeout(watchdogTimer);
      watchdogTimer = null;
    }
  }

  let state = $state<ClippyState>("idle");
  let displayState = $state<ClippyState>("idle");
  let mode = $state<Mode>("light");
  let blinkOpen = $state(true);
  let lookDir = $state<"left" | "right" | "center">("center");
  // Hover state: true while the cursor is over the Clippy window. Drives
  // the "Clippy notices you" beat — eyes scale up, pupils track cursor.
  let hovering = $state(false);
  // Pupil offset (in SVG units, viewBox is -20..160 wide / 0..170 tall).
  // While hovering, these track the cursor relative to Clippy's centre;
  // while idle, the existing lookDir 3-state sway drives eyeShiftX.
  let hoverShiftX = $state(0);
  let hoverShiftY = $state(0);

  // Transient message override — when Rust emits `wispr:clippy_message`
  // (e.g. "Copied to clipboard" after a cross-process silent delivery),
  // we show this text in the bubble for ~3s, overriding the state-driven
  // label. Empty string = no override.
  let toastMessage = $state("");
  let toastKind = $state<"info" | "error">("info");
  let toastTimer: ReturnType<typeof setTimeout> | null = null;
  function showToast(msg: string, kind: "info" | "error" = "info", durationMs = 3000) {
    toastMessage = msg;
    toastKind = kind;
    if (toastTimer) clearTimeout(toastTimer);
    toastTimer = setTimeout(() => {
      toastMessage = "";
      toastTimer = null;
    }, durationMs);
  }

  const MIN_DWELL_MS = 1400;
  // States that need full play-time (pipeline) — when displayState is one of
  // these, the next transition must wait MIN_DWELL_MS.
  function needsDwell(s: ClippyState): boolean {
    return s === "thinking" || s === "writing" || s === "pasting";
  }

  let displayQueue: ClippyState[] = [];
  let displayTimer: ReturnType<typeof setTimeout> | null = null;

  function enqueueDisplay(target: ClippyState) {
    const last = displayQueue.length > 0 ? displayQueue[displayQueue.length - 1] : displayState;
    if (last === target) return; // coalesce duplicates
    displayQueue.push(target);
    if (!displayTimer) tickDisplay();
  }

  function tickDisplay() {
    const next = displayQueue.shift();
    if (next === undefined) {
      displayTimer = null;
      return;
    }
    displayState = next;
    const delay = needsDwell(next) ? MIN_DWELL_MS : 0;
    displayTimer = setTimeout(tickDisplay, delay);
  }

  // Drive displayState from the actual state changes.
  $effect(() => {
    enqueueDisplay(state);
  });

  // "Phew" transient — true for ~700ms right after listening ends. Used by
  // the stylized skin to render a sweat-drop + relief beat between states.
  // Triggered off displayState transitions (not raw state) so the visual
  // story stays consistent.
  let phewActive = $state(false);
  let _prevDisplay: ClippyState = "idle";
  $effect(() => {
    const cur = displayState;
    if (_prevDisplay === "listening" && cur !== "listening") {
      phewActive = true;
      setTimeout(() => { phewActive = false; }, 700);
    }
    _prevDisplay = cur;
  });

  // Skin comes from the shared store (driven by sidebar picker via events).
  let skin = $derived(skinStore.current);

  // Ask the backend to force-repaint the floater (size nudge) — used to heal
  // a blank-after-resume WebView2 surface. Safe to over-call: the backend
  // no-ops when the window is hidden.
  function recoverFloater(why: string) {
    console.warn(`[clippy] ${why} — recovering floater`);
    invoke("recover_clippy_window").catch((e) =>
      console.warn("[clippy] recover_clippy_window failed", e),
    );
  }

  // Recover on skin change too. If the surface died after resume, the user's
  // instinct is to fiddle with the avatar picker — so a skin switch should
  // itself force a repaint. Skip the very first run (initial mount, healthy
  // window) to avoid a needless nudge.
  let _skinInit = false;
  $effect(() => {
    skin; // track
    if (!_skinInit) {
      _skinInit = true;
      return;
    }
    recoverFloater("skin changed");
  });

  // For "real-clippy" — the actual Microsoft Clippy via vendored clippyts.
  let realClippyAgent: any = null;
  let realClippyError = $state<string | null>(null);
  let realClippyLoading = $state(false);

  function loadRealClippy() {
    if (realClippyAgent || realClippyLoading) return;
    console.log("[clippy] loadRealClippy() starting", clippyJs);
    realClippyLoading = true;
    realClippyError = null;
    try {
      clippyJs.load({
        name: "Clippy",
        successCb: (agent: any) => {
          console.log("[clippy] agent loaded", agent);
          realClippyAgent = agent;
          realClippyLoading = false;

          // Show without animation (true = force show, skip the "Show" animation).
          try {
            agent.show(true);
          } catch (e) {
            console.warn("[clippy] agent.show failed", e);
          }

          // Position the agent element inside OUR window. clippyts defaults to
          // 80% of viewport which puts it offscreen in our 220x240 frame.
          requestAnimationFrame(() => {
            const el = document.querySelector("body > .clippy") as HTMLElement | null;
            if (el) {
              el.style.position = "absolute";
              el.style.left = "50%";
              el.style.top = "auto";
              el.style.bottom = "20px";
              el.style.transform = "translateX(-50%)";
              el.style.zIndex = "1";
              console.log("[clippy] positioned el", el, el.getBoundingClientRect());
            } else {
              console.warn("[clippy] no body > .clippy element found");
            }
          });

          // Kick off a friendly initial animation.
          setTimeout(() => {
            try { agent.play("Greeting"); } catch {}
          }, 250);
        },
        failCb: (err: any) => {
          console.error("[clippy] load failed", err);
          realClippyError = String(err);
          realClippyLoading = false;
        },
      });
    } catch (e) {
      console.error("[clippy] load threw", e);
      realClippyError = String(e);
      realClippyLoading = false;
    }
  }

  function teardownRealClippy() {
    clearStateTimer();
    if (realClippyAgent) {
      try { realClippyAgent.hide(true); } catch {}
      realClippyAgent = null;
    }
    document.querySelectorAll("body > .clippy").forEach((el) => el.remove());
    document.querySelectorAll(".clippy-balloon").forEach((el) => el.remove());
  }

  // Real-Clippy animations driven from app state.
  // Light mode = simple cleanup (lighter, friendlier animations).
  // Advanced mode = heavier processing (more dramatic, wizardly animations).
  function realClippyAnim(s: ClippyState, m: Mode): string | null {
    if (m === "advanced") {
      switch (s) {
        case "listening": return "GetAttention";
        case "thinking":  return "Processing";   // gears turning
        case "writing":   return "GetWizardy";   // wizard wand — heavier transformation
        case "pasting":   return "Congratulate";
        case "idle":      return null;
        default:          return null;
      }
    }
    // Light (F8) — default, friendlier set.
    switch (s) {
      case "listening": return "GetAttention";
      case "thinking":  return "Thinking";
      case "writing":   return "Writing";
      case "pasting":   return "Congratulate";
      case "idle":      return null;
      default:          return null;
    }
  }

  // Load / teardown when skin changes.
  $effect(() => {
    if (skin === "real-clippy") {
      loadRealClippy();
    } else {
      teardownRealClippy();
    }
  });

  // One animation per (state, mode). No random pool — Clippy LOOPS the same
  // animation while in a state. User feedback: random cycling was disruptive
  // ("zoom out, say hello, back to reading"). Single-animation loops feel
  // deliberate and stay connected to what's actually happening.
  type AnimMap = Record<ClippyState, string | null>;
  const ANIMS_LIGHT: AnimMap = {
    idle: null, // let clippyts pick subtle Idle* animations automatically
    listening: "GetAttention",   // peers/leans toward user
    thinking: "Thinking",         // hand on chin
    writing: "Writing",            // pen + paper
    pasting: "Congratulate",       // celebrate
  };
  const ANIMS_ADVANCED: AnimMap = {
    idle: null,
    listening: "GetAttention",
    thinking: "Processing",        // heavier "gears turning" feel
    writing: "GetWizardy",         // magic wand transformation
    pasting: "Congratulate",
  };

  let stateTimer: ReturnType<typeof setInterval> | null = null;

  function animFor(s: ClippyState, m: Mode): string | null {
    const map = m === "advanced" ? ANIMS_ADVANCED : ANIMS_LIGHT;
    return map[s];
  }

  function clearStateTimer() {
    if (stateTimer) {
      clearInterval(stateTimer);
      stateTimer = null;
    }
  }

  // Drive real-Clippy animations from displayState + mode (uses display
  // state so animations play out fully through the pipeline).
  $effect(() => {
    const curSkin = skin;
    const curState = displayState;
    const curMode = mode;

    if (curSkin !== "real-clippy") {
      clearStateTimer();
      return;
    }
    if (!realClippyAgent) return;

    clearStateTimer();

    const name = animFor(curState, curMode);
    console.log("[clippy] state effect", { state: curState, mode: curMode, animation: name });

    if (!name) {
      // Idle / unknown — let clippyts pick subtle Idle* animations automatically.
      return;
    }

    const playOne = () => {
      try {
        const ok = realClippyAgent?.play(name, 5000);
        if (!ok) console.warn("[clippy] play returned false:", name);
      } catch (e) {
        console.warn("[clippy] play threw", e);
      }
    };
    playOne();

    if (curState === "pasting") return;

    // Loop the SAME animation every 5s so behavior stays connected to state.
    stateTimer = setInterval(playOne, 5000);
  });

  function mapFlow(s: string): ClippyState {
    switch (s) {
      case "recording":
        return "listening";
      case "transcribing":
        return "thinking";
      case "cleaning":
        return "writing";
      case "injecting":
        return "pasting";
      default:
        return "idle";
    }
  }

  onMount(() => {
    skinStore.subscribe();

    let unlisten: (() => void) | undefined;
    let unlistenMode: (() => void) | undefined;
    let unlistenMsg: (() => void) | undefined;
    let unlistenErr: (() => void) | undefined;
    let unlistenActiveApp: (() => void) | undefined;
    let unlistenSttProv: (() => void) | undefined;
    let unlistenLlmProv: (() => void) | undefined;
    let unlistenWarn: (() => void) | undefined;
    listen<string>("wispr:clippy_message", (e) => {
      console.log("[clippy] wispr:clippy_message", e.payload);
      showToast(e.payload, "info", 3000);
    }).then((u) => (unlistenMsg = u));
    listen<string>("wispr:flow_error", (e) => {
      console.warn("[clippy] wispr:flow_error", e.payload);
      // Force-reset all state — Rust's wrapper also emits "idle" but be
      // defensive in case events arrive out of order.
      state = "idle";
      displayState = "idle";
      displayQueue = [];
      if (displayTimer) {
        clearTimeout(displayTimer);
        displayTimer = null;
      }
      disarmWatchdog();
      showToast(e.payload, "error", 5000);
    }).then((u) => (unlistenErr = u));
    listen<string>("wispr:state", (e) => {
      const next = mapFlow(e.payload);
      console.log("[clippy] wispr:state", e.payload, "→", next);
      state = next;
      // Clear stale provider labels at the start/end of a run so a finished
      // pipeline doesn't leave "transcribing · Groq" hanging around.
      if (next === "idle" || next === "listening") {
        sttProvider = "";
        llmProvider = "";
      }
      // Watchdog policy: arm ONLY for transient pipeline states that have
      // a known upper bound (thinking/writing/pasting). Recording is
      // user-controlled — a 5-minute monologue is legitimate, not a stuck
      // pipeline. v0.4.1 had a bug where the watchdog armed on
      // `listening` and fired at 90s mid-recording, force-resetting the
      // UI to idle and showing "Took too long" even though Rust was
      // happily still recording.
      if (next === "idle" || next === "listening") {
        disarmWatchdog();
      } else {
        armWatchdog();
      }
      if (next === "pasting") {
        setTimeout(() => {
          state = "idle";
          disarmWatchdog();
        }, 800);
      }
    }).then((u) => (unlisten = u));
    listen<string>("wispr:mode", (e) => {
      const m = e.payload === "advanced" ? "advanced" : "light";
      console.log("[clippy] wispr:mode", m);
      mode = m;
    }).then((u) => (unlistenMode = u));
    // Friendly app name from Rust focus-capture. Empty payload = unknown.
    listen<string>("wispr:active_app", (e) => {
      console.log("[clippy] wispr:active_app", e.payload);
      activeApp = e.payload ?? "";
    }).then((u) => (unlistenActiveApp = u));
    // Provider attribution for the in-progress stages.
    listen<string>("wispr:stt_provider", (e) => {
      sttProvider = e.payload ?? "";
    }).then((u) => (unlistenSttProv = u));
    listen<string>("wispr:llm_provider", (e) => {
      llmProvider = e.payload ?? "";
    }).then((u) => (unlistenLlmProv = u));
    // Non-fatal cleanup warning (LLM step failed, raw text pasted). Shown as
    // an error-styled toast WITHOUT resetting pipeline state — unlike
    // flow_error, the run actually succeeded (the user got their text).
    listen<string>("wispr:clippy_warning", (e) => {
      console.warn("[clippy] wispr:clippy_warning", e.payload);
      showToast(e.payload, "error", 5000);
    }).then((u) => (unlistenWarn = u));

    // Blinks during idle AND listening so Clippy feels alive while attentive.
    // Uses displayState (the visible state) so blinks follow what's drawn.
    const blinkTimer = setInterval(() => {
      if (displayState !== "idle" && displayState !== "listening") return;
      blinkOpen = false;
      setTimeout(() => (blinkOpen = true), 100);
      // Double-blink during listening to emphasize attentiveness.
      if (displayState === "listening") {
        setTimeout(() => (blinkOpen = false), 220);
        setTimeout(() => (blinkOpen = true), 320);
      }
    }, 2200 + Math.random() * 1500);

    const lookTimer = setInterval(() => {
      if (displayState !== "idle") return;
      const dirs: Array<"left" | "right" | "center"> = ["left", "right", "center"];
      lookDir = dirs[Math.floor(Math.random() * dirs.length)];
    }, 6000 + Math.random() * 6000);

    // Resume watchdog. This window is a transparent, always-on-top WebView2
    // floater; on Windows its DirectComposition surface is torn down when the
    // machine sleeps (DWM restarts on resume) and the fox goes invisible even
    // though the window is still "shown". The webview's JS keeps running, so a
    // wall-clock timer is a reliable suspend detector: if far more than the
    // 1s interval elapsed between ticks, the host was almost certainly
    // suspended (or heavily throttled) — ask Rust to force a repaint.
    let lastBeat = Date.now();
    const resumeWatch = setInterval(() => {
      const now = Date.now();
      const gap = now - lastBeat;
      lastBeat = now;
      if (gap > 4000) recoverFloater(`resume detected (gap ${gap}ms)`);
    }, 1000);
    // Also self-heal the instant the floater regains visibility/focus —
    // covers the case where the surface is dead and the 1s tick hasn't fired
    // yet. These are cheap; the backend only nudges when the window is
    // actually meant to be on-screen.
    const onVisible = () => {
      if (document.visibilityState === "visible") recoverFloater("became visible");
    };
    const onFocus = () => recoverFloater("regained focus");
    document.addEventListener("visibilitychange", onVisible);
    window.addEventListener("focus", onFocus);

    // Layer 3: periodic ping to the Rust watchdog so it knows the webview
    // is alive.  If this stops arriving, Rust force-repaints the floater.
    const jsPingInterval = setInterval(() => {
      invoke("js_heartbeat_ping").catch(() => {});
    }, 10_000);

    const saved = localStorage.getItem("wispr.clippy.pos");
    if (saved) {
      try {
        const { x, y } = JSON.parse(saved);
        getCurrentWindow().setPosition(new LogicalPosition(x, y));
      } catch {
        /* ignore */
      }
    }

    let posSaveTimer: ReturnType<typeof setTimeout> | undefined;
    const persist = async () => {
      try {
        const pos = await getCurrentWindow().outerPosition();
        localStorage.setItem(
          "wispr.clippy.pos",
          JSON.stringify({ x: pos.x, y: pos.y }),
        );
      } catch {
        /* ignore */
      }
    };
    const onMove = () => {
      if (posSaveTimer) clearTimeout(posSaveTimer);
      posSaveTimer = setTimeout(persist, 500);
    };
    window.addEventListener("mouseup", onMove);

    return () => {
      unlisten?.();
      unlistenMode?.();
      unlistenMsg?.();
      unlistenErr?.();
      unlistenActiveApp?.();
      unlistenSttProv?.();
      unlistenLlmProv?.();
      unlistenWarn?.();
      disarmWatchdog();
      clearInterval(blinkTimer);
      clearInterval(lookTimer);
      clearInterval(resumeWatch);
      clearInterval(jsPingInterval);
      document.removeEventListener("visibilitychange", onVisible);
      window.removeEventListener("focus", onFocus);
      window.removeEventListener("mouseup", onMove);
    };
  });

  // Double-click anywhere on Clippy → bring the main window forward.
  // Replaces the old X-dismiss button (looked odd on hover and conflicted
  // with the user's mental model — Clippy IS the entry point now).
  async function openMainWindow() {
    try {
      const { WebviewWindow } = await import("@tauri-apps/api/webviewWindow");
      const main = await WebviewWindow.getByLabel("main");
      if (!main) return;
      await main.show();
      await main.unminimize();
      await main.setFocus();
    } catch (e) {
      console.warn("openMainWindow failed", e);
    }
  }

  // When hovering, eyes follow the cursor (continuous tracking). Otherwise
  // they sway through the random 3-state look-direction. Hover takes
  // priority for the "noticing you" feel.
  let eyeShiftX = $derived(
    hovering ? hoverShiftX : (lookDir === "left" ? -2.2 : lookDir === "right" ? 2.2 : 0),
  );
  let eyeShiftY = $derived(hovering ? hoverShiftY : 0);

  // Labels driving the bubble text. The themed-per-skin map collapsed back
  // to a single set when the "chippy" skin was retired — kept as a derived
  // so future skin-specific overrides have a single place to land.
  // Friendly app name surfaced from Rust via `wispr:active_app` (emitted at
  // recording start). Empty string = unknown / no foreground app captured;
  // the bubble falls back to a generic "listening…" in that case.
  let activeApp = $state("");

  // Which provider is handling each stage, surfaced from Rust so the bubble
  // can read "transcribing · Groq" / "polishing · Gemini" and any stall is
  // clearly attributable. Cleared when we return to idle.
  let sttProvider = $state("");
  let llmProvider = $state("");
  function prettyProvider(name: string): string {
    if (name === "groq") return "Groq";
    if (name === "gemini") return "Gemini";
    return name;
  }

  // Seconds elapsed in the current listening state. Drives a series of
  // increasingly hammy labels — Clippy / Foxy quietly start commenting if
  // the user holds F8 forever. Resets when state leaves "listening".
  let listenElapsed = $state(0);
  $effect(() => {
    if (displayState !== "listening") return;
    listenElapsed = 0;
    const t = setInterval(() => { listenElapsed += 1; }, 1000);
    return () => clearInterval(t);
  });

  function listenLabel(secs: number, app: string): string {
    const tail = app ? ` · ${app}` : "";
    if (secs < 15)  return `listening…${tail}`;
    if (secs < 30)  return `still listening…${tail}`;
    if (secs < 45)  return `wow, you have a lot to say${tail}`;
    if (secs < 60)  return `how long is this going to go?${tail}`;
    if (secs < 90)  return `did another you grab F8?${tail}`;
    if (secs < 120) return `okay, I'll keep waiting${tail}`;
    return `marathon mode${tail}`;
  }

  let labels = $derived({
    listening: listenLabel(listenElapsed, activeApp),
    thinking: sttProvider ? `transcribing · ${prettyProvider(sttProvider)}` : "thinking",
    writing: llmProvider ? `polishing · ${prettyProvider(llmProvider)}` : "polishing",
    writingIcon: "✏️",
    pasting: "done!",
  });
</script>

<div
  class="clippy-stage"
  data-tauri-drag-region
  role="button"
  tabindex="0"
  aria-label="Clippy floater — drag to move, double-click to open main window"
  ondblclick={openMainWindow}
  onmouseenter={() => (hovering = true)}
  onmouseleave={() => { hovering = false; hoverShiftX = 0; hoverShiftY = 0; }}
  onmousemove={(e) => {
    if (!hovering) return;
    // Translate cursor position to a small pupil offset. Window inner
    // dimensions are 190x210 (set in tauri.conf.json). Clippy's centre is
    // roughly at (95, 105). Clamp to a max pupil deflection of ±3.5 SVG units.
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const dx = (e.clientX - r.left) - r.width / 2;
    const dy = (e.clientY - r.top) - r.height * 0.55; // Clippy sits below centre
    const maxDx = r.width / 2;
    const maxDy = r.height * 0.45;
    hoverShiftX = Math.max(-3.5, Math.min(3.5, (dx / maxDx) * 3.5));
    hoverShiftY = Math.max(-2.5, Math.min(2.5, (dy / maxDy) * 2.5));
  }}
>
  <!-- Toast bubble (cross-skin). Renders for transient Rust-emitted
       messages like "Copied to clipboard" — important enough that real-Clippy
       users see it too, not just the SVG skins. -->
  {#if toastMessage}
    <div class="bubble show" data-state={toastKind === "error" ? "toast-error" : "toast"}>
      <span class="bubble-text">{toastMessage}</span>
      <span class="bubble-emoji">{toastKind === "error" ? "⚠" : "📋"}</span>
    </div>
  {/if}

  <!-- Soft floor glow/shadow under Clippy — renders for ALL skins (not just
       the SVG paperclip) because it grounds the character visually. Pulses
       gently while listening to reinforce the "alive and attentive" feel. -->
  {#if skin !== "off"}
    <div class="shadow" class:pulse={displayState === "listening"}></div>
  {/if}

  {#if skin === "stylized" || skin === "fox" || skin === "duck" || skin === "cat"}

    <!-- State-driven bubble — shown for skins that don't have their own
         balloon (real Clippy has its own). Hidden while a toast is up so
         we don't stack two bubbles. Same bubble for all SVG/PNG skins so
         the dialog vocabulary feels consistent across skins. -->
    {#if !toastMessage}
      <div class="bubble" class:show={displayState !== "idle"} data-state={displayState} data-skin={skin}>
        {#if displayState === "listening"}
          <span class="bubble-text">{labels.listening}</span>
          <span class="bubble-eq"><span></span><span></span><span></span><span></span></span>
        {:else if displayState === "thinking"}
          <span class="bubble-text">{labels.thinking}</span>
          <span class="bubble-dots"><span></span><span></span><span></span></span>
        {:else if displayState === "writing"}
          <span class="bubble-text">{labels.writing}</span>
          <span class="bubble-pencil">{labels.writingIcon}</span>
        {:else if displayState === "pasting"}
          <span class="bubble-text">{labels.pasting}</span>
        {/if}
      </div>
    {/if}
  {/if}

  {#if skin === "stylized"}
    <!-- Stylized paperclip with rich state-specific animations:
         - listening: turns toward viewer, big ear pops out, alert sway
         - phew transition: brief sweat-drop right after listening ends
         - thinking advanced: brain bubble overhead
         - writing: paper slides in beside Clippy, pen scribbles
         - pasting: paper flies away, Clippy bounces. -->
    <svg
      class="character clippy-stylized"
      viewBox="-20 0 180 170"
      xmlns="http://www.w3.org/2000/svg"
      data-state={displayState}
      data-mode={mode}
      data-phew={phewActive ? "1" : "0"}
      aria-hidden="true"
    >
      <!-- ─── Paper notepad — appears when writing/pasting ─────────────── -->
      {#if displayState === "writing" || displayState === "pasting"}
        <g class="paper">
          <rect x="78" y="60" width="58" height="76" rx="3" fill="#fffaf0" stroke="#1d1d1f" stroke-width="1.5" />
          <!-- Ruled lines -->
          <line x1="84" y1="74" x2="130" y2="74" stroke="#cfd5e2" stroke-width="0.8" />
          <line x1="84" y1="82" x2="130" y2="82" stroke="#cfd5e2" stroke-width="0.8" />
          <line x1="84" y1="90" x2="130" y2="90" stroke="#cfd5e2" stroke-width="0.8" />
          <line x1="84" y1="98" x2="130" y2="98" stroke="#cfd5e2" stroke-width="0.8" />
          <line x1="84" y1="106" x2="130" y2="106" stroke="#cfd5e2" stroke-width="0.8" />
          <line x1="84" y1="114" x2="130" y2="114" stroke="#cfd5e2" stroke-width="0.8" />
          <line x1="84" y1="122" x2="130" y2="122" stroke="#cfd5e2" stroke-width="0.8" />
          <!-- Scribble strokes -->
          <path class="scribble s1" d="M 84 74 L 128 74" stroke="#1d1d1f" stroke-width="1.5" fill="none" stroke-linecap="round" />
          <path class="scribble s2" d="M 84 82 L 124 82" stroke="#1d1d1f" stroke-width="1.5" fill="none" stroke-linecap="round" />
          <path class="scribble s3" d="M 84 90 L 130 90" stroke="#1d1d1f" stroke-width="1.5" fill="none" stroke-linecap="round" />
          <path class="scribble s4" d="M 84 98 L 116 98" stroke="#1d1d1f" stroke-width="1.5" fill="none" stroke-linecap="round" />
          <path class="scribble s5" d="M 84 106 L 122 106" stroke="#1d1d1f" stroke-width="1.5" fill="none" stroke-linecap="round" />
        </g>
      {/if}

      <!-- ─── Brain bubble — advanced thinking only ─────────────────────── -->
      {#if displayState === "thinking" && mode === "advanced"}
        <g class="brain-bubble">
          <!-- Tiny trail of bubbles -->
          <circle cx="78" cy="38" r="2.5" fill="#fff" stroke="#1d1d1f" stroke-width="1" />
          <circle cx="85" cy="30" r="3.5" fill="#fff" stroke="#1d1d1f" stroke-width="1" />
          <!-- Main thought cloud -->
          <ellipse cx="100" cy="20" rx="22" ry="14" fill="#fff" stroke="#1d1d1f" stroke-width="1.6" />
          <!-- Brain icon (pink lobes) -->
          <g transform="translate(100, 20)">
            <path d="M -8 0 C -10 -5, -4 -8, 0 -5 C 4 -8, 10 -5, 8 0 C 10 4, 4 6, 0 4 C -4 6, -10 4, -8 0 Z" fill="#f5b8c8" stroke="#a44a5e" stroke-width="0.8" />
            <path d="M 0 -5 L 0 4" stroke="#a44a5e" stroke-width="0.6" />
          </g>
        </g>
      {/if}

      <!-- ─── Sweat drop (phew) — short transient between listening and next state ─ -->
      {#if phewActive}
        <g class="phew-drop">
          <path d="M 80 30 Q 78 24, 80 18 Q 82 24, 80 30 Z" fill="#7cb6ff" stroke="#1d1d1f" stroke-width="1" />
          <text x="78" y="14" font-size="6" fill="#1d1d1f" font-family="ui-sans-serif, sans-serif">phew</text>
        </g>
      {/if}

      <!-- ─── BODY GROUP — the paperclip itself, plus the listening ear ─── -->
      <g class="body-group">
        <!-- White halo for dark-background visibility -->
        <g stroke="#ffffff" stroke-width="11" fill="none" stroke-linecap="round" stroke-linejoin="round" opacity="0.95">
          <path d="M 50 30 C 50 18, 70 18, 70 30 L 70 110 C 70 132, 38 132, 38 110 L 38 50 C 38 38, 58 38, 58 50 L 58 100" />
        </g>
        <g stroke="#ffffff" stroke-width="7.5" stroke-linecap="round" fill="none" opacity="0.95">
          <path d="M 36 36 Q 42 32, 48 36" />
          <path d="M 60 36 Q 66 32, 72 36" />
        </g>

        <!-- The big LISTENING EAR — appears on left side while recording.
             Larger, more dimensional, with shading + a drop shadow to suggest
             it's leaning forward toward the user. -->
        {#if displayState === "listening"}
          <!-- Elephant ear, take 3. Previous attempt was too "leaf" — this
               one has the unmistakable elephant silhouette: wide rounded
               base meeting the head, dropping into a broad heavy fan that
               curls outward and downward to a softly-pointed tip near
               the lower-left. Inner ear opening is a clear C-shape, not
               a concentric oval. Outer rim is darker than the inner
               cartilage, like an actual elephant ear in cross-section. -->
          <g class="ear">
            <defs>
              <linearGradient id="earOuter" x1="0" y1="0" x2="1" y2="1">
                <stop offset="0%"  stop-color="#9a9a9e"/>
                <stop offset="60%" stop-color="#7a7a7e"/>
                <stop offset="100%" stop-color="#5a5a5e"/>
              </linearGradient>
              <radialGradient id="earCanal" cx="0.45" cy="0.55" r="0.6">
                <stop offset="0%"   stop-color="#4a4a4e"/>
                <stop offset="100%" stop-color="#2a2a2e"/>
              </radialGradient>
            </defs>

            <!-- ground shadow — pulses in sync with the flap -->
            <ellipse class="ear-shadow" cx="4" cy="96" rx="22" ry="3" fill="rgba(0,0,0,0.28)"/>

            <!-- the ear leaf -->
            <g class="ear-leaf">
              <!-- Outer silhouette. Heavier than a leaf:
                   - Wide attachment at the top right (x=32, y=48–62) where
                     the ear meets the head — broad and flat-ish
                   - Steep drop on the right edge (back of the ear)
                   - Bulges OUT to the left as the lobe (x=-18)
                   - Curls under at the bottom, softly pointed tip near (-12, 92)
                   - Inner edge curves back UP toward the attachment
                     with a softer convexity — the cartilage "well" -->
              <path
                d="M 32,48
                   C 28,46  20,45  10,46
                   C  -2,47 -12,54 -17,66
                   C -20,76 -16,86  -8,92
                   C   0,95  10,93  18,86
                   C  24,80 28,72  30,62
                   C  31,58 32,54  32,48
                   Z"
                fill="url(#earOuter)"
                stroke="#1d1d1f"
                stroke-width="1.8"
                stroke-linejoin="round"/>

              <!-- Inner ear canal — a curved C-shape, not a closed shape.
                   This is what reads as "elephant ear" rather than just
                   "grey blob": a visible opening near the attachment, the
                   cavity that an elephant's ear actually has. -->
              <path
                d="M 22,56
                   C 14,54  4,58  -3,65
                   C -8,72  -6,82  2,86
                   C   8,88 14,84 18,76
                   C  21,68 22,62 22,56
                   Z"
                fill="url(#earCanal)"
                opacity="0.85"/>

              <!-- A subtle cartilage ridge along the upper inner edge —
                   the part where the ear folds outward away from the head. -->
              <path
                d="M 24,52 C 16,50 6,53 -4,60"
                fill="none"
                stroke="#1d1d1f"
                stroke-width="1.2"
                stroke-linecap="round"
                opacity="0.6"/>

              <!-- Highlight on the upper rim — gives the ear a sense of
                   thickness/dimensionality. -->
              <path
                d="M 28,49 C 22,47 14,48 6,52"
                fill="none"
                stroke="#d8d8db"
                stroke-width="1.3"
                stroke-linecap="round"
                opacity="0.75"/>

              <!-- Tip detail — a slightly darker stroke at the lobe's
                   pointed end suggests the underside curling under. -->
              <path
                d="M -10,88 Q -6,93 0,92"
                fill="none"
                stroke="#1d1d1f"
                stroke-width="1"
                stroke-linecap="round"
                opacity="0.7"/>
            </g>
          </g>
        {/if}

        <!-- Paperclip body. Reverted from the v0.3.0 tick-morph experiment
             back to the simple curve path. The morph + sparkles introduced
             visual regressions for Clippy #1 — kept the v0.2.0 baseline
             intact and we'll iterate on the new fox skin separately. -->
        <g class="body" stroke="#1d1d1f" stroke-width="6" fill="none" stroke-linecap="round" stroke-linejoin="round">
          <path d="M 50 30 C 50 18, 70 18, 70 30 L 70 110 C 70 132, 38 132, 38 110 L 38 50 C 38 38, 58 38, 58 50 L 58 100" />
        </g>
        <g class="brows" stroke="#1d1d1f" stroke-width="3.5" stroke-linecap="round" fill="none">
          <path d="M 36 36 Q 42 32, 48 36" />
          <path d="M 60 36 Q 66 32, 72 36" />
        </g>
        <g class="eyes" class:hover={hovering}>
          <!-- Bigger eyes per user feedback: sclera ~doubled (was 6×7),
               pupil ~doubled (was r=2.4). Eyes also pop slightly on hover
               (CSS transform on .eyes.hover) for a "Clippy notices you"
               beat. Pupil tracks cursor in two axes when hovering, not
               just the idle look-direction sway. -->
          <ellipse cx="44" cy="51" rx="8.5" ry={blinkOpen ? 9.5 : 0.6} fill="#ffffff" stroke="#1d1d1f" stroke-width="2.2" />
          <circle cx={44 + eyeShiftX} cy={51 + eyeShiftY} r={blinkOpen ? 4 : 0} fill="#1d1d1f" />
          <!-- Tiny catchlight on each pupil — makes the eyes feel alive -->
          <circle cx={42.5 + eyeShiftX * 0.7} cy={49 + eyeShiftY * 0.7} r={blinkOpen ? 1.1 : 0} fill="#ffffff" opacity="0.95" />

          <ellipse cx="66" cy="51" rx="8.5" ry={blinkOpen ? 9.5 : 0.6} fill="#ffffff" stroke="#1d1d1f" stroke-width="2.2" />
          <circle cx={66 + eyeShiftX} cy={51 + eyeShiftY} r={blinkOpen ? 4 : 0} fill="#1d1d1f" />
          <circle cx={64.5 + eyeShiftX * 0.7} cy={49 + eyeShiftY * 0.7} r={blinkOpen ? 1.1 : 0} fill="#ffffff" opacity="0.95" />
        </g>
      </g>
    </svg>
  {:else if skin === "fox"}
    <!-- Watercolor fox — wispr-fox's own mascot (default skin in v0.4.2+).
         Renders one of the asset-pack PNGs based on the current pipeline
         state; cross-fades between them via CSS opacity stacking so state
         transitions feel soft instead of janky frame-swaps. The hover
         class triggers a curious head-tilt overlay. -->
    <div class="fox-stage" data-state={displayState} class:hover={hovering}>
      <img class="fox-layer fox-idle"      src="/fox/fox-sitting.png"     alt="" />
      <img class="fox-layer fox-listening" src="/fox/fox-recording.png"   alt="" />
      <img class="fox-layer fox-thinking"  src="/fox/fox-curious.png"     alt="" />
      <img class="fox-layer fox-writing"   src="/fox/fox-curious.png"     alt="" />
      <img class="fox-layer fox-pasting"   src="/fox/fox-success.png"     alt="" />
      <!-- Hover-only "curious" overlay sits on top of idle so even at rest
           the fox reacts when you move the cursor over the floater. -->
      <img class="fox-layer fox-hover"     src="/fox/fox-curious.png"     alt="" />
    </div>
  {:else if skin === "real-clippy"}
    <!-- The REAL Microsoft Clippy via vendored clippyts library.
         clippyts injects a div.clippy directly into document.body and
         drives sprite-based frame animations. We just kick it off; the
         actual rendering lives outside our Svelte tree. -->
    {#if realClippyLoading}
      <div class="real-msg">loading Clippy…</div>
    {:else if realClippyError}
      <div class="real-msg error">Clippy failed: {realClippyError}</div>
    {/if}

  {:else if skin === "duck"}
    <!-- ═══════════════════════════════════════════════════════════════════
         RUBBER DUCK — programmer's debugging companion.
         Yellow duck bobbing on water. States:
           idle:      gentle bob, water ripples, occasional beak quack
           listening: head tilts toward user, beak opens, bubbles rise
           thinking:  tiny glasses appear, looks up, bubbles → "?"
           writing:   wing holds pencil, notepad slides in
           pasting:   big splash, duck bounces, water droplets
         ═══════════════════════════════════════════════════════════════════ -->
    <svg
      class="character duck-skin"
      viewBox="-10 -10 160 180"
      xmlns="http://www.w3.org/2000/svg"
      data-state={displayState}
      data-mode={mode}
      aria-hidden="true"
    >
      <defs>
        <linearGradient id="duck-body-grad" x1="0" y1="0" x2="0.2" y2="1">
          <stop offset="0%" stop-color="#FFE566"/>
          <stop offset="70%" stop-color="#FFD700"/>
          <stop offset="100%" stop-color="#DAB800"/>
        </linearGradient>
        <linearGradient id="duck-head-grad" x1="0.3" y1="0" x2="0.7" y2="1">
          <stop offset="0%" stop-color="#FFF3B0"/>
          <stop offset="100%" stop-color="#FFE066"/>
        </linearGradient>
        <radialGradient id="duck-water-grad" cx="0.5" cy="0.3" r="0.7">
          <stop offset="0%" stop-color="#A8DAEF"/>
          <stop offset="100%" stop-color="#5BA8D0"/>
        </radialGradient>
        <linearGradient id="duck-beak-grad" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0%" stop-color="#FFA500"/>
          <stop offset="100%" stop-color="#E07000"/>
        </linearGradient>
      </defs>

      <!-- ─── Water layer ───────────────────────────────────────────────── -->
      <g class="duck-water">
        <ellipse cx="70" cy="152" rx="52" ry="10" fill="url(#duck-water-grad)" opacity="0.5"/>
        <path class="duck-ripple r1" d="M 25 150 Q 45 145, 65 150 Q 85 155, 105 150" fill="none" stroke="#5BA8D0" stroke-width="1.2" opacity="0.6"/>
        <path class="duck-ripple r2" d="M 30 155 Q 50 150, 70 155 Q 90 160, 110 155" fill="none" stroke="#5BA8D0" stroke-width="1" opacity="0.4"/>
      </g>

      <!-- ─── Splash droplets — pasting only ─────────────────────────── -->
      {#if displayState === "pasting"}
        <g class="duck-splash">
          <circle class="drop d1" cx="30" cy="130" r="3" fill="#A8DAEF"/>
          <circle class="drop d2" cx="50" cy="118" r="2.5" fill="#87CEEB"/>
          <circle class="drop d3" cx="95" cy="122" r="3" fill="#A8DAEF"/>
          <circle class="drop d4" cx="110" cy="132" r="2" fill="#87CEEB"/>
          <circle class="drop d5" cx="70" cy="112" r="2.5" fill="#A8DAEF"/>
        </g>
      {/if}

      <!-- ─── Bath bubbles — listening / thinking ─────────────────────── -->
      {#if displayState === "listening" || displayState === "thinking"}
        <g class="duck-bubbles">
          <circle class="bub b1" cx="30" cy="130" r="3" fill="none" stroke="#87CEEB" stroke-width="0.8" opacity="0.7"/>
          <circle class="bub b2" cx="22" cy="115" r="4" fill="none" stroke="#87CEEB" stroke-width="0.8" opacity="0.5"/>
          <circle class="bub b3" cx="35" cy="100" r="3.5" fill="none" stroke="#87CEEB" stroke-width="0.8" opacity="0.4"/>
          {#if displayState === "thinking"}
            <text class="duck-q" x="20" y="88" font-size="14" fill="#5BA8D0" font-family="ui-sans-serif, sans-serif" opacity="0.6">?</text>
          {/if}
        </g>
      {/if}

      <!-- ─── Notepad — writing / pasting ─────────────────────────────── -->
      {#if displayState === "writing" || displayState === "pasting"}
        <g class="duck-paper">
          <rect x="95" y="72" width="48" height="62" rx="3" fill="#fffaf0" stroke="#1d1d1f" stroke-width="1.2"/>
          <line x1="100" y1="84" x2="138" y2="84" stroke="#cfd5e2" stroke-width="0.7"/>
          <line x1="100" y1="92" x2="138" y2="92" stroke="#cfd5e2" stroke-width="0.7"/>
          <line x1="100" y1="100" x2="138" y2="100" stroke="#cfd5e2" stroke-width="0.7"/>
          <line x1="100" y1="108" x2="138" y2="108" stroke="#cfd5e2" stroke-width="0.7"/>
          <line x1="100" y1="116" x2="138" y2="116" stroke="#cfd5e2" stroke-width="0.7"/>
          <path class="scribble s1" d="M 100 84 L 136 84" stroke="#1d1d1f" stroke-width="1.3" fill="none" stroke-linecap="round"/>
          <path class="scribble s2" d="M 100 92 L 130 92" stroke="#1d1d1f" stroke-width="1.3" fill="none" stroke-linecap="round"/>
          <path class="scribble s3" d="M 100 100 L 134 100" stroke="#1d1d1f" stroke-width="1.3" fill="none" stroke-linecap="round"/>
          <path class="scribble s4" d="M 100 108 L 122 108" stroke="#1d1d1f" stroke-width="1.3" fill="none" stroke-linecap="round"/>
        </g>
      {/if}

      <!-- ─── Pencil in wing — writing ────────────────────────────────── -->
      {#if displayState === "writing"}
        <g class="duck-pencil-wing">
          <line x1="88" y1="105" x2="96" y2="82" stroke="#DAA520" stroke-width="3" stroke-linecap="round"/>
          <polygon points="96,82 94,78 98,78" fill="#1d1d1f"/>
          <line x1="88" y1="105" x2="90" y2="107" stroke="#FF6B6B" stroke-width="2.5" stroke-linecap="round"/>
        </g>
      {/if}

      <!-- ─── Glasses — thinking ──────────────────────────────────────── -->
      {#if displayState === "thinking"}
        <g class="duck-glasses">
          <circle cx="58" cy="56" r="9" fill="none" stroke="#4a4a4e" stroke-width="1.8"/>
          <circle cx="82" cy="56" r="9" fill="none" stroke="#4a4a4e" stroke-width="1.8"/>
          <path d="M 67 56 L 73 56" stroke="#4a4a4e" stroke-width="1.5"/>
          <!-- Lens glare -->
          <path d="M 53 51 Q 55 49, 57 51" fill="none" stroke="#fff" stroke-width="0.8" opacity="0.6"/>
          <path d="M 77 51 Q 79 49, 81 51" fill="none" stroke="#fff" stroke-width="0.8" opacity="0.6"/>
        </g>
      {/if}

      <!-- ─── Body (front-facing, symmetric) ──────────────────────────── -->
      <g class="duck-body-group">
        <!-- White halo for dark backgrounds -->
        <ellipse cx="70" cy="118" rx="42" ry="36" fill="none" stroke="#ffffff" stroke-width="8" opacity="0.9"/>

        <!-- Main body — pear shape (narrower at top, fuller at base) -->
        <path
          d="M 70 88
             C 102 88, 112 112, 108 138
             C 104 158, 36 158, 32 138
             C 28 112, 38 88, 70 88 Z"
          fill="url(#duck-body-grad)" stroke="#D4A800" stroke-width="1.5"
        />

        <!-- Belly highlight (centered, symmetric) -->
        <ellipse cx="70" cy="132" rx="26" ry="14" fill="#FFF3B0" opacity="0.5"/>

        <!-- Wings — symmetric, one each side -->
        <path d="M 38 108 C 28 118, 30 138, 44 138 C 52 134, 48 110, 38 108 Z"
              fill="#ECBF00" stroke="#D4A800" stroke-width="1" opacity="0.9"/>
        <path d="M 102 108 C 112 118, 110 138, 96 138 C 88 134, 92 110, 102 108 Z"
              fill="#ECBF00" stroke="#D4A800" stroke-width="1" opacity="0.9"/>

        <!-- Head — centered, oval (wider than tall = duck-like) -->
        <ellipse cx="70" cy="58" rx="30" ry="28" fill="url(#duck-head-grad)" stroke="#D4A800" stroke-width="1.2"/>

        <!-- Head highlight (subtle, centered top) -->
        <ellipse cx="62" cy="46" rx="11" ry="8" fill="#FFF8D0" opacity="0.4"/>

        <!-- Beak — CENTERED below eyes, front-facing flat oval -->
        <g class="duck-beak">
          <!-- Upper bill -->
          <path d="M 54 72 Q 70 64, 86 72 Q 70 78, 54 72 Z"
                fill="url(#duck-beak-grad)" stroke="#CC7000" stroke-width="1"/>
          <!-- Lower bill -->
          <path d="M 56 74 Q 70 78, 84 74 Q 70 82, 56 74 Z"
                fill="#E07000" stroke="#CC7000" stroke-width="0.9" opacity="0.95"/>
          <!-- Bill seam -->
          <line x1="56" y1="73" x2="84" y2="73" stroke="#CC7000" stroke-width="0.6" opacity="0.7"/>
          <!-- Nostrils -->
          <circle cx="65" cy="69" r="0.9" fill="#8B4513"/>
          <circle cx="75" cy="69" r="0.9" fill="#8B4513"/>
        </g>

        <!-- Eyes — flanking the beak, perfectly symmetric -->
        <g class="duck-eyes" class:hover={hovering}>
          <ellipse cx="58" cy="56" rx="6" ry={blinkOpen ? 7 : 0.5} fill="#ffffff" stroke="#1d1d1f" stroke-width="1.5"/>
          <circle cx={58 + eyeShiftX * 0.8} cy={56 + eyeShiftY * 0.8} r={blinkOpen ? 3.5 : 0} fill="#1d1d1f"/>
          <circle cx={56.5 + eyeShiftX * 0.5} cy={54 + eyeShiftY * 0.5} r={blinkOpen ? 1 : 0} fill="#ffffff" opacity="0.9"/>

          <ellipse cx="82" cy="56" rx="6" ry={blinkOpen ? 7 : 0.5} fill="#ffffff" stroke="#1d1d1f" stroke-width="1.5"/>
          <circle cx={82 + eyeShiftX * 0.8} cy={56 + eyeShiftY * 0.8} r={blinkOpen ? 3.5 : 0} fill="#1d1d1f"/>
          <circle cx={80.5 + eyeShiftX * 0.5} cy={54 + eyeShiftY * 0.5} r={blinkOpen ? 1 : 0} fill="#ffffff" opacity="0.9"/>
        </g>

        <!-- Cheek blush (symmetric, both sides) -->
        <ellipse cx="44" cy="64" rx="5" ry="3" fill="#FFB6C1" opacity="0.4"/>
        <ellipse cx="96" cy="64" rx="5" ry="3" fill="#FFB6C1" opacity="0.4"/>
      </g>

      <!-- ─── Phew drop ──────────────────────────────────────────────── -->
      {#if phewActive}
        <g class="phew-drop">
          <path d="M 98 50 Q 96 44, 98 38 Q 100 44, 98 50 Z" fill="#7cb6ff" stroke="#1d1d1f" stroke-width="0.8"/>
          <text x="95" y="34" font-size="6" fill="#1d1d1f" font-family="ui-sans-serif, sans-serif">phew</text>
        </g>
      {/if}
    </svg>

  {:else if skin === "cat"}
    <!-- ═══════════════════════════════════════════════════════════════════
         ORANGE TABBY CAT — classic ginger with white belly + stripes.
         States:
           idle:      sleek pose, slow breathing, tail twitches
           listening: ears perk, eyes wide, sits up alert
           thinking:  paw to chin, eyes up, tail → question mark
           writing:   rapid-tap paws (typing), tail swishes
           pasting:   smug face, slow stretch, settles back
         ═══════════════════════════════════════════════════════════════════ -->
    <svg
      class="character cat-skin"
      viewBox="-10 -10 160 180"
      xmlns="http://www.w3.org/2000/svg"
      data-state={displayState}
      data-mode={mode}
      aria-hidden="true"
    >
      <defs>
        <linearGradient id="cat-body-grad" x1="0.3" y1="0" x2="0.7" y2="1">
          <stop offset="0%" stop-color="#FF9F4A"/>
          <stop offset="100%" stop-color="#D9651A"/>
        </linearGradient>
        <linearGradient id="cat-head-grad" x1="0.3" y1="0" x2="0.7" y2="1">
          <stop offset="0%" stop-color="#FFB066"/>
          <stop offset="100%" stop-color="#E07020"/>
        </linearGradient>
        <radialGradient id="cat-eye-grad" cx="0.4" cy="0.4" r="0.6">
          <stop offset="0%" stop-color="#AAFF44"/>
          <stop offset="100%" stop-color="#66CC00"/>
        </radialGradient>
      </defs>

      <!-- ─── Tail ────────────────────────────────────────────────────── -->
      <g class="cat-tail">
        {#if displayState === "thinking"}
          <!-- Question-mark tail -->
          <path d="M 108 130 C 125 120, 130 100, 118 90 C 108 82, 100 90, 110 95" fill="none" stroke="#D9651A" stroke-width="5" stroke-linecap="round"/>
          <circle cx="110" cy="100" r="2.5" fill="#D9651A"/>
          <!-- Tail stripes -->
          <path d="M 115 122 L 122 119" stroke="#A04510" stroke-width="1.4" stroke-linecap="round" opacity="0.8"/>
          <path d="M 122 110 L 128 108" stroke="#A04510" stroke-width="1.4" stroke-linecap="round" opacity="0.8"/>
        {:else}
          <path d="M 108 130 C 120 115, 125 100, 115 88 C 108 80, 98 88, 108 95" fill="none" stroke="#D9651A" stroke-width="5" stroke-linecap="round"/>
          <!-- Tail stripes -->
          <path d="M 114 122 L 122 120" stroke="#A04510" stroke-width="1.4" stroke-linecap="round" opacity="0.8"/>
          <path d="M 120 108 L 126 106" stroke="#A04510" stroke-width="1.4" stroke-linecap="round" opacity="0.8"/>
        {/if}
      </g>

      <!-- ─── Body ────────────────────────────────────────────────────── -->
      <g class="cat-body-group">
        <!-- Soft halo for dark backgrounds (lighter orange wash) -->
        <ellipse cx="65" cy="130" rx="46" ry="26" fill="none" stroke="#FFF0D8" stroke-width="6" opacity="0.55"/>

        <!-- Main body — orange tabby, slightly longer/leaner than before -->
        <ellipse cx="65" cy="130" rx="44" ry="24" fill="url(#cat-body-grad)" stroke="#A04510" stroke-width="1.2"/>

        <!-- White belly patch (classic orange-tabby chest + tummy) -->
        <ellipse cx="58" cy="138" rx="22" ry="13" fill="#FFFAF0" opacity="0.95"/>
        <path d="M 50 122 Q 58 118, 66 122 Q 72 130, 66 142 Q 58 146, 50 142 Q 44 132, 50 122 Z" fill="#FFFAF0" opacity="0.85"/>

        <!-- Tabby stripes (darker orange, classic mackerel pattern) -->
        <g class="cat-stripes" opacity="0.7">
          <path d="M 78 116 Q 85 122, 88 130" stroke="#A04510" stroke-width="2" fill="none" stroke-linecap="round"/>
          <path d="M 92 118 Q 99 124, 102 134" stroke="#A04510" stroke-width="2" fill="none" stroke-linecap="round"/>
          <path d="M 100 128 Q 105 134, 106 142" stroke="#A04510" stroke-width="1.8" fill="none" stroke-linecap="round"/>
          <path d="M 28 120 Q 22 128, 24 138" stroke="#A04510" stroke-width="1.8" fill="none" stroke-linecap="round" opacity="0.5"/>
        </g>

        <!-- Front paws (white, like classic orange-and-white tabby socks) -->
        <g class="cat-paws">
          {#if displayState === "writing"}
            <!-- Typing paws — alternating left/right tap -->
            <g class="paw-left-tap">
              <ellipse cx="42" cy="148" rx="8" ry="5" fill="#FFFAF0" stroke="#A04510" stroke-width="0.8"/>
              <path d="M 36 146 L 36 143 M 39 145 L 39 142 M 42 145 L 42 142" stroke="#A04510" stroke-width="0.8" stroke-linecap="round"/>
            </g>
            <g class="paw-right-tap">
              <ellipse cx="82" cy="148" rx="8" ry="5" fill="#FFFAF0" stroke="#A04510" stroke-width="0.8"/>
              <path d="M 79 145 L 79 142 M 82 145 L 82 142 M 85 146 L 85 143" stroke="#A04510" stroke-width="0.8" stroke-linecap="round"/>
            </g>
          {:else if displayState === "thinking"}
            <!-- Paw to chin -->
            <ellipse cx="42" cy="148" rx="8" ry="5" fill="#FFFAF0" stroke="#A04510" stroke-width="0.8"/>
            <g class="paw-chin">
              <ellipse cx="78" cy="108" rx="6" ry="5" fill="#FFFAF0" stroke="#A04510" stroke-width="0.8"/>
            </g>
          {:else}
            <ellipse cx="42" cy="148" rx="8" ry="5" fill="#FFFAF0" stroke="#A04510" stroke-width="0.8"/>
            <ellipse cx="82" cy="148" rx="8" ry="5" fill="#FFFAF0" stroke="#A04510" stroke-width="0.8"/>
          {/if}
        </g>
      </g>

      <!-- ─── Head ────────────────────────────────────────────────────── -->
      <g class="cat-head-group">
        <!-- Neck -->
        <rect x="50" y="95" width="30" height="20" rx="8" fill="#E07020"/>

        <!-- Head circle (orange tabby) -->
        <circle cx="65" cy="85" r="28" fill="url(#cat-head-grad)" stroke="#A04510" stroke-width="1"/>

        <!-- Head highlight -->
        <ellipse cx="58" cy="76" rx="12" ry="8" fill="#FFD9A0" opacity="0.4"/>

        <!-- Tabby forehead stripes (the classic ginger "M") -->
        <g class="cat-forehead-stripes" opacity="0.75">
          <path d="M 56 68 L 60 76" stroke="#A04510" stroke-width="1.8" stroke-linecap="round"/>
          <path d="M 65 66 L 65 74" stroke="#A04510" stroke-width="1.8" stroke-linecap="round"/>
          <path d="M 74 68 L 70 76" stroke="#A04510" stroke-width="1.8" stroke-linecap="round"/>
        </g>

        <!-- Side cheek stripes -->
        <path d="M 40 84 L 47 86" stroke="#A04510" stroke-width="1.5" stroke-linecap="round" opacity="0.7"/>
        <path d="M 83 86 L 90 84" stroke="#A04510" stroke-width="1.5" stroke-linecap="round" opacity="0.7"/>

        <!-- White muzzle / chin patch -->
        <ellipse cx="65" cy="98" rx="13" ry="8" fill="#FFFAF0" opacity="0.95"/>

        <!-- Ears -->
        <g class="cat-ears">
          <!-- Left ear -->
          <path d="M 40 72 L 32 42 L 50 64 Z" fill="url(#cat-head-grad)" stroke="#A04510" stroke-width="1"/>
          <path d="M 42 68 L 36 50 L 48 64 Z" fill="#FF9999" opacity="0.6"/>
          <!-- Right ear -->
          <path d="M 90 72 L 98 42 L 80 64 Z" fill="url(#cat-head-grad)" stroke="#A04510" stroke-width="1"/>
          <path d="M 88 68 L 94 50 L 82 64 Z" fill="#FF9999" opacity="0.6"/>
        </g>

        <!-- Eyes -->
        <g class="cat-eyes" class:hover={hovering}>
          <!-- Left eye -->
          <ellipse cx="52" cy="82" rx="8" ry={blinkOpen ? 8.5 : 0.6} fill="url(#cat-eye-grad)" stroke="#1a1a1a" stroke-width="1.5"/>
          <!-- Slit pupil — widens on hover -->
          <ellipse cx={52 + eyeShiftX * 0.7} cy={82 + eyeShiftY * 0.5} rx={hovering ? 2.5 : 1.2} ry={blinkOpen ? 6.5 : 0} fill="#111111"/>
          <!-- Catchlight -->
          <circle cx={50 + eyeShiftX * 0.4} cy={79 + eyeShiftY * 0.3} r={blinkOpen ? 1.5 : 0} fill="#ffffff" opacity="0.85"/>

          <!-- Right eye -->
          <ellipse cx="78" cy="82" rx="8" ry={blinkOpen ? 8.5 : 0.6} fill="url(#cat-eye-grad)" stroke="#1a1a1a" stroke-width="1.5"/>
          <ellipse cx={78 + eyeShiftX * 0.7} cy={82 + eyeShiftY * 0.5} rx={hovering ? 2.5 : 1.2} ry={blinkOpen ? 6.5 : 0} fill="#111111"/>
          <circle cx={76 + eyeShiftX * 0.4} cy={79 + eyeShiftY * 0.3} r={blinkOpen ? 1.5 : 0} fill="#ffffff" opacity="0.85"/>
        </g>

        <!-- Nose (pink, on white muzzle) -->
        <path d="M 62 92 L 65 96 L 68 92 Z" fill="#FF6B6B" stroke="#cc4444" stroke-width="0.5"/>

        <!-- Mouth (subtle on the white muzzle) -->
        <path d="M 60 98 Q 65 101, 70 98" fill="none" stroke="#A04510" stroke-width="0.9" stroke-linecap="round"/>
        {#if displayState === "pasting"}
          <!-- Smug grin -->
          <path d="M 58 98 Q 65 104, 72 98" fill="none" stroke="#A04510" stroke-width="1.3" stroke-linecap="round"/>
        {/if}

        <!-- Whiskers (light cream, contrast against orange) -->
        <g class="cat-whiskers">
          <line x1="25" y1="88" x2="47" y2="90" stroke="#FFF8E0" stroke-width="0.8" stroke-linecap="round" opacity="0.9"/>
          <line x1="24" y1="94" x2="47" y2="93" stroke="#FFF8E0" stroke-width="0.8" stroke-linecap="round" opacity="0.9"/>
          <line x1="26" y1="100" x2="47" y2="96" stroke="#FFF8E0" stroke-width="0.8" stroke-linecap="round" opacity="0.9"/>
          <line x1="83" y1="90" x2="105" y2="88" stroke="#FFF8E0" stroke-width="0.8" stroke-linecap="round" opacity="0.9"/>
          <line x1="83" y1="93" x2="106" y2="94" stroke="#FFF8E0" stroke-width="0.8" stroke-linecap="round" opacity="0.9"/>
          <line x1="83" y1="96" x2="104" y2="100" stroke="#FFF8E0" stroke-width="0.8" stroke-linecap="round" opacity="0.9"/>
        </g>
      </g>

      <!-- ─── Phew drop ──────────────────────────────────────────────── -->
      {#if phewActive}
        <g class="phew-drop">
          <path d="M 100 65 Q 98 59, 100 53 Q 102 59, 100 65 Z" fill="#7cb6ff" stroke="#1d1d1f" stroke-width="0.8"/>
          <text x="97" y="49" font-size="6" fill="#1d1d1f" font-family="ui-sans-serif, sans-serif">phew</text>
        </g>
      {/if}
    </svg>
  {/if}
</div>

<style>
  :global(html), :global(body) {
    margin: 0;
    padding: 0;
    background: transparent !important;
    overflow: hidden;
    user-select: none;
    -webkit-user-select: none;
  }

  /* clippyts injects a div.clippy + div.clippy-balloon into document.body.
     Override its default position:fixed so it sits inside our floating window. */
  :global(body > .clippy) {
    position: absolute !important;
    bottom: 14px !important;
    left: 50% !important;
    transform: translateX(-50%) !important;
    pointer-events: none !important;
    z-index: 1;
  }
  :global(body > .clippy-balloon) {
    z-index: 100;
  }

  /* ─── Fox skin (watercolor PNG cross-fade) ────────────────────────────
     Five stacked layers (one per state) all positioned in the same place
     inside .fox-stage; opacity is toggled based on [data-state] and the
     .hover class on the stage. Soft cross-fade timing makes transitions
     feel painterly, not janky. The idle layer is the only one breathing
     by default; other layers freeze when not visible so the fox doesn't
     wiggle weirdly behind itself. */
  .fox-stage {
    position: relative;
    width: 100%;
    height: 100%;
    display: flex;
    align-items: flex-end;
    justify-content: center;
    pointer-events: none;
    filter: drop-shadow(0 6px 12px rgba(120, 80, 30, 0.18));
  }
  .fox-layer {
    position: absolute;
    bottom: 6px;
    left: 50%;
    transform: translateX(-50%);
    width: 116px;
    height: 116px;
    object-fit: contain;
    opacity: 0;
    transition: opacity 240ms ease;
  }
  /* Default: idle visible. */
  .fox-stage[data-state="idle"] .fox-idle {
    opacity: 1;
    animation: fox-idle-breathe 3.6s ease-in-out infinite;
  }
  .fox-stage[data-state="listening"] .fox-listening {
    opacity: 1;
    animation: fox-listen-perk 1.4s ease-in-out infinite;
  }
  .fox-stage[data-state="thinking"] .fox-thinking {
    opacity: 1;
    animation: fox-think-tilt 2s ease-in-out infinite;
  }
  .fox-stage[data-state="writing"] .fox-writing {
    opacity: 1;
    animation: fox-think-tilt 1.4s ease-in-out infinite;
  }
  .fox-stage[data-state="pasting"] .fox-pasting {
    opacity: 1;
    animation: fox-paste-bounce 0.6s cubic-bezier(0.34, 1.56, 0.64, 1) both;
  }
  /* Hover state — only when idle. When the cursor is over the floater
     and Clippy is otherwise sitting calmly, show the curious head-tilt. */
  .fox-stage.hover[data-state="idle"] .fox-idle  { opacity: 0; }
  .fox-stage.hover[data-state="idle"] .fox-hover { opacity: 1; animation: fox-hover-tilt 2.2s ease-in-out infinite; }

  @keyframes fox-idle-breathe {
    0%, 100% { transform: translateX(-50%) translateY(0)   scale(1); }
    50%      { transform: translateX(-50%) translateY(-2px) scale(1.015); }
  }
  @keyframes fox-listen-perk {
    0%, 100% { transform: translateX(-50%) rotate(-1deg) translateY(0); }
    50%      { transform: translateX(-50%) rotate(2deg)  translateY(-3px); }
  }
  @keyframes fox-think-tilt {
    0%, 100% { transform: translateX(-50%) rotate(-3deg); }
    50%      { transform: translateX(-50%) rotate(3deg); }
  }
  @keyframes fox-paste-bounce {
    0%   { transform: translateX(-50%) translateY(0)    scale(1); }
    40%  { transform: translateX(-50%) translateY(-10px) scale(1.08, 0.94); }
    100% { transform: translateX(-50%) translateY(0)    scale(1); }
  }
  @keyframes fox-hover-tilt {
    0%, 100% { transform: translateX(-50%) rotate(-2deg) translateY(0); }
    50%      { transform: translateX(-50%) rotate(2deg)  translateY(-1px); }
  }

  .real-msg {
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    background: rgba(255, 255, 255, 0.92);
    border: 1px solid rgba(0, 0, 0, 0.1);
    padding: 6px 12px;
    border-radius: 14px;
    font-size: 11px;
    color: #6e6e73;
    pointer-events: none;
    font-family: ui-sans-serif, system-ui, -apple-system, sans-serif;
  }
  .real-msg.error {
    color: #b3261e;
    border-color: #ffd7d0;
    background: #fff3f0;
  }

  .clippy-stage {
    position: relative;
    width: 100vw;
    height: 100vh;
    display: flex;
    align-items: flex-end;
    justify-content: center;
    padding-bottom: 14px;
    cursor: grab;
  }

  .clippy-stage:active {
    cursor: grabbing;
  }

  /* Floor shadow */
  .shadow {
    position: absolute;
    bottom: 6px;
    left: 50%;
    transform: translateX(-50%);
    width: 70px;
    height: 8px;
    background: radial-gradient(ellipse at center, rgba(0, 0, 0, 0.18), rgba(0, 0, 0, 0));
    border-radius: 50%;
    transition: width 200ms ease, opacity 200ms ease;
    pointer-events: none;
  }

  .shadow.pulse {
    animation: shadow-pulse 1.4s ease-in-out infinite;
  }

  @keyframes shadow-pulse {
    0%, 100% { width: 70px; opacity: 1; }
    50% { width: 56px; opacity: 0.6; }
  }

  /* Common character behaviour — shared across all three skins. */
  .character {
    overflow: visible;
    transform-origin: 50% 90%;
    animation: idle-bob 3.6s ease-in-out infinite;
    pointer-events: none;
  }

  .clippy-stylized {
    width: 150px;
    height: 150px;
    filter: drop-shadow(0 1px 2px rgba(0, 0, 0, 0.25))
            drop-shadow(0 0 6px rgba(255, 255, 255, 0.4));
  }

  /* "Clippy notices you" — eyes pop slightly larger when the cursor enters
     the floater window. Combined with the pupil tracking (in-script), this
     gives Clippy a clear "I see you" beat without being clingy. */
  .clippy-stylized .eyes {
    transform-origin: 55px 51px;
    transition: transform 180ms cubic-bezier(0.34, 1.56, 0.64, 1);
  }
  .clippy-stylized .eyes.hover {
    transform: scale(1.12);
  }

  /* (v0.3.0 click-giggle removed per user feedback — Clippy #1 stays the
     v0.2.0 baseline. Click-driven idle animations will live on the new
     fox skin instead.) */

  /* Body turns slightly toward the user when listening — like the user said,
     "turns to me." Combined with the ear popping out of the left side. */
  .clippy-stylized .body-group {
    transform-origin: 50% 90%;
    transition: transform 280ms cubic-bezier(0.32, 1.4, 0.4, 1);
  }
  .clippy-stylized[data-state="listening"] .body-group {
    transform: rotate(-8deg) translateX(2px);
  }

  /* Elephant ear — unfurls outward like a rolled palm leaf at activation,
     then settles into a continuous gentle flap (slight rotation + tiny
     downward bob) like an elephant listening. Ground shadow pulses in
     sync to sell the motion. */
  .clippy-stylized .ear {
    transform-origin: 32px 55px;
    animation:
      ear-unfurl 440ms cubic-bezier(0.22, 1.2, 0.4, 1) both,
      ear-flap 2.2s ease-in-out 440ms infinite;
  }
  .clippy-stylized .ear .ear-leaf {
    transform-origin: 32px 55px;
    transform-box: fill-box;
  }
  .clippy-stylized .ear .ear-shadow {
    transform-origin: 6px 92px;
    animation: ear-shadow-flap 2.2s ease-in-out 440ms infinite;
  }
  /* Roll-out: starts tucked tight against body (scaleX 0), unrolls leftward */
  @keyframes ear-unfurl {
    0%   { transform: scaleX(0.05) scaleY(0.55) rotate(8deg);  opacity: 0; }
    35%  { transform: scaleX(0.55) scaleY(0.85) rotate(4deg);  opacity: 1; }
    65%  { transform: scaleX(1.08) scaleY(1.04) rotate(-3deg); opacity: 1; }
    85%  { transform: scaleX(0.97) scaleY(0.99) rotate(1deg);  opacity: 1; }
    100% { transform: scaleX(1)    scaleY(1)    rotate(0);     opacity: 1; }
  }
  /* Continuous gentle flap — slight rotation + tiny vertical bob */
  @keyframes ear-flap {
    0%, 100% { transform: rotate(-2.5deg) translate(0, 0); }
    50%      { transform: rotate(2.5deg)  translate(-1px, 1.5px); }
  }
  /* Shadow pulses subtly with the flap */
  @keyframes ear-shadow-flap {
    0%, 100% { transform: scaleX(1)    scaleY(1);   opacity: 1; }
    50%      { transform: scaleX(0.92) scaleY(0.9); opacity: 0.75; }
  }

  /* Paper slides in from the right when writing, scribbles animate after. */
  .clippy-stylized .paper {
    transform-origin: 78px 100px;
    animation: paper-slide-in 360ms cubic-bezier(0.22, 1, 0.36, 1) both;
  }
  @keyframes paper-slide-in {
    0%   { transform: translateX(40px) rotate(8deg); opacity: 0; }
    100% { transform: translateX(0) rotate(0deg);    opacity: 1; }
  }

  /* When pasting, the paper flies off to the right (toward the focused app). */
  .clippy-stylized[data-state="pasting"] .paper {
    animation: paper-fly-away 600ms cubic-bezier(0.5, 0, 0.75, 0) both;
    animation-delay: 100ms;
  }
  @keyframes paper-fly-away {
    0%   { transform: translateX(0) translateY(0) rotate(0deg); opacity: 1; }
    100% { transform: translateX(80px) translateY(-30px) rotate(-25deg); opacity: 0; }
  }

  /* Scribble lines reveal one at a time (stroke-dasharray drawing trick). */
  .clippy-stylized .scribble {
    stroke-dasharray: 60;
    stroke-dashoffset: 60;
    animation: scribble-draw 0.5s cubic-bezier(0.4, 0, 0.4, 1) forwards;
  }
  .clippy-stylized .scribble.s1 { animation-delay: 0.3s; }
  .clippy-stylized .scribble.s2 { animation-delay: 0.6s; }
  .clippy-stylized .scribble.s3 { animation-delay: 0.9s; }
  .clippy-stylized .scribble.s4 { animation-delay: 1.2s; }
  .clippy-stylized .scribble.s5 { animation-delay: 1.5s; }
  @keyframes scribble-draw {
    to { stroke-dashoffset: 0; }
  }

  /* Brain bubble — appears overhead for advanced thinking. */
  .clippy-stylized .brain-bubble {
    transform-origin: 100px 20px;
    animation: brain-pop 380ms cubic-bezier(0.34, 1.56, 0.64, 1) both;
  }
  @keyframes brain-pop {
    0%   { transform: scale(0); opacity: 0; }
    60%  { transform: scale(1.1); opacity: 1; }
    100% { transform: scale(1); opacity: 1; }
  }
  /* Subtle pulse while it sits there */
  .clippy-stylized[data-state="thinking"][data-mode="advanced"] .brain-bubble {
    animation: brain-pop 380ms cubic-bezier(0.34, 1.56, 0.64, 1) both,
               brain-pulse 1.2s ease-in-out 380ms infinite;
  }
  @keyframes brain-pulse {
    0%, 100% { transform: scale(1) translateY(0); }
    50%      { transform: scale(1.04) translateY(-1px); }
  }

  /* Phew drop — fades in, floats up, fades out (700ms total). */
  .clippy-stylized .phew-drop {
    animation: phew 700ms ease-out both;
  }
  @keyframes phew {
    0%   { transform: translateY(8px); opacity: 0; }
    25%  { transform: translateY(-2px); opacity: 1; }
    100% { transform: translateY(-22px); opacity: 0; }
  }

  @keyframes idle-bob {
    0%, 100% { transform: translateY(0) rotate(0deg); }
    50% { transform: translateY(-2px) rotate(-1deg); }
  }

  /* State-driven animations. Note: the stylized skin overrides these so
     the body-group can do its own (more deliberate) listening turn. */
  .character:not(.clippy-stylized)[data-state="listening"] {
    animation: lean-in 1s ease-in-out infinite;
  }
  /* For stylized: subtle listening bob, not a side-to-side sway. */
  .clippy-stylized[data-state="listening"] {
    animation: listen-bob 1.2s ease-in-out infinite;
  }
  @keyframes listen-bob {
    0%, 100% { transform: translateY(0); }
    50%      { transform: translateY(-2px); }
  }

  @keyframes lean-in {
    0%, 100% { transform: rotate(-2deg) translateY(-1px); }
    50% { transform: rotate(2deg) translateY(-1px); }
  }

  .character[data-state="thinking"] {
    animation: thinking-tilt 1.4s ease-in-out infinite;
  }

  .character[data-state="thinking"] .brows {
    animation: brow-pulse 0.9s ease-in-out infinite;
  }

  @keyframes thinking-tilt {
    0%, 100% { transform: rotate(0deg); }
    50% { transform: rotate(4deg); }
  }

  @keyframes brow-pulse {
    0%, 100% { transform: translateY(0); }
    50% { transform: translateY(-1.5px); }
  }

  .character[data-state="writing"] {
    animation: writing-jitter 0.5s ease-in-out infinite;
  }

  @keyframes writing-jitter {
    0%, 100% { transform: translateX(0) rotate(0deg); }
    25% { transform: translateX(-1px) rotate(-1deg); }
    50% { transform: translateX(0) rotate(0deg); }
    75% { transform: translateX(1px) rotate(1deg); }
  }

  .character[data-state="pasting"] {
    animation: bounce 0.4s ease-out;
  }

  @keyframes bounce {
    0% { transform: translateY(0) scale(1); }
    40% { transform: translateY(-12px) scale(1.05, 0.95); }
    100% { transform: translateY(0) scale(1); }
  }

  /* (v0.3.0 paperclip→checkmark morph + sparkles removed — caused
       visual regressions on the stylized skin per user feedback. Will
       resurface as part of the new fox skin in a future build.) */

  /* Speech bubble — pinned to the top of the window. */
  .bubble {
    position: absolute;
    top: 6px;
    left: 50%;
    transform: translateX(-50%) translateY(-6px) scale(0.92);
    max-width: 170px;
    background: #fff;
    border: 1px solid rgba(0, 0, 0, 0.12);
    border-radius: 14px;
    padding: 6px 11px;
    font-size: 11px;
    color: #1d1d1f;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.12);
    opacity: 0;
    pointer-events: none;
    transition: opacity 200ms ease, transform 200ms cubic-bezier(0.34, 1.56, 0.64, 1);
    display: flex;
    align-items: center;
    gap: 6px;
    font-family: ui-sans-serif, system-ui, -apple-system, "Segoe UI", sans-serif;
    z-index: 5;
  }
  /* Bubble text wraps inside the fixed-width bubble. The duration-aware
     listening copy can get long ("how long is this going to go?") — let
     it wrap to a second line rather than overflow the window. */
  .bubble-text {
    white-space: normal;
    line-height: 1.3;
    text-align: left;
    flex: 1 1 auto;
    min-width: 0;
  }

  .bubble.show {
    opacity: 1;
    transform: translateX(-50%) translateY(0) scale(1);
  }

  .bubble::after {
    content: "";
    position: absolute;
    bottom: -5px;
    left: 50%;
    transform: translateX(-50%) rotate(45deg);
    width: 8px;
    height: 8px;
    background: #fff;
    border-right: 1px solid rgba(0, 0, 0, 0.12);
    border-bottom: 1px solid rgba(0, 0, 0, 0.12);
  }

  /* Fox skin gets a warmer cream-tinted bubble + dark-brown text so the
     dialog reads as part of the Foxy palette instead of pure-white iOS-ish
     pop. Stylized skin keeps the original neutral palette. */
  .bubble[data-skin="fox"] {
    background: #faf6ec;
    color: #2b2218;
    border-color: rgba(120, 80, 30, 0.18);
    box-shadow: 0 4px 12px rgba(120, 80, 30, 0.18);
  }
  .bubble[data-skin="fox"]::after {
    background: #faf6ec;
    border-right-color: rgba(120, 80, 30, 0.18);
    border-bottom-color: rgba(120, 80, 30, 0.18);
  }

  .bubble-text { font-weight: 500; }

  .bubble-eq {
    display: inline-flex;
    align-items: flex-end;
    gap: 2px;
    height: 11px;
  }

  .bubble-eq span {
    width: 2px;
    background: #0a84ff;
    border-radius: 1px;
    animation: eq-bar 0.7s ease-in-out infinite;
  }

  .bubble-eq span:nth-child(1) { animation-delay: 0s; height: 4px; }
  .bubble-eq span:nth-child(2) { animation-delay: 0.15s; height: 8px; }
  .bubble-eq span:nth-child(3) { animation-delay: 0.30s; height: 11px; }
  .bubble-eq span:nth-child(4) { animation-delay: 0.45s; height: 6px; }

  @keyframes eq-bar {
    0%, 100% { transform: scaleY(0.4); }
    50% { transform: scaleY(1); }
  }

  .bubble-dots {
    display: inline-flex;
    gap: 2px;
  }

  .bubble-dots span {
    width: 4px;
    height: 4px;
    background: #6e6e73;
    border-radius: 50%;
    animation: dot-bounce 1.2s ease-in-out infinite;
  }

  .bubble-dots span:nth-child(2) { animation-delay: 0.15s; }
  .bubble-dots span:nth-child(3) { animation-delay: 0.30s; }

  @keyframes dot-bounce {
    0%, 60%, 100% { transform: translateY(0); opacity: 0.4; }
    30% { transform: translateY(-3px); opacity: 1; }
  }

  .bubble-pencil {
    display: inline-block;
    animation: pencil-wiggle 0.4s ease-in-out infinite;
    font-size: 11px;
  }

  .bubble-emoji {
    display: inline-block;
    font-size: 13px;
    animation: emoji-pop 220ms cubic-bezier(0.34, 1.56, 0.64, 1) both;
  }

  @keyframes emoji-pop {
    0%   { transform: scale(0); }
    60%  { transform: scale(1.2); }
    100% { transform: scale(1); }
  }

  /* Toast variant — slightly more emphatic styling so it reads as an event,
     not an ongoing state. */
  .bubble[data-state="toast"] {
    background: #1d1d1f;
    color: #fff;
    border-color: #1d1d1f;
  }
  .bubble[data-state="toast"]::after {
    background: #1d1d1f;
    border-color: #1d1d1f;
  }
  /* Error toast — red theme so failures aren't confused with neutral events. */
  .bubble[data-state="toast-error"] {
    background: #b3261e;
    color: #fff;
    border-color: #b3261e;
    max-width: 200px;
    white-space: normal;
  }
  .bubble[data-state="toast-error"]::after {
    background: #b3261e;
    border-color: #b3261e;
  }

  @keyframes pencil-wiggle {
    0%, 100% { transform: rotate(-12deg); }
    50% { transform: rotate(8deg); }
  }

  /* ═══════════════════════════════════════════════════════════════════════
     RUBBER DUCK — animations
     ═══════════════════════════════════════════════════════════════════════ */

  .duck-skin { pointer-events: none; width: 100%; height: 100%; }

  .duck-body-group {
    animation: duck-idle-bob 3.6s ease-in-out infinite;
  }
  .duck-skin[data-state="listening"] .duck-body-group {
    animation: duck-listen-tilt 1.6s ease-in-out infinite;
  }
  .duck-skin[data-state="thinking"] .duck-body-group {
    animation: duck-think-look 2s ease-in-out infinite;
  }
  .duck-skin[data-state="writing"] .duck-body-group {
    animation: duck-write-jitter 0.5s ease-in-out infinite;
  }
  .duck-skin[data-state="pasting"] .duck-body-group {
    animation: duck-splash-bounce 0.6s cubic-bezier(0.34, 1.56, 0.64, 1) both;
  }

  @keyframes duck-idle-bob {
    0%, 100% { transform: translateY(0); }
    50%      { transform: translateY(-3px); }
  }
  @keyframes duck-listen-tilt {
    0%, 100% { transform: rotate(-3deg) translateY(-1px); }
    50%      { transform: rotate(3deg) translateY(-4px); }
  }
  @keyframes duck-think-look {
    0%, 100% { transform: rotate(0deg) translateY(-1px); }
    50%      { transform: rotate(-4deg) translateY(-3px); }
  }
  @keyframes duck-write-jitter {
    0%, 100% { transform: translateX(0); }
    25%      { transform: translateX(-1px); }
    75%      { transform: translateX(1px); }
  }
  @keyframes duck-splash-bounce {
    0%   { transform: translateY(0) scale(1); }
    35%  { transform: translateY(-14px) scale(1.06, 0.94); }
    100% { transform: translateY(0) scale(1); }
  }

  /* Beak opens during listening */
  .duck-skin[data-state="listening"] .duck-beak {
    animation: duck-beak-open 0.8s ease-in-out infinite;
  }
  @keyframes duck-beak-open {
    0%, 100% { transform: scaleY(1); }
    50%      { transform: scaleY(1.3) translateY(-1px); }
  }

  /* Water ripples */
  .duck-ripple {
    animation: duck-ripple-sway 3s ease-in-out infinite;
  }
  .duck-ripple.r2 { animation-delay: 0.5s; }
  @keyframes duck-ripple-sway {
    0%, 100% { transform: translateX(0); opacity: 0.4; }
    50%      { transform: translateX(4px); opacity: 0.7; }
  }

  /* Splash pasting: ripples pulse outward */
  .duck-skin[data-state="pasting"] .duck-ripple {
    animation: duck-ripple-splash 0.6s ease-out both;
  }
  @keyframes duck-ripple-splash {
    0%   { transform: scaleX(1); opacity: 0.6; }
    100% { transform: scaleX(1.4); opacity: 0; }
  }

  /* Splash droplets fly outward */
  .duck-splash .drop {
    animation: duck-drop-fly 0.7s cubic-bezier(0.2, 0.8, 0.3, 1) both;
  }
  .drop.d1 { animation-delay: 0s; }
  .drop.d2 { animation-delay: 0.05s; }
  .drop.d3 { animation-delay: 0.1s; }
  .drop.d4 { animation-delay: 0.15s; }
  .drop.d5 { animation-delay: 0.08s; }
  @keyframes duck-drop-fly {
    0%   { transform: translateY(0) scale(1); opacity: 1; }
    100% { transform: translateY(-30px) scale(0.3); opacity: 0; }
  }

  /* Bath bubbles float up */
  .duck-bubbles .bub {
    animation: duck-bubble-rise 2.5s ease-in-out infinite;
  }
  .bub.b1 { animation-delay: 0s; }
  .bub.b2 { animation-delay: 0.6s; }
  .bub.b3 { animation-delay: 1.2s; }
  @keyframes duck-bubble-rise {
    0%   { transform: translateY(0) scale(0.6); opacity: 0; }
    30%  { opacity: 0.7; }
    100% { transform: translateY(-35px) scale(1.1); opacity: 0; }
  }

  /* Question mark floats */
  .duck-q {
    animation: duck-q-bob 1.5s ease-in-out infinite;
  }
  @keyframes duck-q-bob {
    0%, 100% { transform: translateY(0); }
    50%      { transform: translateY(-4px); }
  }

  /* Glasses slide in during thinking */
  .duck-glasses {
    animation: duck-glasses-on 0.4s cubic-bezier(0.34, 1.56, 0.64, 1) both;
  }
  @keyframes duck-glasses-on {
    0%   { transform: translateY(-12px) scale(0.8); opacity: 0; }
    100% { transform: translateY(0) scale(1); opacity: 1; }
  }

  /* Paper notepad (shared with stylized — reuses scribble keyframes) */
  .duck-paper {
    animation: duck-paper-slide 0.36s cubic-bezier(0.34, 1.56, 0.64, 1) both;
  }
  .duck-skin[data-state="pasting"] .duck-paper {
    animation: duck-paper-fly 0.6s cubic-bezier(0.5, 0, 0.75, 0) both;
  }
  @keyframes duck-paper-slide {
    0%   { transform: translateX(30px) rotate(6deg); opacity: 0; }
    100% { transform: translateX(0) rotate(0deg); opacity: 1; }
  }
  @keyframes duck-paper-fly {
    0%   { transform: translateX(0) translateY(0) rotate(0deg); opacity: 1; }
    100% { transform: translateX(60px) translateY(-20px) rotate(-20deg); opacity: 0; }
  }

  /* Pencil in wing wiggle */
  .duck-pencil-wing {
    animation: duck-pencil-scribble 0.35s ease-in-out infinite;
    transform-origin: 88px 105px;
  }
  @keyframes duck-pencil-scribble {
    0%, 100% { transform: rotate(-3deg); }
    50%      { transform: rotate(5deg); }
  }

  /* Duck bubble — light blue water theme */
  .bubble[data-skin="duck"] {
    background: #E8F4FD;
    color: #1a4a5e;
    border-color: rgba(91, 168, 208, 0.25);
    box-shadow: 0 4px 12px rgba(91, 168, 208, 0.2);
  }
  .bubble[data-skin="duck"]::after {
    background: #E8F4FD;
    border-right-color: rgba(91, 168, 208, 0.25);
    border-bottom-color: rgba(91, 168, 208, 0.25);
  }

  /* ═══════════════════════════════════════════════════════════════════════
     DESK CAT — animations
     ═══════════════════════════════════════════════════════════════════════ */

  .cat-skin { pointer-events: none; width: 100%; height: 100%; }

  /* Idle: gentle breathing + sleepy eyes */
  .cat-body-group {
    animation: cat-idle-breathe 3.8s ease-in-out infinite;
  }
  .cat-head-group {
    animation: cat-idle-breathe 3.8s ease-in-out infinite;
  }

  /* Listening: perked up, alert bob */
  .cat-skin[data-state="listening"] .cat-body-group {
    animation: cat-listen-alert 1.2s ease-in-out infinite;
  }
  .cat-skin[data-state="listening"] .cat-head-group {
    animation: cat-listen-head 1.4s ease-in-out infinite;
  }
  .cat-skin[data-state="listening"] .cat-ears {
    animation: cat-ears-perk 0.4s cubic-bezier(0.34, 1.56, 0.64, 1) both;
  }

  /* Thinking: contemplative tilt */
  .cat-skin[data-state="thinking"] .cat-body-group {
    animation: cat-idle-breathe 3s ease-in-out infinite;
  }
  .cat-skin[data-state="thinking"] .cat-head-group {
    animation: cat-think-tilt 2s ease-in-out infinite;
  }

  /* Writing: typing animation */
  .cat-skin[data-state="writing"] .cat-body-group {
    animation: cat-write-focus 0.6s ease-in-out infinite;
  }
  .cat-skin[data-state="writing"] .cat-head-group {
    animation: cat-write-focus 0.6s ease-in-out infinite;
  }
  .cat-skin[data-state="writing"] .paw-left-tap {
    animation: cat-paw-tap 0.3s ease-in-out infinite;
  }
  .cat-skin[data-state="writing"] .paw-right-tap {
    animation: cat-paw-tap 0.3s ease-in-out infinite 0.15s;
  }

  /* Pasting: smug stretch */
  .cat-skin[data-state="pasting"] .cat-body-group {
    animation: cat-stretch 0.8s cubic-bezier(0.34, 1.56, 0.64, 1) both;
  }
  .cat-skin[data-state="pasting"] .cat-head-group {
    animation: cat-smug-settle 0.6s ease-out both;
  }

  /* Tail animations */
  .cat-tail {
    animation: cat-tail-idle 4s ease-in-out infinite;
  }
  .cat-skin[data-state="listening"] .cat-tail {
    animation: cat-tail-alert 1s ease-in-out infinite;
  }
  .cat-skin[data-state="writing"] .cat-tail {
    animation: cat-tail-swish 0.8s ease-in-out infinite;
  }

  /* Whisker twitch during listening */
  .cat-skin[data-state="listening"] .cat-whiskers {
    animation: cat-whisker-twitch 0.6s ease-in-out infinite;
  }

  @keyframes cat-idle-breathe {
    0%, 100% { transform: translateY(0) scale(1); }
    50%      { transform: translateY(-1.5px) scale(1.01, 0.99); }
  }
  @keyframes cat-listen-alert {
    0%, 100% { transform: translateY(0); }
    50%      { transform: translateY(-3px); }
  }
  @keyframes cat-listen-head {
    0%, 100% { transform: translateY(0) rotate(0deg); }
    30%      { transform: translateY(-4px) rotate(-2deg); }
    70%      { transform: translateY(-3px) rotate(2deg); }
  }
  @keyframes cat-ears-perk {
    0%   { transform: scaleY(0.85) translateY(3px); }
    60%  { transform: scaleY(1.08); }
    100% { transform: scaleY(1) translateY(0); }
  }
  @keyframes cat-think-tilt {
    0%, 100% { transform: rotate(0deg) translateY(-1px); }
    50%      { transform: rotate(5deg) translateY(-3px); }
  }
  @keyframes cat-write-focus {
    0%, 100% { transform: translateY(0); }
    50%      { transform: translateY(-1px); }
  }
  @keyframes cat-paw-tap {
    0%, 100% { transform: translateY(0); }
    50%      { transform: translateY(-4px); }
  }
  @keyframes cat-stretch {
    0%   { transform: scaleX(1) scaleY(1); }
    40%  { transform: scaleX(1.04) scaleY(0.96) translateY(-2px); }
    100% { transform: scaleX(1) scaleY(1); }
  }
  @keyframes cat-smug-settle {
    0%   { transform: translateY(-4px); }
    100% { transform: translateY(0); }
  }
  @keyframes cat-tail-idle {
    0%, 100% { transform: rotate(0deg); }
    25%      { transform: rotate(3deg); }
    75%      { transform: rotate(-2deg); }
  }
  @keyframes cat-tail-alert {
    0%, 100% { transform: rotate(0deg) translateY(0); }
    50%      { transform: rotate(-5deg) translateY(-2px); }
  }
  @keyframes cat-tail-swish {
    0%, 100% { transform: rotate(0deg); }
    25%      { transform: rotate(8deg); }
    75%      { transform: rotate(-8deg); }
  }
  @keyframes cat-whisker-twitch {
    0%, 100% { transform: scaleX(1); }
    50%      { transform: scaleX(1.06); }
  }

  /* Orange-cat bubble — warm dark brown with orange accent.
     The cat is intentionally usable on dark backgrounds, so the bubble
     stays dark for contrast — but the accents pick up the orange fur
     and the green eyes for cohesion. */
  .bubble[data-skin="cat"] {
    background: #2A1A10;
    color: #FFE8D0;
    border-color: rgba(255, 159, 74, 0.3);
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.35);
  }
  .bubble[data-skin="cat"]::after {
    background: #2A1A10;
    border-right-color: rgba(255, 159, 74, 0.3);
    border-bottom-color: rgba(255, 159, 74, 0.3);
  }
  /* Override EQ bar / dots for orange bubble — orange fur, green eyes vibe */
  .bubble[data-skin="cat"] .bubble-eq span { background: #FF9F4A; }
  .bubble[data-skin="cat"] .bubble-dots span { background: #C08060; }

  /* X dismiss button was removed — double-click Clippy to open the main
     window instead. Hide via tray → Toggle Clippy. */
</style>
