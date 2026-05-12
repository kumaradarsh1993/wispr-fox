<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
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
  // Single-click "giggle" — a one-off bounce/wiggle animation that plays
  // when the user clicks Clippy (NOT double-click, which opens the main
  // window). Lasts ~600ms; resets if the user clicks again mid-animation.
  let clickWiggling = $state(false);
  let clickWiggleTimer: ReturnType<typeof setTimeout> | null = null;
  function playClickWiggle() {
    clickWiggling = true;
    if (clickWiggleTimer) clearTimeout(clickWiggleTimer);
    clickWiggleTimer = setTimeout(() => { clickWiggling = false; }, 600);
  }
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
      // Arm or disarm the watchdog based on whether we're in a non-idle
      // state. Each non-idle transition resets the 90s window, so a slow
      // legitimate pipeline (chunked STT + LLM) won't trip the alarm.
      if (next === "idle") {
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
      disarmWatchdog();
      clearInterval(blinkTimer);
      clearInterval(lookTimer);
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
  let labels = $derived({
    listening: "listening…",
    thinking: "thinking",
    writing: "polishing",
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
  onclick={playClickWiggle}
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

  {#if skin === "stylized" || skin === "beige"}

    <!-- State-driven bubble (SVG skins only — real Clippy uses its own
         balloon for these). Hidden while toast is showing so we don't
         stack two bubbles. -->
    {#if !toastMessage}
      <div class="bubble" class:show={displayState !== "idle"} data-state={displayState}>
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

  {#if skin === "stylized" || skin === "beige"}
    <!-- Stylized paperclip with rich state-specific animations:
         - listening: turns toward viewer, big ear pops out, alert sway
         - phew transition: brief sweat-drop right after listening ends
         - thinking advanced: brain bubble overhead
         - writing: paper slides in beside Clippy, pen scribbles
         - pasting: paper flies away, Clippy bounces.
         When skin === "beige" the same SVG renders with a cream body fill
         + warm dark-brown outline (theme inversion). All animations are
         identical because they target the same CSS classes. -->
    <svg
      class="character clippy-stylized"
      class:beige={skin === "beige"}
      class:wiggle={clickWiggling}
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

        <!-- Paperclip body. The path is declared via inline `style="d: path(...)"`
             instead of `d="..."` so CSS @keyframes can interpolate the `d`
             property to morph into a checkmark when dictation completes
             (see `.body path` rules + `paperclip-to-tick` keyframe). The
             paperclip silhouette is approximated with straight L commands
             so every keyframe shares the same M+C+5L command signature
             (required for d-interpolation). -->
        <g class="body" stroke="#1d1d1f" stroke-width="6" fill="none" stroke-linecap="round" stroke-linejoin="round">
          <path style="d: path('M 50 30 C 50 18, 70 18, 70 30 L 70 110 L 38 110 L 38 50 L 58 50 L 58 100');" />
        </g>
        <!-- "Done!" sparkles — fade in around the moment the tick is fully
             formed during the pasting animation, then fade out as the body
             morphs back. Greenish (#34c759) so it reads as success. -->
        <g class="sparkles" fill="#34c759" stroke="none" opacity="0">
          <circle class="spark spark-1" cx="85" cy="45" r="2.5" />
          <circle class="spark spark-2" cx="25" cy="65" r="2" />
          <circle class="spark spark-3" cx="60" cy="100" r="1.8" />
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

  /* Click giggle — one-off wobble triggered by a single click on Clippy
     (not double-click, which opens the main window). 600ms total: a quick
     squash, a side-to-side jiggle, then back to rest. Plays atop whatever
     state animation is already running. */
  .clippy-stylized.wiggle {
    animation: clippy-wiggle 600ms cubic-bezier(0.36, 0, 0.66, -0.56);
  }
  @keyframes clippy-wiggle {
    0%   { transform: rotate(0deg) scale(1); }
    20%  { transform: rotate(-6deg) scale(1.04, 0.96); }
    40%  { transform: rotate(5deg)  scale(0.97, 1.03); }
    60%  { transform: rotate(-3deg) scale(1.02, 0.98); }
    80%  { transform: rotate(2deg)  scale(0.99, 1.01); }
    100% { transform: rotate(0deg)  scale(1); }
  }

  /* ─── Beige skin variant ────────────────────────────────────────────
     Theme-reversed Clippy: warm cream outline + brown features instead
     of the default near-black-on-transparent. Reuses ALL the same SVG
     paths and animation classes — only the colour palette changes via
     CSS overrides (SVG inline `stroke=` attrs are overridden by CSS
     `stroke:` declarations of higher specificity). */
  .clippy-stylized.beige .body {
    stroke: #f0e3c6;
    filter: drop-shadow(0 1px 1.5px rgba(80, 50, 10, 0.35));
  }
  .clippy-stylized.beige .brows {
    stroke: #8a5a2a;
    stroke-width: 4;
  }
  .clippy-stylized.beige .eyes ellipse {
    fill: #fff9ec;
    stroke: #6b3a0e;
    stroke-width: 2.4;
  }
  .clippy-stylized.beige .eyes circle:nth-of-type(odd) {
    fill: #3a1a02;   /* darker brown pupils — warmer than pure black */
  }
  /* The halo behind the paperclip body (drawn earlier in the SVG as a
     white "stroke-width 11" wrap for visibility against dark wallpapers)
     stays white — the cream body sits ON TOP. */

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

  /* ─── Paperclip → Checkmark morph on completion ──────────────────────
     When the dictation flow finishes (data-state="pasting"), the
     paperclip's wire body morphs into a checkmark, holds for ~400ms,
     then morphs back. Implemented via CSS `d:` interpolation — only
     works because each keyframe's path uses the SAME command signature
     (M C L L L L L) so the renderer can lerp point-by-point.
     Sparkles burst alongside the tick beat. */
  .clippy-stylized[data-state="pasting"] .body path {
    animation: paperclip-to-tick 1500ms cubic-bezier(0.65, 0, 0.35, 1) both;
  }
  @keyframes paperclip-to-tick {
    0% {
      d: path('M 50 30 C 50 18, 70 18, 70 30 L 70 110 L 38 110 L 38 50 L 58 50 L 58 100');
    }
    35% {
      /* Joints loosen — the inner loop straightens, the body unfurls. */
      d: path('M 40 60 C 42 55, 50 55, 52 58 L 60 80 L 50 95 L 55 75 L 70 60 L 75 55');
    }
    50%, 75% {
      /* Clean tick. Last three points repeat to keep the segment count
         at 6 (matching the paperclip's command signature). */
      d: path('M 32 72 C 34 76, 38 80, 40 82 L 48 92 L 48 92 L 78 52 L 78 52 L 78 52');
    }
    100% {
      d: path('M 50 30 C 50 18, 70 18, 70 30 L 70 110 L 38 110 L 38 50 L 58 50 L 58 100');
    }
  }

  .clippy-stylized[data-state="pasting"] .sparkles {
    animation: sparkle-burst 1500ms ease-out both;
  }
  @keyframes sparkle-burst {
    0%, 40%   { opacity: 0; transform: scale(0.4); }
    55%, 70%  { opacity: 1; transform: scale(1.1); }
    85%, 100% { opacity: 0; transform: scale(0.8); }
  }
  .clippy-stylized .sparkles {
    transform-origin: 50px 75px;
  }
  .clippy-stylized .spark {
    transform-box: fill-box;
    transform-origin: center;
  }
  .clippy-stylized .spark-2 { animation-delay: 60ms; }
  .clippy-stylized .spark-3 { animation-delay: 120ms; }

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
    white-space: nowrap;
    font-family: ui-sans-serif, system-ui, -apple-system, "Segoe UI", sans-serif;
    z-index: 5;
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

  /* X dismiss button was removed — double-click Clippy to open the main
     window instead. Hide via tray → Toggle Clippy. */
</style>
