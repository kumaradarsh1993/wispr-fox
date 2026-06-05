<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { LogicalPosition, PhysicalPosition } from "@tauri-apps/api/window";
  import { skinStore } from "$lib/skin-store.svelte";
  import { floaterScale, floaterDebug } from "$lib/floater-scale.svelte";
  import clippyJs from "$lib/clippyjs-vendor/clippy.js";
  import FloaterContextMenu from "$lib/FloaterContextMenu.svelte";

  // Right-click context menu state. Renders our custom app actions instead
  // of the default WebView2 / WKWebView menu (Inspect Element, etc.).
  let ctxMenuOpen = $state(false);
  let ctxMenuX = $state(0);
  let ctxMenuY = $state(0);

  function openContextMenu(e: MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    // Clamp position so the menu stays inside the floater window (190×210).
    // The menu is ~168px wide max and ~190px tall worst case (avatar pane
    // with 6 rows). Anchor to click point but pull back from the edges.
    const win = { w: window.innerWidth, h: window.innerHeight };
    const menuW = 170;
    const menuH = 200;
    ctxMenuX = Math.max(4, Math.min(win.w - menuW - 4, e.clientX));
    ctxMenuY = Math.max(4, Math.min(win.h - menuH - 4, e.clientY));
    ctxMenuOpen = true;
  }

  // Block the default browser context menu on the entire document — we never
  // want users to see Inspect Element or Reload from a right-click anywhere
  // in the floater window.
  function suppressDocCtx(e: MouseEvent) {
    e.preventDefault();
  }

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

  // Nightly.13's clickthrough/polling experiment was reverted in nightly.14.
  // The 30 Hz cursor poller + per-mousemove `set_ignore_cursor_events` toggle
  // was thrashing the window state ~60×/sec, causing visible flicker on
  // Windows (eye-tracking resetting, fast "refresh" feel) and leaving the
  // window in ignore-mode often enough on Mac that drag-to-move and right-
  // click both felt broken. The cursor_poller.rs module + set_clickthrough
  // command stay in the codebase (no harm, possible future use) but nothing
  // calls them anymore. The CSS `pointer-events: visiblePainted` on SVG
  // also stays — it's a free win for hit-testing inside our own window
  // without any thrashing cost.
  //
  // Path A from the nightly.13 plan replaces it: per-skin window sizes that
  // grow when the bubble appears and shrink back when it hides, with a
  // center-anchored resize so the avatar visually stays put.

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

  // ── Per-skin window sizing: three deliberate size-states ─────────────
  //
  // Design intent (per user, nightly.15): ONE box, no per-frame thrash.
  // The window only changes size on a genuine *size-state* change, of
  // which there are three:
  //
  //   • active  — a bubble is showing (recording / thinking / writing /
  //               pasting). Widest: must host the centred speech bubble.
  //   • idle    — at rest, no bubble. Tight box hugging the avatar so the
  //               transparent click-dead-zone around it is minimal.
  //   • dormant — no activity for DORMANT_MS. Smallest: the avatar shrinks
  //               into a calmer, smaller self and the box tucks in around
  //               it (kills almost all dead-zone while you're not using it).
  //
  // During a single dictation the box grows ONCE (idle→active) and shrinks
  // ONCE (active→idle) — not on every pipeline step (thinking/writing/
  // pasting all map to "active"). Hover changes NOTHING at the window
  // level (that was the nightly.13 flicker); it only wakes a dormant
  // floater back to idle.
  //
  // Sizes below are the "design" LOGICAL px at scale 1.0. They were
  // re-derived from each skin's actual painted bounds (the old numbers
  // were eyeballed — the paperclip's 150px art was being clipped by a
  // 110px window, and every skin reserved bubble-width even at rest,
  // which is the "too much space on the right" the user reported). The
  // active width is bubble-driven (~196) for every skin; the idle/dormant
  // widths track the art. Final values get tuned over a nightly or two.
  //
  // The resize math converts these to PHYSICAL px (Tauri's window APIs all
  // speak physical — see the Retina deep-dive in CLAUDE.md; DO NOT mix
  // logical/physical or the floater lands off-screen).
  type Size = { w: number; h: number };
  type SizeState = "dormant" | "idle" | "active";
  type SkinSizes = Record<SizeState, Size>;
  const WINDOW_SIZES: Record<string, SkinSizes> = {
    stylized:      { dormant: { w: 100, h: 104 }, idle: { w: 136, h: 144 }, active: { w: 190, h: 188 } },
    fox:           { dormant: { w: 100, h: 104 }, idle: { w: 130, h: 140 }, active: { w: 190, h: 186 } },
    "real-clippy": { dormant: { w: 104, h: 100 }, idle: { w: 126, h: 124 }, active: { w: 186, h: 168 } },
    cat:           { dormant: { w: 122, h: 134 }, idle: { w: 158, h: 178 }, active: { w: 196, h: 226 } },
    "cat-lab":     { dormant: { w: 122, h: 134 }, idle: { w: 158, h: 178 }, active: { w: 196, h: 226 } },
    // "off" never shows a visible floater so the size is moot, but keep an
    // entry so the lookup is total.
    off:           { dormant: { w: 120, h: 140 }, idle: { w: 140, h: 160 }, active: { w: 190, h: 200 } },
  };

  // Dormant-mode timer. After this much idle-with-no-hover the floater
  // settles into its smallest self. Tunable; surfaced as a setting later.
  const DORMANT_MS = 60_000;
  let dormant = $state(false);
  let dormantTimer: ReturnType<typeof setTimeout> | null = null;
  // Drive the dormant flag from activity. Re-runs when displayState or the
  // hover flag flips (NOT on mousemove — hoverShiftX/Y aren't read here, so
  // tracking the cursor never reschedules and never resizes the window).
  $effect(() => {
    const busy = displayState !== "idle" || hovering;
    if (dormantTimer) {
      clearTimeout(dormantTimer);
      dormantTimer = null;
    }
    if (busy) {
      dormant = false; // any activity wakes us immediately
    } else {
      // At rest + not hovered → start the countdown to dormant.
      dormantTimer = setTimeout(() => {
        dormant = true;
        dormantTimer = null;
      }, DORMANT_MS);
    }
    return () => {
      if (dormantTimer) {
        clearTimeout(dormantTimer);
        dormantTimer = null;
      }
    };
  });

  // The single source of truth for the window's size bucket.
  let sizeState = $derived<SizeState>(
    displayState !== "idle" ? "active" : dormant ? "dormant" : "idle",
  );
  // Avatar art shrink applied ONLY in dormant (baked into the avatar's
  // width/height via the --state-scale CSS var so it composes cleanly with
  // the transform-based bob/breathe animations instead of fighting them).
  let stateScale = $derived(sizeState === "dormant" ? 0.72 : 1);
  // User-chosen size multiplier (sticky, sidebar + settings slider).
  let fscale = $derived(floaterScale.current);

  // Debug overlay (off by default). Shows the requested vs ACTUAL window size
  // so we can tell at a glance whether setSize is taking effect — the whole
  // point of this turn's fix. dbg.targetLogical is what we asked for;
  // dbg.actualLogical is what the OS actually gave us back (outerSize ÷ sf).
  let debug = $derived(floaterDebug.current);
  let dbg = $state({
    targetW: 0,
    targetH: 0,
    actualW: 0,
    actualH: 0,
    sf: 1,
  });

  // De-dupe guard so an effect double-fire doesn't spam the backend.
  let _lastResizeKey = "";

  /**
   * Resize the floater window via a RUST command, centre-anchored. We do NOT
   * use the JS window API here: on the floater webview `outerSize()` rejects
   * (and `setSize()` silently no-ops), so the old JS path aborted before it
   * ever resized — the "got 0×0, never resizes" bug. The Rust side uses native
   * window calls and returns the ACTUAL physical size + scale factor so the
   * debug overlay can show requested-vs-actual.
   */
  async function resizeFloaterCentered(target: Size) {
    const key = `${target.w}x${target.h}`;
    if (key === _lastResizeKey) return;
    _lastResizeKey = key;
    dbg.targetW = target.w;
    dbg.targetH = target.h;
    try {
      const [aw, ah, sf] = await invoke<[number, number, number]>("resize_floater", {
        width: target.w,
        height: target.h,
        center: true,
      });
      dbg.sf = sf;
      dbg.actualW = sf ? Math.round(aw / sf) : aw;
      dbg.actualH = sf ? Math.round(ah / sf) : ah;
    } catch (e) {
      console.warn("[clippy] resize_floater failed", e);
    }
  }

  // Watch (skin, sizeState, user-scale) and apply the right physical size.
  // Re-runs only on real size-state changes — pipeline steps that all map to
  // "active" don't churn the window, and hover never touches it. The no-op
  // guard inside resizeFloaterCentered absorbs any duplicate target.
  $effect(() => {
    const sizes = WINDOW_SIZES[skin] ?? WINDOW_SIZES.fox;
    const base = sizes[sizeState];
    const target = {
      w: Math.round(base.w * fscale),
      h: Math.round(base.h * fscale),
    };
    void resizeFloaterCentered(target);
  });

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
    floaterScale.subscribe();
    floaterDebug.subscribe();
    // Belt-and-suspenders: guarantee the window is resizable so programmatic
    // setSize actually applies. A non-resizable Tauri window silently ignores
    // setSize on Windows — the root cause of "the box never changes size".
    // (tauri.conf.json also sets resizable:true now; this covers cached state.)
    getCurrentWindow().setResizable(true).catch(() => {});

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

    // Place the floater on first launch / restore saved position.
    //
    // CRITICAL BUG FIX (nightly.12, reported on M4 Pro): the previous
    // implementation mixed PHYSICAL and LOGICAL pixel coordinates.
    // `availableMonitors()`/`primaryMonitor()` return positions/sizes in
    // PHYSICAL px (i.e. multiplied by scaleFactor). `outerPosition()` also
    // returns PHYSICAL. But `setPosition(new LogicalPosition(...))` expects
    // LOGICAL px — Tauri internally converts back to physical by multiplying
    // by scaleFactor. On a 2× Retina display that's a 2× error: placing the
    // window 2× further than intended, well past the right edge of the screen.
    // The floater rendered correctly, the JS heartbeat fired (proving the
    // WKWebView was alive) — the user just couldn't see it because it was
    // *literally off the edge of the monitor*. Toggling/avatar-switching
    // didn't help because every code path kept the same broken position.
    //
    // Fix: use PhysicalPosition consistently (matches what outerPosition()
    // returns AND what availableMonitors() reports). All saved/restored values
    // stay in physical px; no scale-factor conversions needed.
    //
    // We also persist via the new persist() that always saves physical.
    (async () => {
      const win = getCurrentWindow();
      // The webview's CSS world is in logical px; the floater design is 190x210
      // logical. We need them in physical to compare against monitor bounds.
      const logicalWinW = 190;
      const logicalWinH = 210;

      const placeDefault = async () => {
        try {
          const { availableMonitors, primaryMonitor } = await import(
            "@tauri-apps/api/window"
          );
          const monitors = await availableMonitors();
          let m = monitors[0];
          try {
            const p = await primaryMonitor();
            if (p) m = p;
          } catch {
            /* fall back to monitors[0] */
          }
          if (!m) return;
          const sf = m.scaleFactor ?? 1;
          const winWPhys = Math.round(logicalWinW * sf);
          const winHPhys = Math.round(logicalWinH * sf);
          // Leave a sensible margin from the right + bottom edges, in physical px.
          const marginXPhys = Math.round(24 * sf);
          const marginYPhys = Math.round(60 * sf);
          const x = m.position.x + m.size.width - winWPhys - marginXPhys;
          const y = m.position.y + m.size.height - winHPhys - marginYPhys;
          console.info(
            "[clippy] placeDefault →",
            { x, y, sf, monitor: { pos: m.position, size: m.size } },
          );
          await win.setPosition(new PhysicalPosition(x, y));
        } catch (e) {
          console.warn("[clippy] default-position placement failed", e);
        }
      };

      const saved = localStorage.getItem("wispr.clippy.pos");
      if (!saved) {
        await placeDefault();
        return;
      }
      let parsed: { x: number; y: number };
      try {
        parsed = JSON.parse(saved);
      } catch {
        localStorage.removeItem("wispr.clippy.pos");
        await placeDefault();
        return;
      }
      try {
        const { availableMonitors, primaryMonitor } = await import(
          "@tauri-apps/api/window"
        );
        const monitors = await availableMonitors();
        // Find any monitor whose bounds (PHYSICAL) overlap with the saved
        // position (PHYSICAL) by at least a margin so a small bit of window
        // poking onto a monitor still counts.
        let probe = monitors[0];
        try {
          const p = await primaryMonitor();
          if (p) probe = p;
        } catch {/* keep monitors[0] */}
        const sf = (probe?.scaleFactor) ?? 1;
        const winWPhys = Math.round(logicalWinW * sf);
        const winHPhys = Math.round(logicalWinH * sf);
        const marginPhys = Math.round(60 * sf);
        const inside = monitors.some((mn) => {
          const left = mn.position.x;
          const top = mn.position.y;
          const right = left + mn.size.width;
          const bottom = top + mn.size.height;
          return (
            parsed.x + winWPhys - marginPhys > left &&
            parsed.x + marginPhys < right &&
            parsed.y + winHPhys - marginPhys > top &&
            parsed.y + marginPhys < bottom
          );
        });
        if (inside) {
          console.info("[clippy] restore saved (physical px) →", parsed);
          await win.setPosition(new PhysicalPosition(parsed.x, parsed.y));
        } else {
          console.warn(
            "[clippy] saved position",
            parsed,
            "is offscreen; dropping it and using default",
          );
          localStorage.removeItem("wispr.clippy.pos");
          await placeDefault();
        }
      } catch (e) {
        console.warn("[clippy] position restore failed", e);
        await placeDefault();
      }
    })();

    let posSaveTimer: ReturnType<typeof setTimeout> | undefined;
    const persist = async () => {
      try {
        const pos = await getCurrentWindow().outerPosition();
        // outerPosition() is PHYSICAL px; persist as physical and restore as
        // physical so the two sides agree. Mixing logical/physical here was
        // the M4 Pro invisible-floater bug.
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
    if (secs < 90)  return `still here whenever you're ready${tail}`;
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

<svelte:window oncontextmenu={suppressDocCtx} />

<div
  class="clippy-stage"
  data-tauri-drag-region
  data-size={sizeState}
  style="--fscale:{fscale}; --state-scale:{stateScale};"
  role="button"
  tabindex="0"
  aria-label="Clippy floater — drag to move, double-click to open main window, right-click for options"
  ondblclick={openMainWindow}
  oncontextmenu={openContextMenu}
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

  <!-- Dormant "napping" cue — a soft drifting z z z while the floater has
       receded to its smallest self. Disappears the instant anything wakes it. -->
  {#if skin !== "off" && sizeState === "dormant"}
    <div class="zzz" aria-hidden="true">z&thinsp;z&thinsp;z</div>
  {/if}

  <!-- Debug overlay (Settings → Appearance → Floater debug overlay). Draws
       the exact window bounds + a live size readout so you can SEE whether
       the box is resizing and how tightly it hugs the avatar. The "ask vs
       got" line is the tell: if they differ, setSize was rejected. -->
  {#if debug}
    <div class="dbg-frame" aria-hidden="true"></div>
    <div class="dbg-readout" aria-hidden="true">
      <div><b>{skin}</b> · {sizeState}</div>
      <div>ask {dbg.targetW}×{dbg.targetH}</div>
      <div class:dbg-bad={dbg.actualW !== dbg.targetW || dbg.actualH !== dbg.targetH}>
        got {dbg.actualW}×{dbg.actualH}
      </div>
      <div>scale {Math.round(fscale * 100)}% · sf {dbg.sf}</div>
    </div>
  {/if}

  {#if skin === "stylized" || skin === "fox" || skin === "cat" || skin === "cat-lab"}

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

  {:else if skin === "cat"}
    <!-- ═══════════════════════════════════════════════════════════════════
         DESK CAT — sleepy keyboard cat (charcoal, green eyes).
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
          <stop offset="0%" stop-color="#3D3D3D"/>
          <stop offset="100%" stop-color="#222222"/>
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
          <path d="M 108 130 C 125 120, 130 100, 118 90 C 108 82, 100 90, 110 95" fill="none" stroke="#2B2B2B" stroke-width="5" stroke-linecap="round"/>
          <circle cx="110" cy="100" r="2.5" fill="#2B2B2B"/>
        {:else}
          <path d="M 108 130 C 120 115, 125 100, 115 88 C 108 80, 98 88, 108 95" fill="none" stroke="#2B2B2B" stroke-width="5" stroke-linecap="round"/>
        {/if}
      </g>

      <!-- ─── Body ────────────────────────────────────────────────────── -->
      <g class="cat-body-group">
        <!-- White halo for dark backgrounds -->
        <ellipse cx="65" cy="128" rx="48" ry="28" fill="none" stroke="#ffffff" stroke-width="7" opacity="0.85"/>

        <!-- Main body — rounded lozenge, compact when idle -->
        <ellipse cx="65" cy="128" rx="46" ry="26" fill="url(#cat-body-grad)" stroke="#1a1a1a" stroke-width="1.2"/>

        <!-- Belly shimmer (subtle lighter patch) -->
        <ellipse cx="60" cy="132" rx="20" ry="12" fill="#444444" opacity="0.3"/>

        <!-- Front paws -->
        <g class="cat-paws">
          {#if displayState === "writing"}
            <!-- Typing paws — alternating left/right tap -->
            <g class="paw-left-tap">
              <ellipse cx="42" cy="148" rx="8" ry="5" fill="#333333" stroke="#1a1a1a" stroke-width="0.8"/>
              <path d="M 36 146 L 36 143 M 39 145 L 39 142 M 42 145 L 42 142" stroke="#1a1a1a" stroke-width="0.8" stroke-linecap="round"/>
            </g>
            <g class="paw-right-tap">
              <ellipse cx="82" cy="148" rx="8" ry="5" fill="#333333" stroke="#1a1a1a" stroke-width="0.8"/>
              <path d="M 79 145 L 79 142 M 82 145 L 82 142 M 85 146 L 85 143" stroke="#1a1a1a" stroke-width="0.8" stroke-linecap="round"/>
            </g>
          {:else if displayState === "thinking"}
            <!-- Paw to chin -->
            <ellipse cx="42" cy="148" rx="8" ry="5" fill="#333333" stroke="#1a1a1a" stroke-width="0.8"/>
            <g class="paw-chin">
              <ellipse cx="78" cy="108" rx="6" ry="5" fill="#333333" stroke="#1a1a1a" stroke-width="0.8"/>
            </g>
          {:else}
            <ellipse cx="42" cy="148" rx="8" ry="5" fill="#333333" stroke="#1a1a1a" stroke-width="0.8"/>
            <ellipse cx="82" cy="148" rx="8" ry="5" fill="#333333" stroke="#1a1a1a" stroke-width="0.8"/>
          {/if}
        </g>
      </g>

      <!-- ─── Head ────────────────────────────────────────────────────── -->
      <g class="cat-head-group">
        <!-- Neck -->
        <rect x="50" y="95" width="30" height="20" rx="8" fill="#2B2B2B"/>

        <!-- Head circle -->
        <circle cx="65" cy="85" r="28" fill="#2B2B2B" stroke="#1a1a1a" stroke-width="1"/>

        <!-- Head highlight -->
        <ellipse cx="58" cy="76" rx="12" ry="8" fill="#363636" opacity="0.5"/>

        <!-- Ears -->
        <g class="cat-ears">
          <!-- Left ear -->
          <path d="M 40 72 L 32 42 L 50 64 Z" fill="#2B2B2B" stroke="#1a1a1a" stroke-width="1"/>
          <path d="M 42 68 L 36 50 L 48 64 Z" fill="#FF9999" opacity="0.5"/>
          <!-- Right ear -->
          <path d="M 90 72 L 98 42 L 80 64 Z" fill="#2B2B2B" stroke="#1a1a1a" stroke-width="1"/>
          <path d="M 88 68 L 94 50 L 82 64 Z" fill="#FF9999" opacity="0.5"/>
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

        <!-- Nose -->
        <path d="M 63 92 L 65 96 L 67 92 Z" fill="#FF6B6B" stroke="#cc4444" stroke-width="0.5"/>

        <!-- Mouth -->
        <path d="M 60 97 Q 65 100, 70 97" fill="none" stroke="#1a1a1a" stroke-width="0.8" stroke-linecap="round"/>
        {#if displayState === "pasting"}
          <!-- Smug grin -->
          <path d="M 58 97 Q 65 103, 72 97" fill="none" stroke="#1a1a1a" stroke-width="1.2" stroke-linecap="round"/>
        {/if}

        <!-- Whiskers -->
        <g class="cat-whiskers">
          <line x1="25" y1="88" x2="47" y2="90" stroke="#666" stroke-width="0.7" stroke-linecap="round"/>
          <line x1="24" y1="94" x2="47" y2="93" stroke="#666" stroke-width="0.7" stroke-linecap="round"/>
          <line x1="26" y1="100" x2="47" y2="96" stroke="#666" stroke-width="0.7" stroke-linecap="round"/>
          <line x1="83" y1="90" x2="105" y2="88" stroke="#666" stroke-width="0.7" stroke-linecap="round"/>
          <line x1="83" y1="93" x2="106" y2="94" stroke="#666" stroke-width="0.7" stroke-linecap="round"/>
          <line x1="83" y1="96" x2="104" y2="100" stroke="#666" stroke-width="0.7" stroke-linecap="round"/>
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

  {:else if skin === "cat-lab"}
    <!-- ═══════════════════════════════════════════════════════════════════
         DESK CAT — experimental ("lab") variant.
         Same silhouette, pose, and animations as "cat"; visual refinement
         only — replace the thick white halo with thin (~1px) edge highlights
         tracing the outer silhouette, lighter accent tones on belly/paws/
         inner ears so features read on dark wallpapers, defined neck with
         a soft shoulder shadow, lighter mouth stroke, two-tone tail (white
         edge + dark fill) so it doesn't disappear into the background.
         Class chain `cat-skin cat-lab-skin` so all .cat-skin keyframes apply.
         ═══════════════════════════════════════════════════════════════════ -->
    <svg
      class="character cat-skin cat-lab-skin"
      viewBox="-10 -10 160 180"
      xmlns="http://www.w3.org/2000/svg"
      data-state={displayState}
      data-mode={mode}
      aria-hidden="true"
    >
      <defs>
        <linearGradient id="catl-body-grad" x1="0.3" y1="0" x2="0.7" y2="1">
          <stop offset="0%" stop-color="#3D3D3D"/>
          <stop offset="100%" stop-color="#222222"/>
        </linearGradient>
        <linearGradient id="catl-belly-grad" x1="0.5" y1="0" x2="0.5" y2="1">
          <stop offset="0%" stop-color="#5a5a5a" stop-opacity="0.55"/>
          <stop offset="100%" stop-color="#444" stop-opacity="0.25"/>
        </linearGradient>
        <radialGradient id="catl-eye-grad" cx="0.4" cy="0.4" r="0.6">
          <stop offset="0%" stop-color="#AAFF44"/>
          <stop offset="100%" stop-color="#66CC00"/>
        </radialGradient>
      </defs>

      <!-- ─── Tail — two-tone so it never melts into the wallpaper ─── -->
      <g class="cat-tail">
        {#if displayState === "thinking"}
          <!-- Question-mark tail: white edge underlay + dark stroke on top -->
          <path d="M 108 130 C 125 120, 130 100, 118 90 C 108 82, 100 90, 110 95" fill="none" stroke="#ffffff" stroke-width="6.6" stroke-linecap="round" opacity="0.45"/>
          <path d="M 108 130 C 125 120, 130 100, 118 90 C 108 82, 100 90, 110 95" fill="none" stroke="#2B2B2B" stroke-width="4.4" stroke-linecap="round"/>
          <circle cx="110" cy="100" r="2.8" fill="#ffffff" opacity="0.45"/>
          <circle cx="110" cy="100" r="2.2" fill="#2B2B2B"/>
        {:else}
          <path d="M 108 130 C 120 115, 125 100, 115 88 C 108 80, 98 88, 108 95" fill="none" stroke="#ffffff" stroke-width="6.6" stroke-linecap="round" opacity="0.45"/>
          <path d="M 108 130 C 120 115, 125 100, 115 88 C 108 80, 98 88, 108 95" fill="none" stroke="#2B2B2B" stroke-width="4.4" stroke-linecap="round"/>
        {/if}
      </g>

      <!-- ─── Body ────────────────────────────────────────────────────── -->
      <g class="cat-body-group">
        <!-- Main body: gradient fill + thin white edge highlight tracing
             only the silhouette (no thick halo). -->
        <ellipse cx="65" cy="128" rx="46" ry="26" fill="url(#catl-body-grad)" stroke="#ffffff" stroke-width="1.1" stroke-opacity="0.55"/>

        <!-- Belly shimmer — a softer, lighter patch that clearly reads as
             "underside" instead of disappearing into the body. -->
        <ellipse cx="60" cy="134" rx="26" ry="11" fill="url(#catl-belly-grad)"/>
        <!-- Faint highlight curve along the upper belly seam -->
        <path d="M 36 130 Q 60 142 86 130" fill="none" stroke="#ffffff" stroke-width="0.6" stroke-opacity="0.25"/>

        <!-- Shoulder shading — gives the front arms separation from the body -->
        <ellipse cx="42" cy="134" rx="9" ry="11" fill="#1a1a1a" opacity="0.35"/>
        <ellipse cx="88" cy="134" rx="9" ry="11" fill="#1a1a1a" opacity="0.35"/>

        <!-- Front paws — lighter fill (#525252) + thin white edge -->
        <g class="cat-paws">
          {#if displayState === "writing"}
            <g class="paw-left-tap">
              <ellipse cx="42" cy="148" rx="8" ry="5" fill="#525252" stroke="#ffffff" stroke-width="0.7" stroke-opacity="0.6"/>
              <path d="M 36 146 L 36 143 M 39 145 L 39 142 M 42 145 L 42 142" stroke="#0d0d0d" stroke-width="0.9" stroke-linecap="round"/>
            </g>
            <g class="paw-right-tap">
              <ellipse cx="82" cy="148" rx="8" ry="5" fill="#525252" stroke="#ffffff" stroke-width="0.7" stroke-opacity="0.6"/>
              <path d="M 79 145 L 79 142 M 82 145 L 82 142 M 85 146 L 85 143" stroke="#0d0d0d" stroke-width="0.9" stroke-linecap="round"/>
            </g>
          {:else if displayState === "thinking"}
            <ellipse cx="42" cy="148" rx="8" ry="5" fill="#525252" stroke="#ffffff" stroke-width="0.7" stroke-opacity="0.6"/>
            <g class="paw-chin">
              <ellipse cx="78" cy="108" rx="6" ry="5" fill="#525252" stroke="#ffffff" stroke-width="0.7" stroke-opacity="0.6"/>
            </g>
          {:else}
            <ellipse cx="42" cy="148" rx="8" ry="5" fill="#525252" stroke="#ffffff" stroke-width="0.7" stroke-opacity="0.6"/>
            <ellipse cx="82" cy="148" rx="8" ry="5" fill="#525252" stroke="#ffffff" stroke-width="0.7" stroke-opacity="0.6"/>
          {/if}
        </g>
      </g>

      <!-- ─── Head + neck ─────────────────────────────────────────────── -->
      <g class="cat-head-group">
        <!-- Neck — rounded with subtle gradient, anchored into the body,
             with a thin highlight along the front edge so it doesn't read
             as a hard block. -->
        <path d="M 49 96 Q 49 113 56 116 L 74 116 Q 81 113 81 96 Z" fill="#2B2B2B" stroke="#ffffff" stroke-width="0.7" stroke-opacity="0.45"/>
        <!-- Inner neck shadow — depth where the neck meets the chest -->
        <path d="M 51 110 Q 65 116 79 110" fill="none" stroke="#0a0a0a" stroke-width="1.2" stroke-opacity="0.55"/>

        <!-- Head circle with thin edge highlight -->
        <circle cx="65" cy="85" r="28" fill="url(#catl-body-grad)" stroke="#ffffff" stroke-width="1.1" stroke-opacity="0.55"/>

        <!-- Head highlight (top-left light source) -->
        <ellipse cx="56" cy="74" rx="13" ry="9" fill="#3a3a3a" opacity="0.55"/>

        <!-- Ears -->
        <g class="cat-ears">
          <path d="M 40 72 L 32 42 L 50 64 Z" fill="#2B2B2B" stroke="#ffffff" stroke-width="0.9" stroke-opacity="0.55"/>
          <path d="M 42 68 L 36 50 L 48 64 Z" fill="#FF9999" opacity="0.7"/>
          <path d="M 90 72 L 98 42 L 80 64 Z" fill="#2B2B2B" stroke="#ffffff" stroke-width="0.9" stroke-opacity="0.55"/>
          <path d="M 88 68 L 94 50 L 82 64 Z" fill="#FF9999" opacity="0.7"/>
        </g>

        <!-- Eyes -->
        <g class="cat-eyes" class:hover={hovering}>
          <ellipse cx="52" cy="82" rx="8" ry={blinkOpen ? 8.5 : 0.6} fill="url(#catl-eye-grad)" stroke="#0d0d0d" stroke-width="1.3"/>
          <ellipse cx={52 + eyeShiftX * 0.7} cy={82 + eyeShiftY * 0.5} rx={hovering ? 2.5 : 1.2} ry={blinkOpen ? 6.5 : 0} fill="#0a0a0a"/>
          <circle cx={50 + eyeShiftX * 0.4} cy={79 + eyeShiftY * 0.3} r={blinkOpen ? 1.5 : 0} fill="#ffffff" opacity="0.9"/>

          <ellipse cx="78" cy="82" rx="8" ry={blinkOpen ? 8.5 : 0.6} fill="url(#catl-eye-grad)" stroke="#0d0d0d" stroke-width="1.3"/>
          <ellipse cx={78 + eyeShiftX * 0.7} cy={82 + eyeShiftY * 0.5} rx={hovering ? 2.5 : 1.2} ry={blinkOpen ? 6.5 : 0} fill="#0a0a0a"/>
          <circle cx={76 + eyeShiftX * 0.4} cy={79 + eyeShiftY * 0.3} r={blinkOpen ? 1.5 : 0} fill="#ffffff" opacity="0.9"/>
        </g>

        <!-- Nose with thin highlight -->
        <path d="M 63 92 L 65 96 L 67 92 Z" fill="#FF6B6B" stroke="#ffffff" stroke-width="0.5" stroke-opacity="0.55"/>

        <!-- Mouth — light grey so it actually reads against the dark muzzle -->
        {#if displayState === "pasting"}
          <path d="M 58 97 Q 65 103, 72 97" fill="none" stroke="#dcdcdc" stroke-width="1.1" stroke-linecap="round"/>
        {:else}
          <!-- Y-junction under the nose, then the smile curve, both in light grey -->
          <path d="M 65 96 L 65 99" fill="none" stroke="#cfcfcf" stroke-width="0.9" stroke-linecap="round"/>
          <path d="M 59 99 Q 65 103, 71 99" fill="none" stroke="#cfcfcf" stroke-width="0.9" stroke-linecap="round"/>
        {/if}

        <!-- Whiskers — lighter so they sit clearly against the dark head -->
        <g class="cat-whiskers">
          <line x1="25" y1="88" x2="47" y2="90" stroke="#bdbdbd" stroke-width="0.7" stroke-linecap="round"/>
          <line x1="24" y1="94" x2="47" y2="93" stroke="#bdbdbd" stroke-width="0.7" stroke-linecap="round"/>
          <line x1="26" y1="100" x2="47" y2="96" stroke="#bdbdbd" stroke-width="0.7" stroke-linecap="round"/>
          <line x1="83" y1="90" x2="105" y2="88" stroke="#bdbdbd" stroke-width="0.7" stroke-linecap="round"/>
          <line x1="83" y1="93" x2="106" y2="94" stroke="#bdbdbd" stroke-width="0.7" stroke-linecap="round"/>
          <line x1="83" y1="96" x2="104" y2="100" stroke="#bdbdbd" stroke-width="0.7" stroke-linecap="round"/>
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

{#if ctxMenuOpen}
  <FloaterContextMenu
    x={ctxMenuX}
    y={ctxMenuY}
    recState={state}
    onClose={() => (ctxMenuOpen = false)}
  />
{/if}

<style>
  :global(html), :global(body) {
    margin: 0;
    padding: 0;
    background: transparent !important;
    overflow: hidden;
    user-select: none;
    -webkit-user-select: none;
  }

  /* ── Clickthrough hit-shape scoping ────────────────────────────────────
     The floater window stays catching by default, but JS dynamically toggles
     `set_ignore_cursor_events(true)` whenever the cursor leaves the avatar
     shape (or the bubble, when visible). The hit-test uses
     `document.elementFromPoint(x, y)` and walks up looking for one of the
     "catching" classes below — so the CSS here is mostly a guarantee that
     SVG avatars only intercept clicks on PAINTED pixels (not their
     transparent bounding rect). Browsers natively respect
     `pointer-events: visiblePainted` on SVG: clicks/hover only register
     where the SVG actually drew something. */
  :global(svg.character) {
    pointer-events: visiblePainted;
  }
  /* Real-Clippy sprite is a div with background-image; its rectangular
     bounds are tight against the visible sprite so a rect hit-test is fine. */
  :global(body > .clippy),
  :global(body > .clippy-balloon),
  :global(.bubble) {
    pointer-events: auto;
  }
  /* The fox-stage is a 130×150 cluster of cross-faded PNGs; clicks should
     register over the cluster but NOT over the transparent gap between
     the cluster and the window edge. */
  :global(.fox-stage) {
    pointer-events: auto;
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
    /* Size = base art × user-scale × dormant-shrink. Baked into width/height
       (not a transform) so it composes with the per-layer breathe/tilt
       transforms instead of overwriting them. */
    width: calc(116px * var(--fscale, 1) * var(--state-scale, 1));
    height: calc(116px * var(--fscale, 1) * var(--state-scale, 1));
    object-fit: contain;
    opacity: 0;
    transition: opacity 240ms ease, width 320ms ease, height 320ms ease;
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

  /* ── Dormant rest ─────────────────────────────────────────────────────
     After DORMANT_MS of no activity the avatar shrinks (via --state-scale,
     already baked into each skin's width) and dims slightly so it recedes
     until you need it. A tiny "z z z" drifts up to show it's napping, not
     gone. Any activity OR a hover wakes it instantly (back to idle size). */
  .clippy-stage[data-size="dormant"] .character,
  .clippy-stage[data-size="dormant"] .fox-stage {
    opacity: 0.78;
    transition: opacity 360ms ease;
  }
  .zzz {
    position: absolute;
    top: 6px;
    left: 56%;
    font-size: calc(12px * var(--fscale, 1));
    font-weight: 600;
    letter-spacing: 0.04em;
    color: #9aa0aa;
    text-shadow: 0 1px 2px rgba(255, 255, 255, 0.55);
    pointer-events: none;
    font-family: ui-sans-serif, system-ui, -apple-system, sans-serif;
    animation: zzz-drift 3.2s ease-in-out infinite;
  }
  @keyframes zzz-drift {
    0%   { opacity: 0; transform: translateY(5px) scale(0.9); }
    30%  { opacity: 0.9; }
    70%  { opacity: 0.9; }
    100% { opacity: 0; transform: translateY(-10px) scale(1.05); }
  }

  /* ── Debug overlay ────────────────────────────────────────────────────
     Off by default. The frame traces the exact webview (= window content)
     bounds so dead-zone is obvious; the readout shows requested vs actual
     size so a rejected setSize is visible. */
  .dbg-frame {
    position: fixed;
    inset: 0;
    border: 1px dashed rgba(255, 0, 140, 0.9);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.35);
    pointer-events: none;
    z-index: 9998;
  }
  .dbg-readout {
    position: fixed;
    top: 2px;
    left: 2px;
    z-index: 9999;
    pointer-events: none;
    font-family: ui-monospace, "SF Mono", Consolas, monospace;
    font-size: 9px;
    line-height: 1.25;
    color: #fff;
    background: rgba(0, 0, 0, 0.72);
    border-radius: 4px;
    padding: 2px 4px;
    white-space: nowrap;
  }
  .dbg-readout b { color: #ff5fb0; }
  .dbg-readout .dbg-bad { color: #ff6b6b; font-weight: 700; }

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
    /* Was a flat 150px square, which over-ran the idle window and got
       clipped left/right. Now 128px wide with height derived from the
       viewBox aspect, scaled by user-scale × dormant-shrink. */
    width: calc(128px * var(--fscale, 1) * var(--state-scale, 1));
    height: auto;
    filter: drop-shadow(0 1px 2px rgba(0, 0, 0, 0.25))
            drop-shadow(0 0 6px rgba(255, 255, 255, 0.4));
    transition: width 320ms ease;
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
     DESK CAT — animations
     ═══════════════════════════════════════════════════════════════════════ */

  .cat-skin {
    pointer-events: none;
    /* Fixed scaled box (was width/height:100%, which stretched with the
       window and grew the cat whenever the bubble appeared). Height derives
       from the viewBox aspect. */
    width: calc(150px * var(--fscale, 1) * var(--state-scale, 1));
    height: auto;
    transition: width 320ms ease;
  }

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

  /* Cat bubble — dark charcoal with green accent.
     cat-lab shares the same bubble theme (visual change is on the body only). */
  .bubble[data-skin="cat"],
  .bubble[data-skin="cat-lab"] {
    background: #2B2B2B;
    color: #e0e0e0;
    border-color: rgba(127, 255, 0, 0.2);
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
  }
  .bubble[data-skin="cat"]::after,
  .bubble[data-skin="cat-lab"]::after {
    background: #2B2B2B;
    border-right-color: rgba(127, 255, 0, 0.2);
    border-bottom-color: rgba(127, 255, 0, 0.2);
  }
  /* Override EQ bar color for dark bubble */
  .bubble[data-skin="cat"] .bubble-eq span,
  .bubble[data-skin="cat-lab"] .bubble-eq span { background: #7FFF00; }
  .bubble[data-skin="cat"] .bubble-dots span,
  .bubble[data-skin="cat-lab"] .bubble-dots span { background: #999; }

  /* X dismiss button was removed — double-click Clippy to open the main
     window instead. Hide via tray → Toggle Clippy. */
</style>
