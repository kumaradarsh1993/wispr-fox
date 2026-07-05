<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { PhysicalPosition } from "@tauri-apps/api/window";
  import { skinStore } from "$lib/skin-store.svelte";
  import { avatarVisibility } from "$lib/avatar-visibility.svelte";
  import { isMac } from "$lib/hotkey-display";
  import { placeFloaterDefault, posKeyFor, logicalWinSize } from "$lib/floater-place";
  import { floaterScale, floaterDebug, floaterFixedBox } from "$lib/floater-scale.svelte";
  import clippyJs from "$lib/clippyjs-vendor/clippy.js";
  import FloaterContextMenu from "$lib/FloaterContextMenu.svelte";
  import RasterAvatar from "$lib/RasterAvatar.svelte";
  import { RASTER_AVATAR_ART, isRasterAvatarSkin } from "$lib/avatar-packs";

  // Right-click context menu state. Renders our custom app actions instead
  // of the default WebView2 / WKWebView menu (Inspect Element, etc.).
  let ctxMenuOpen = $state(false);
  let ctxMenuX = $state(0);
  let ctxMenuY = $state(0);

  function openContextMenu(e: MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    // Opening the menu grows the window to MENU_W×MENU_H (see the resize
    // effect) so the whole menu fits even at Small size. We can't rely on the
    // click coordinates (they're in the OLD, possibly tiny window), so anchor
    // the menu near the top-centre of the grown window where it's guaranteed
    // not to be clipped. ~168px-wide menu centred in MENU_W.
    ctxMenuX = Math.max(6, Math.round((MENU_W - 168) / 2));
    ctxMenuY = 8;
    ctxMenuOpen = true;
  }

  // Block the default browser context menu on the entire document — we never
  // want users to see Inspect Element or Reload from a right-click anywhere
  // in the floater window.
  function suppressDocCtx(e: MouseEvent) {
    e.preventDefault();
  }

  // Manual window drag with a MOVEMENT THRESHOLD. We do NOT use
  // `data-tauri-drag-region` (its double-click-to-maximize blew the
  // transparent floater up to fill the screen). But we also can't call
  // `startDragging()` straight away on mousedown: doing so enters the OS
  // move-loop immediately and SWALLOWS the subsequent double-click, so
  // "double-click the avatar → open the main window" silently stopped
  // working. Instead we arm on mousedown and only begin dragging once the
  // pointer actually moves past a few px — a plain click (or double-click)
  // never crosses the threshold, so it reaches the dblclick handler.
  let dragArmed: { x: number; y: number } | null = null;
  const DRAG_THRESHOLD_SQ = 16; // (4px)²

  function onStageMouseDown(e: MouseEvent) {
    if (e.button !== 0) return; // left button only; right opens the menu
    dragArmed = { x: e.clientX, y: e.clientY };
  }
  function maybeStartDrag(e: MouseEvent) {
    if (!dragArmed || (e.buttons & 1) === 0) return;
    const dx = e.clientX - dragArmed.x;
    const dy = e.clientY - dragArmed.y;
    if (dx * dx + dy * dy > DRAG_THRESHOLD_SQ) {
      dragArmed = null;
      getCurrentWindow().startDragging().catch(() => {});
    }
  }
  function endDrag() {
    dragArmed = null;
  }

  type ClippyState = "idle" | "listening" | "thinking" | "writing" | "pasting";
  type Mode = "light" | "advanced";

  // `flowState` is the *actual* flow state from Rust (changes fast during pipeline).
  // `displayState` is what Clippy is currently animating — it lags `flowState`
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
      flowState = "idle";
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

  let flowState = $state<ClippyState>("idle");
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
  // Idle hover quip text (set by the hover-quip effect further down; declared
  // here because the window-sizing `bubbleUp` derived reads it).
  let hoverQuip = $state("");
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
    enqueueDisplay(flowState);
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

  // macOS renders the floater on an OPAQUE window (transparency ghost-window
  // workaround), so the wave pill uses the cream card look there instead of a
  // translucent dark pill. Detected once from the user agent.
  const isMacPlatform = isMac();

  // ── Per-skin window sizing: TWO boxes per avatar (rest / talking) ─────
  //
  // v1.4.0: the v1.3.0 "ONE fixed box" model permanently reserved the
  // speech-bubble band ABOVE the avatar's head — ~110px of always-there
  // invisible window that sat on top of whatever app the user had behind
  // the floater, hiding content and eating clicks (user: "the transparent
  // background obstructs my messages"). Now each avatar has a tight REST
  // box (just the character + breathing room) and a taller TALK box (adds
  // the bubble band). The window grows upward the moment a bubble needs to
  // show and shrinks back shortly after it hides. This is safe where the
  // old dynamic-resize experiment wasn't because every piece it was missing
  // now exists: the resize is ONE atomic native SetWindowPos (no flicker),
  // bottom-centre anchored (the character never moves), and it changes only
  // on bubble visibility - never per pipeline state, never per frame.
  // v2.0.0 keeps the avatar scale uniform, but decouples the bubble scale:
  // very small avatars still need readable status text, while large avatars
  // should not get a comically large bubble.
  //
  // Each avatar's footprint + where its head is, all LOGICAL px at scale 1.0.
  //   w/h  = the rendered character size (must match the avatar CSS below).
  //   head = distance from the window BOTTOM up to where the speech bubble's
  //          tail sits — just above the character's visible head. The bubble
  //          anchors here and grows UPWARD, so even at rest it hugs the head
  //          (no big gap) and long text never reaches the face.
  // The box is derived from these so it's as TIGHT as possible while still
  // fitting the character + a 3-line bubble. Bubble-driven width keeps L/R
  // padding minimal; bottom padding is small (the shadow sits at 6px).
  type Size = { w: number; h: number };
  type Art = { w: number; h: number; head: number };
  const ART: Record<string, Art> = {
    fox:           { w: 116, h: 116, head: 110 },
    stylized:      { w: 128, h: 122, head: 120 },
    "real-clippy": { w: 118, h: 112, head: 110 },
    cat:           { w: 150, h: 168, head: 128 },
    "cat-lab":     { w: 150, h: 168, head: 128 },
    duo:           { w: 198, h: 117, head: 107 }, // two cats side by side — wide, deliberately LOW
    "duo-hd":      { w: 200, h: 130, head: 120 }, // remastered duo — a touch taller for the pounce headroom
    off:           { w: 116, h: 116, head: 110 },
    // Minimal skins — NO head (no bubble ever). head=0 collapses the two-box
    // model to a single REST box (see boxFor + isMinimalSkin).
    // Wave bar: small Apple-style pill (~Clippy width). Siri orb: tiny circle.
    wave:          { w: 132, h: 38,  head: 0 },
    siri:          { w: 58,  h: 58,  head: 0 },
    ...RASTER_AVATAR_ART,
  };
  const SIDE_PAD = 8; // L/R breathing room around the character
  const BOTTOM_PAD = 8; // gap below the character (shadow lives at 6px)
  const TOP_MARGIN = 8; // gap above the character/bubble to the window top
  // Clear air between the top of the character's head and the bottom (tail)
  // of the speech bubble. Slim (v1.4 feedback: the bubble band made the box
  // needlessly tall — pull the bubble toward the avatar).
  const HEAD_GAP = 10;
  // Vertical room above the head for the bubble itself. The bubble is now
  // HARD-CAPPED at two lines (CSS line-clamp + smaller type + wider bubble),
  // so the band only needs: 2 lines (~27px) + padding/border (~13px) +
  // TOP_MARGIN-ish slack. Was 104 for a 4–5 line bubble — that head-room is
  // exactly the vertical dead space the user flagged.
  const BUBBLE_BAND = 62;
  const BUBBLE_W = 226; // min box width - unchanged (the user explicitly does
                        // NOT want a wider window; the bubble inside it gets
                        // wider instead, see .bubble max-width).

  function bubbleScaleFor(scale: number): number {
    if (scale <= 0.6) return 1.2;
    if (scale <= 0.8) return 1.2 - ((scale - 0.6) / 0.2) * 0.15;
    if (scale <= 1.0) return 1.05 - ((scale - 0.8) / 0.2) * 0.05;
    if (scale <= 1.25) return 1.0 - ((scale - 1.0) / 0.25) * 0.125;
    return Math.max(0.86, 0.875 - (scale - 1.25) * 0.05);
  }

  function bubbleGapFor(scale: number): number {
    return Math.max(12, HEAD_GAP * scale);
  }

  // Minimal skins (wave bar, Siri orb): no bubbles, no quips, no floor
  // shadow — always the tight REST box, and their windows default to their
  // own screen positions (see lib/floater-place.ts).
  const MINIMAL_SKINS = new Set(["wave", "siri"]);
  function isMinimalSkin(s: string): boolean {
    return MINIMAL_SKINS.has(s);
  }

  function boxFor(skin: string, talking: boolean, avatarScale: number, bubbleScale: number): Size {
    const a = ART[skin] ?? ART.fox;
    const rest: Size = {
      w: Math.ceil((a.w + 2 * SIDE_PAD) * avatarScale),
      h: Math.ceil((a.h + BOTTOM_PAD + TOP_MARGIN) * avatarScale),
    };
    // Minimal skins never show a bubble — always the tight REST box regardless
    // of the `talking` flag.
    if (isMinimalSkin(skin)) return rest;
    if (!talking) return rest;
    return {
      w: Math.max(rest.w, Math.ceil(BUBBLE_W * bubbleScale)),
      h: Math.max(
        rest.h,
        Math.ceil(a.head * avatarScale + bubbleGapFor(avatarScale) + BUBBLE_BAND * bubbleScale),
      ),
    };
  }

  // A bubble is "up" when either the state bubble or a transient toast is
  // showing. Growing happens immediately (the window must be tall before the
  // bubble's 200ms fade-in paints); shrinking waits a beat so the fade-OUT
  // finishes inside the still-tall window and quick state churn (e.g.
  // thinking→writing) never causes a shrink-grow stutter.
  // Minimal skins show NO bubbles/toasts/quips ever — so they never grow the box.
  let bubbleUp = $derived(
    !isMinimalSkin(skin) &&
      (toastMessage !== "" || hoverQuip !== "" || (skin !== "off" && displayState !== "idle")),
  );
  let talking = $state(false);
  let _shrinkTimer: ReturnType<typeof setTimeout> | null = null;
  $effect(() => {
    if (bubbleUp) {
      if (_shrinkTimer) {
        clearTimeout(_shrinkTimer);
        _shrinkTimer = null;
      }
      talking = true;
    } else if (talking && !_shrinkTimer) {
      _shrinkTimer = setTimeout(() => {
        _shrinkTimer = null;
        talking = false;
      }, 350);
    }
  });

  // The right-click menu renders INSIDE this window and does NOT scale with
  // fscale, so while it's open the window must be at least this big (logical
  // px) or the menu gets cropped. Tall enough for the Avatar sub-pane (7 rows).
  const MENU_W = 192;
  const MENU_H = 316;

  // User-chosen size multiplier (sticky, sidebar + settings slider). The ONLY
  // input (besides skin + the open menu) that changes the window size now.
  let fscale = $derived(floaterScale.current);
  let bubbleScale = $derived(bubbleScaleFor(fscale));
  let bubbleGap = $derived(bubbleGapFor(fscale));

  // Where the bubble's tail sits (logical px from window bottom), scaled.
  // The bubble anchors here and grows upward. v2.0.0 keeps a minimum physical
  // gap so the larger small-size bubble no longer lands on the avatar's face.
  let bubbleBottom = $derived(((ART[skin]?.head ?? ART.fox.head) * fscale) + bubbleGap);

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

  // Stage visibility used to MASK resizes. After a native window resize the
  // webview re-rasterizes asynchronously: for a few frames the old content
  // stays at its old size anchored to the window's (moved) top-left corner,
  // so the avatar visibly teleported toward a corner, clipped, then snapped
  // back (user report on nightly.2/.3 — SWP_NOCOPYBITS alone can't fix this,
  // it's the compositor, not the blit). Hiding the stage for the resize and
  // fading it back turns the corner-glitch into a soft ~150ms blink that
  // blends into the avatar's own state cross-fade.
  let stageVisible = $state(true);
  let _maskDepth = 0;
  const nextFrame = () => new Promise<void>((r) => requestAnimationFrame(() => r()));

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
    // Skip the mask on the very first sizing after mount — the window is
    // settling into place anyway and there's nothing on screen to glitch.
    const mask = _lastResizeKey !== "";
    _lastResizeKey = key;
    dbg.targetW = target.w;
    dbg.targetH = target.h;
    if (mask) {
      _maskDepth++;
      stageVisible = false;
      // Two rAFs: one to flush the hidden state into the DOM, one to be sure
      // that frame actually reached the compositor before the window moves.
      await nextFrame();
      await nextFrame();
    }
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
    } finally {
      if (mask) {
        // Give the webview one frame to lay out at the new size, then fade
        // the stage back in (CSS handles the 140ms ease).
        await nextFrame();
        _maskDepth--;
        if (_maskDepth === 0) stageVisible = true;
      }
    }
  }

  // User preference: "Full box (classic)" pins the window at the TALK size
  // permanently — zero resizes during dictation, exactly the v1.3.0 model.
  // Surfaced in Settings → Appearance for anyone who finds the masked
  // grow/shrink transition too noticeable.
  let fixedBox = $derived(floaterFixedBox.current);

  // Apply the window size. Re-runs when the skin, the user scale, the
  // open-menu flag, the box-mode preference, or bubble visibility
  // (debounced `talking`) changes — NOT on every pipeline state hop, so
  // mid-dictation transitions (listening→thinking→writing) never touch
  // the window.
  $effect(() => {
    const box = boxFor(skin, fixedBox || talking, fscale, bubbleScale);
    let w = box.w;
    let h = box.h;
    // While the right-click menu is open, ensure the window is at least big
    // enough to show the whole (fixed-size) menu — grow only, never shrink.
    if (ctxMenuOpen) {
      w = Math.max(w, MENU_W);
      h = Math.max(h, MENU_H);
    }
    void resizeFloaterCentered({ w, h });
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
    const s = skin; // track
    if (!_skinInit) {
      _skinInit = true;
      return;
    }
    recoverFloater("skin changed");
    // On skin change, if this skin's positioning class has no saved position,
    // place it at its class default (wave → top-center; character → corner).
    // This is what makes switching TO wave land the pill at top-center the
    // first time, without stomping a saved wave/character position.
    (async () => {
      try {
        if (!localStorage.getItem(posKeyFor(s))) {
          await placeFloaterDefault(s);
        }
      } catch (e) {
        console.warn("[clippy] skin-change placement failed", e);
      }
    })();
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
    avatarVisibility.subscribe();
    floaterScale.subscribe();
    floaterDebug.subscribe();
    floaterFixedBox.subscribe();

    // Warm arrival on first mount. In `always` mode the window is already
    // shown by Rust; play the ENTER so the avatar pops in. In `auto` mode the
    // auto state machine drives enter/exit instead (and the window may be
    // hidden), so skip here.
    if (avatarVisibility.current === "always") {
      playEnter();
    }
    // Belt-and-suspenders: guarantee the window is resizable so programmatic
    // setSize actually applies. A non-resizable Tauri window silently ignores
    // setSize on Windows — the root cause of "the box never changes size".
    // (tauri.conf.json also sets resizable:true now; this covers cached state.)
    getCurrentWindow().setResizable(true).catch(() => {});
    // Forbid maximizing — double-clicking a `data-tauri-drag-region` toggles
    // maximize by default, which blew the transparent floater up to fill the
    // whole screen (and blocked every click behind it). Config sets this too.
    getCurrentWindow().setMaximizable(false).catch(() => {});

    let unlisten: (() => void) | undefined;
    let unlistenMode: (() => void) | undefined;
    let unlistenMsg: (() => void) | undefined;
    let unlistenErr: (() => void) | undefined;
    let unlistenActiveApp: (() => void) | undefined;
    let unlistenSttProv: (() => void) | undefined;
    let unlistenLlmProv: (() => void) | undefined;
    let unlistenWarn: (() => void) | undefined;
    let unlistenLevel: (() => void) | undefined;
    listen<string>("wispr:clippy_message", (e) => {
      console.log("[clippy] wispr:clippy_message", e.payload);
      showToast(e.payload, "info", 3000);
    }).then((u) => (unlistenMsg = u));
    listen<string>("wispr:flow_error", (e) => {
      console.warn("[clippy] wispr:flow_error", e.payload);
      // Force-reset all state — Rust's wrapper also emits "idle" but be
      // defensive in case events arrive out of order.
      flowState = "idle";
      displayState = "idle";
      displayQueue = [];
      if (displayTimer) {
        clearTimeout(displayTimer);
        displayTimer = null;
      }
      disarmWatchdog();
      triggerWaveError(); // red blink on the wave/siri minimal skins
      showToast(e.payload, "error", 5000);
    }).then((u) => (unlistenErr = u));
    listen<string>("wispr:state", (e) => {
      const next = mapFlow(e.payload);
      console.log("[clippy] wispr:state", e.payload, "→", next);
      flowState = next;
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
          flowState = "idle";
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
    // Live mic level for the wave-bar skin (f32 0..1, throttled while
    // recording). Cheap to keep listening on all skins; only the wave skin
    // renders it.
    listen<number>("wispr:level", (e) => {
      const v = typeof e.payload === "number" ? e.payload : Number(e.payload);
      if (!Number.isNaN(v)) pushLevel(v);
    }).then((u) => (unlistenLevel = u));

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
      // Position storage is PER SKIN-CLASS (wave vs character) so switching
      // between the top-center wave pill and a bottom-corner character doesn't
      // make them fight over one saved slot. posKeyFor()/placeFloaterDefault()
      // (lib/floater-place.ts) own the monitor math — single source of truth.
      // The wave window is short/wide; characters are ~190×210 logical.
      const curSkin = skinStore.current;
      const posKey = posKeyFor(curSkin);
      const { w: logicalWinW, h: logicalWinH } = logicalWinSize(curSkin);

      const placeDefault = async () => {
        try {
          await placeFloaterDefault(curSkin);
        } catch (e) {
          console.warn("[clippy] default-position placement failed", e);
        }
      };

      const saved = localStorage.getItem(posKey);
      if (!saved) {
        await placeDefault();
        return;
      }
      let parsed: { x: number; y: number };
      try {
        parsed = JSON.parse(saved);
      } catch {
        localStorage.removeItem(posKey);
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
          localStorage.removeItem(posKey);
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
        // the M4 Pro invisible-floater bug. Keyed by the CURRENT skin's class
        // so wave and character positions are remembered separately.
        localStorage.setItem(
          posKeyFor(skinStore.current),
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
      unlistenLevel?.();
      if (waveRaf != null) cancelAnimationFrame(waveRaf);
      cancelPendingAutoHide();
      if (_enterTimer) clearTimeout(_enterTimer);
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

  // ── duo-hd eye geometry ─────────────────────────────────────────────
  // Eye openness + pupil width for the remastered duo, hoisted out of the
  // markup because Svelte's {@const} can't live directly inside an SVG <g>.
  // Khaumani: heavy-lidded + slit pupils when calm, rounder/wider when
  // engaged. Indy (kitten): big round eyes that dilate when excited.
  let hdKhOpen = $derived(blinkOpen ? (displayState === "idle" && !hovering ? 3 : 5) : 0.5);
  let hdKhPup = $derived(displayState === "idle" || displayState === "writing" ? 0.85 : 1.7);
  let hdInOpen = $derived(blinkOpen ? 5.2 : 0.6);
  let hdInPup = $derived(
    displayState === "listening" || displayState === "pasting" || hovering ? 2.9 : 2.2,
  );

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
    if (name === "deepgram") return "Deepgram";
    if (name === "elevenlabs") return "ElevenLabs";
    if (name === "groq") return "Groq";
    if (name === "gemini") return "Gemini";
    if (name === "openai") return "OpenAI";
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

  // Duration-aware listening copy. The user's real sessions run anywhere
  // from 20 seconds to 15+ minutes, so the tiers stretch all the way out —
  // and every line is written to fit the bubble's 2-line cap. A minute
  // marker rides along once past 60s so a glance tells you how long you've
  // been going.
  function listenLabel(secs: number, app: string): string {
    const tail = app ? ` · ${app}` : "";
    const mins = Math.floor(secs / 60);
    const clock = mins >= 1 ? ` · ${mins}m` : "";
    if (secs < 15)  return `listening…${tail}`;
    if (secs < 30)  return `all ears${tail}`;
    if (secs < 45)  return `got it, keep going${tail}`;
    if (secs < 60)  return `you're on a roll${tail}`;
    if (secs < 90)  return `taking it all down${clock}${tail}`;
    if (secs < 120) return `still with you${clock}${tail}`;
    if (secs < 180) return `essay mode${clock}${tail}`;
    if (secs < 300) return `chapter incoming${clock}${tail}`;
    if (secs < 420) return `deep in the zone${clock}${tail}`;
    if (secs < 600) return `marathon mode${clock}${tail}`;
    if (secs < 900) return `novella territory${clock}${tail}`;
    return `legendary session${clock}${tail}`;
  }

  // Small rotating pools for the quick states so the floater doesn't say
  // the exact same thing hundreds of times a day. Picked once per pipeline
  // run (at recording start) so the label doesn't churn mid-state.
  const DONE_LINES = ["done!", "pasted ✓", "all yours", "shipped!", "in it goes"];
  const THINK_FALLBACKS = ["thinking", "untangling words", "decoding you"];
  const WRITE_FALLBACKS = ["polishing", "tidying it up", "making it shine"];
  function pick(pool: string[]): string {
    return pool[Math.floor(Math.random() * pool.length)];
  }
  let runLines = $state({ done: DONE_LINES[0], think: THINK_FALLBACKS[0], write: WRITE_FALLBACKS[0] });
  $effect(() => {
    if (displayState === "listening") {
      runLines = { done: pick(DONE_LINES), think: pick(THINK_FALLBACKS), write: pick(WRITE_FALLBACKS) };
    }
  });

  let labels = $derived({
    listening: listenLabel(listenElapsed, activeApp),
    thinking: sttProvider ? `transcribing · ${prettyProvider(sttProvider)}` : runLines.think,
    writing: llmProvider ? `polishing · ${prettyProvider(llmProvider)}` : runLines.write,
    writingIcon: "✏️",
    pasting: runLines.done,
  });

  // ── Idle hover quips ────────────────────────────────────────────────
  // The floater spends most of its life idle; give it a little personality
  // when the user comes looking. Hovering the resting avatar for ~700ms
  // surfaces one random quip for a few seconds, once per hover. (Deliberate
  // and user-initiated — no random popups over whatever you're working on.)
  const IDLE_QUIPS = [
    "press F8, I'm warmed up",
    "your words, my paws",
    "quiet day, huh?",
    "I transcribe, therefore I am",
    "say something nice",
    "*stretches* …ready when you are",
    "the mic misses you",
    "got a thought? I'll catch it",
  ];
  const IDLE_QUIPS_DUO = [
    "we work in shifts. both asleep",
    "one of us is listening. probably",
    "*synchronised tail flick*",
    "the white one supervises. allegedly",
    "feed us words",
    "two cats, zero typos",
    "psst — F8. we're bored",
  ];
  const IDLE_QUIPS_CODEX_FOX = [
    "Codex fox online",
    "blue scarf, clean paste",
    "say it. I'll tidy the edges",
    "ears calibrated for accents",
    "F8 and I pounce",
  ];
  const IDLE_QUIPS_ORU_GUJIA = [
    "Gujia supervises. Oru touches everything",
    "two cats, one keyboard",
    "Oru heard F8 and arrived upside down",
    "Gujia has reviewed this silence",
    "cream tax payable in words",
    "we caught the thought. mostly",
  ];
  const IDLE_QUIPS_SPARK = [
    "zap me with a thought",
    "battery full. patience medium",
    "tiny volts, tidy notes",
    "say the thing, I'll spark it",
    "no thunder, just transcription",
  ];
  let _quipShowTimer: ReturnType<typeof setTimeout> | null = null;
  let _quipHideTimer: ReturnType<typeof setTimeout> | null = null;
  $effect(() => {
    // Wave bar never quips.
    const idleHover = skin !== "wave" && hovering && displayState === "idle";
    const pool =
      skin === "codex-fox" ? IDLE_QUIPS_CODEX_FOX :
      skin === "oru-gujia" ? IDLE_QUIPS_ORU_GUJIA :
      skin === "spark-buddy" ? IDLE_QUIPS_SPARK :
      skin === "duo" || skin === "duo-hd" ? IDLE_QUIPS_DUO :
      IDLE_QUIPS;
    if (idleHover) {
      if (!_quipShowTimer && !hoverQuip) {
        _quipShowTimer = setTimeout(() => {
          _quipShowTimer = null;
          hoverQuip = pick(pool);
          _quipHideTimer = setTimeout(() => {
            _quipHideTimer = null;
            hoverQuip = "";
          }, 3800);
        }, 700);
      }
    } else {
      if (_quipShowTimer) {
        clearTimeout(_quipShowTimer);
        _quipShowTimer = null;
      }
      if (_quipHideTimer) {
        clearTimeout(_quipHideTimer);
        _quipHideTimer = null;
      }
      hoverQuip = "";
    }
  });

  // ── Wave bar (skin "wave") ──────────────────────────────────────────────
  // A minimal Wispr-Flow-style pill with a live audio waveform. No text, no
  // bubbles, no quips — ever. Levels arrive from Rust via `wispr:level`
  // (f32 0..1, throttled ~90ms while recording). We keep a ring buffer of
  // recent levels and render ~24 vertical bars, smoothed via rAF so the
  // motion feels liquid rather than steppy.
  const WAVE_BARS = 20;
  // Smoothing factor per rAF tick: how fast a bar chases its target height.
  const WAVE_LERP = 0.3;
  // Idle "resting" bar height (0..1) — a faint centered line of small dots.
  const WAVE_IDLE = 0.05;
  // How long the success (green) breathe lasts, and the error (red) blink.
  const WAVE_SUCCESS_MS = 900;
  const WAVE_ERROR_MS = 900;

  // Target heights (what we're animating toward) and displayed heights (what's
  // actually painted, lerped toward target each frame). Both 0..1.
  let waveTargets = $state<number[]>(new Array(WAVE_BARS).fill(WAVE_IDLE));
  let waveHeights = $state<number[]>(new Array(WAVE_BARS).fill(WAVE_IDLE));
  // Ring buffer of the most recent live levels (newest last). Fed by wispr:level.
  let waveRing: number[] = new Array(WAVE_BARS).fill(WAVE_IDLE);
  let waveSuccessUntil = 0;
  let waveRaf: number | null = null;
  // Smoothed 0..1 mic level — drives the Siri orb's bloom/scale. Decays to 0
  // when no levels arrive so the orb settles between dictations.
  let micLevel = $state(0);

  function pushLevel(v: number) {
    const clamped = Math.max(0, Math.min(1, v));
    waveRing.push(clamped);
    if (waveRing.length > WAVE_BARS) waveRing.shift();
  }

  // Compute the target bar heights from the current display state. Runs each
  // rAF tick so transcribing/cleaning sweeps and idle shimmer stay animated
  // without extra timers.
  function computeWaveTargets(now: number): number[] {
    const s = displayState;
    if (s === "listening") {
      // Live waveform: history left→right, newest on the right.
      return waveRing.slice(-WAVE_BARS);
    }
    if (s === "thinking" || s === "writing") {
      // "Processing" motion — a soft symmetric shimmer that ripples across
      // the bars (reads as thinking, not as a level meter). Stays mid-height.
      const out = new Array(WAVE_BARS);
      for (let i = 0; i < WAVE_BARS; i++) {
        const pos = i / (WAVE_BARS - 1);
        const wobble = Math.sin(pos * Math.PI * 2 + now / 260)
          * Math.sin(pos * Math.PI); // taper the ends down
        out[i] = 0.3 + 0.24 * (0.5 + 0.5 * wobble);
      }
      return out;
    }
    // idle / pasting: settle to a low centered line with a slow breath.
    const breathe = WAVE_IDLE + 0.04 * (0.5 + 0.5 * Math.sin(now / 1500));
    return new Array(WAVE_BARS).fill(breathe);
  }

  function waveTick(now: number) {
    const targets = computeWaveTargets(now);
    const next = new Array(WAVE_BARS);
    for (let i = 0; i < WAVE_BARS; i++) {
      const cur = waveHeights[i] ?? WAVE_IDLE;
      next[i] = cur + (targets[i] - cur) * WAVE_LERP;
    }
    waveTargets = targets;
    waveHeights = next;
    // Smoothed mic level for the Siri orb. While listening chase the newest
    // ring value; otherwise decay toward 0.
    const latest = displayState === "listening" ? (waveRing[waveRing.length - 1] ?? 0) : 0;
    micLevel = micLevel + (latest - micLevel) * (latest > micLevel ? 0.5 : 0.12);
    if (isMinimalSkin(skin)) {
      waveRaf = requestAnimationFrame(waveTick);
    } else {
      waveRaf = null;
    }
  }

  // Success (green breathe) flag — one pulse on paste. Shared by wave + siri.
  let waveSuccess = $state(false);
  $effect(() => {
    if (isMinimalSkin(skin) && displayState === "pasting") {
      waveSuccess = true;
      waveSuccessUntil = Date.now() + WAVE_SUCCESS_MS;
      setTimeout(() => {
        if (Date.now() >= waveSuccessUntil) waveSuccess = false;
      }, WAVE_SUCCESS_MS);
    }
  });

  // Error (red blink ×3) flag — fired from the flow_error listener.
  let waveError = $state(false);
  let _waveErrTimer: ReturnType<typeof setTimeout> | null = null;
  function triggerWaveError() {
    waveError = true;
    if (_waveErrTimer) clearTimeout(_waveErrTimer);
    _waveErrTimer = setTimeout(() => (waveError = false), WAVE_ERROR_MS);
  }

  // Start/stop the rAF loop with the minimal skins (wave + siri).
  $effect(() => {
    if (isMinimalSkin(skin)) {
      if (waveRaf == null) waveRaf = requestAnimationFrame(waveTick);
    } else if (waveRaf != null) {
      cancelAnimationFrame(waveRaf);
      waveRaf = null;
      // Reset so a later switch back starts clean.
      waveRing = new Array(WAVE_BARS).fill(WAVE_IDLE);
      waveHeights = new Array(WAVE_BARS).fill(WAVE_IDLE);
      micLevel = 0;
    }
  });

  // ── Enter / exit arrival animations ─────────────────────────────────────
  // The avatar always "arrives" warmly (the old Clippy touch). In `auto` mode
  // these bracket each show/hide; in `always` mode the enter plays once on
  // first mount so the avatar pops in rather than blinking into existence.
  const ENTER_MS = 380;
  const EXIT_MS = 240;
  // After the flow returns to idle, wait this long (letting the success/status
  // bubble finish) before playing the exit + hiding in `auto` mode.
  const AUTO_HIDE_GRACE_MS = 1800;

  let arriveClass = $state<"enter" | "exit" | "">("");
  let _enterTimer: ReturnType<typeof setTimeout> | null = null;
  function playEnter() {
    if (_enterTimer) clearTimeout(_enterTimer);
    arriveClass = "enter";
    _enterTimer = setTimeout(() => {
      arriveClass = "";
      _enterTimer = null;
    }, ENTER_MS);
  }
  function playExit(): Promise<void> {
    if (_enterTimer) {
      clearTimeout(_enterTimer);
      _enterTimer = null;
    }
    arriveClass = "exit";
    return new Promise((resolve) => setTimeout(resolve, EXIT_MS));
  }

  // ── Auto-mode visibility state machine ──────────────────────────────────
  // When visibility === "auto", THIS webview owns show/hide (the window keeps
  // running JS even while hidden). Show (no focus steal) + ENTER on the first
  // non-idle state; after returning to idle, wait AUTO_HIDE_GRACE_MS, play
  // EXIT, then hide. A re-trigger during the grace/exit cancels the pending
  // hide cleanly.
  let _autoHideTimer: ReturnType<typeof setTimeout> | null = null;
  let _autoShown = false; // our belief about whether we've shown the window
  let _autoExiting = false;

  function cancelPendingAutoHide() {
    if (_autoHideTimer) {
      clearTimeout(_autoHideTimer);
      _autoHideTimer = null;
    }
    _autoExiting = false;
  }

  async function autoShow() {
    cancelPendingAutoHide();
    if (_autoShown) {
      // Already visible; if we were mid-exit, cancel it and re-enter warmly.
      playEnter();
      return;
    }
    _autoShown = true;
    try {
      await getCurrentWindow().show(); // NO setFocus — never steal focus.
    } catch (e) {
      console.warn("[clippy] auto show failed", e);
    }
    playEnter();
  }

  function scheduleAutoHide() {
    cancelPendingAutoHide();
    _autoHideTimer = setTimeout(async () => {
      _autoHideTimer = null;
      _autoExiting = true;
      await playExit();
      // Re-check: a re-trigger during the exit animation may have flipped us
      // back to a non-idle state — bail without hiding.
      if (!_autoExiting) return;
      _autoExiting = false;
      _autoShown = false;
      arriveClass = "";
      try {
        await getCurrentWindow().hide();
      } catch (e) {
        console.warn("[clippy] auto hide failed", e);
      }
    }, AUTO_HIDE_GRACE_MS);
  }

  // React to flow state + visibility changes for auto mode.
  $effect(() => {
    const vis = avatarVisibility.current;
    const st = flowState;
    if (vis === "hidden") {
      // Self-enforce hidden from this side too. Rust unconditionally shows
      // the floater at startup (it can't read localStorage), so without this
      // a "hidden" user would get a permanently-revived avatar every launch.
      cancelPendingAutoHide();
      _autoShown = false;
      getCurrentWindow().hide().catch(() => {});
      return;
    }
    if (vis !== "auto") {
      // Leaving auto: cancel any pending hide; the main window now owns the
      // window state via applyVisibilityWindow.
      cancelPendingAutoHide();
      return;
    }
    if (st !== "idle") {
      // Active pipeline → make sure we're shown (cancels any pending hide).
      void autoShow();
    } else if (_autoShown) {
      // Returned to idle while shown → schedule the graceful hide.
      scheduleAutoHide();
    } else {
      // Idle and not shown (e.g. just flipped to auto while idle) → hide after
      // a short grace so the user sees it take effect.
      cancelPendingAutoHide();
      _autoHideTimer = setTimeout(async () => {
        _autoHideTimer = null;
        try {
          await getCurrentWindow().hide();
        } catch {}
      }, 600);
    }
  });
</script>

<svelte:window oncontextmenu={suppressDocCtx} />

<div
  class="clippy-stage"
  class:stage-hidden={!stageVisible}
  class:arrive-enter={arriveClass === "enter"}
  class:arrive-exit={arriveClass === "exit"}
  data-arrive-origin={isMinimalSkin(skin) ? "center" : "bottom"}
  style="--fscale:{fscale}; --bubble-scale:{bubbleScale}; --bubble-bottom:{bubbleBottom}px;"
  role="button"
  tabindex="0"
  aria-label="wispr-fox floater — drag to move, right-click for options"
  onmousedown={onStageMouseDown}
  onmouseup={endDrag}
  ondblclick={openMainWindow}
  oncontextmenu={openContextMenu}
  onmouseenter={() => (hovering = true)}
  onmouseleave={() => { hovering = false; hoverShiftX = 0; hoverShiftY = 0; endDrag(); }}
  onmousemove={(e) => {
    maybeStartDrag(e);
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
  {#if toastMessage && skin !== "wave"}
    <div class="bubble show" data-state={toastKind === "error" ? "toast-error" : "toast"}>
      <span class="bubble-text">{toastMessage}</span>
      <span class="bubble-emoji">{toastKind === "error" ? "⚠" : "📋"}</span>
    </div>
  {/if}

  <!-- Soft floor glow/shadow under Clippy — renders for ALL skins (not just
       the SVG paperclip) because it grounds the character visually. Pulses
       gently while listening to reinforce the "alive and attentive" feel. -->
  {#if skin !== "off" && !isMinimalSkin(skin)}
    <div class="shadow" class:pulse={displayState === "listening"}></div>
  {/if}

  <!-- Debug overlay (Settings → Appearance → Floater debug overlay). Draws
       the exact window bounds + a live size readout. The "ask vs got" line is
       the tell: if they differ, the resize was rejected. -->
  {#if debug}
    <div class="dbg-frame" aria-hidden="true"></div>
    <div class="dbg-readout" aria-hidden="true">
      <div><b>{skin}</b></div>
      <div>ask {dbg.targetW}×{dbg.targetH}</div>
      <div class:dbg-bad={dbg.actualW !== dbg.targetW || dbg.actualH !== dbg.targetH}>
        got {dbg.actualW}×{dbg.actualH}
      </div>
      <div>scale {Math.round(fscale * 100)}% · sf {dbg.sf}</div>
    </div>
  {/if}

  {#if skin === "stylized" || skin === "fox" || isRasterAvatarSkin(skin) || skin === "cat" || skin === "duo" || skin === "duo-hd" || skin === "real-clippy"}

    <!-- State-driven bubble — our own consistent dialog box, shown for ALL
         skins including real Clippy (user asked for the same dialog box on
         old-style Clippy too; we never call clippyts' own .speak balloon, so
         there's no double-bubble). Hidden while a toast is up so we don't
         stack two bubbles. -->
    {#if !toastMessage}
      <div
        class="bubble"
        class:show={displayState !== "idle" || hoverQuip !== ""}
        data-state={displayState === "idle" && hoverQuip ? "quip" : displayState}
        data-skin={skin}
      >
        {#if displayState === "idle" && hoverQuip}
          <span class="bubble-text">{hoverQuip}</span>
        {:else if displayState === "listening"}
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
  {:else if isRasterAvatarSkin(skin)}
    <RasterAvatar
      {skin}
      state={displayState}
      {mode}
      {hovering}
      {phewActive}
    />
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

  {:else if skin === "duo"}
    <!-- ═══════════════════════════════════════════════════════════════════
         KHAUMANI & INDY — the two-cat team (modeled on the user's real cats).
         A serene WHITE cat loafing on a console slab (left) supervises while
         an ORANGE tabby kitten (right) does the actual work. Wide + LOW art
         so the floater claims minimal height.
         States:
           idle:      white loaf-breathes + slow blinks; orange sways, head
                      tilts now and then; every ~16s orange leans over to
                      bother the white one, whose ear flicks in reply
           listening: both perk — ears up, eyes wide, orange sits tall
           thinking:  white squints with thought-dots; orange head-tilts
           writing:   orange types furiously; white supervises (gaze down)
           pasting:   PAW BUMP — both raise a paw, sparkles
         ═══════════════════════════════════════════════════════════════════ -->
    <svg
      class="character duo-skin"
      viewBox="0 0 220 130"
      xmlns="http://www.w3.org/2000/svg"
      data-state={displayState}
      data-mode={mode}
      aria-hidden="true"
    >
      <defs>
        <linearGradient id="duo-orange-grad" x1="0.3" y1="0" x2="0.7" y2="1">
          <stop offset="0%" stop-color="#f5ab60"/>
          <stop offset="100%" stop-color="#e08334"/>
        </linearGradient>
        <linearGradient id="duo-white-grad" x1="0.5" y1="0" x2="0.5" y2="1">
          <stop offset="0%" stop-color="#ffffff"/>
          <stop offset="100%" stop-color="#f1ede3"/>
        </linearGradient>
        <linearGradient id="duo-slab-grad" x1="0.5" y1="0" x2="0.5" y2="1">
          <stop offset="0%" stop-color="#f4f2ec"/>
          <stop offset="100%" stop-color="#dcd8cc"/>
        </linearGradient>
      </defs>

      <!-- ─── Console slab (the white cat's throne, à la the PS5) ────── -->
      <g class="duo-slab">
        <rect x="8" y="112" width="102" height="14" rx="6" fill="url(#duo-slab-grad)" stroke="#c5c0b2" stroke-width="0.8"/>
        <rect x="14" y="117" width="90" height="3.6" rx="1.8" fill="#2b2b2b"/>
      </g>

      <!-- ─── WHITE CAT — loafing, serene ────────────────────────────── -->
      <g class="duo-white">
        <g class="duo-white-body">
          <!-- Loaf -->
          <ellipse cx="55" cy="91" rx="39" ry="22" fill="url(#duo-white-grad)" stroke="#d8d2c4" stroke-width="1"/>
          <!-- Haunch hint -->
          <path d="M 24 84 Q 18 95 26 106" fill="none" stroke="#ddd7c9" stroke-width="1" stroke-linecap="round"/>
          <!-- Tail wrapped around the front of the loaf -->
          <g class="duo-white-tail">
            <path d="M 90 100 Q 70 116 40 110 Q 28 107 26 100" fill="none" stroke="#f8f6f0" stroke-width="7.5" stroke-linecap="round"/>
            <path d="M 90 100 Q 70 116 40 110 Q 28 107 26 100" fill="none" stroke="#d8d2c4" stroke-width="8.8" stroke-linecap="round" opacity="0.25"/>
          </g>
          <!-- Tucked front paws peeking out of the loaf -->
          <ellipse cx="62" cy="110" rx="7" ry="3.6" fill="#ffffff" stroke="#ddd7c9" stroke-width="0.7"/>
          <ellipse cx="78" cy="110" rx="7" ry="3.6" fill="#ffffff" stroke="#ddd7c9" stroke-width="0.7"/>
        </g>

        <g class="duo-white-head">
          <!-- Ears (slim, tall — like the photo) -->
          <g class="duo-ears duo-white-ears">
            <path d="M 62 58 L 57 36 L 74 50 Z" fill="#fbfaf6" stroke="#d8d2c4" stroke-width="0.9"/>
            <path d="M 63 55 L 60 42 L 71 50 Z" fill="#f2c6c2" opacity="0.8"/>
            <g class="duo-white-ear-r">
              <path d="M 90 56 L 95 34 L 78 48 Z" fill="#fbfaf6" stroke="#d8d2c4" stroke-width="0.9"/>
              <path d="M 89 53 L 92 40 L 81 48 Z" fill="#f2c6c2" opacity="0.8"/>
            </g>
          </g>
          <!-- Head — slim face -->
          <circle cx="76" cy="69" r="19" fill="url(#duo-white-grad)" stroke="#d8d2c4" stroke-width="1"/>
          <!-- Cheek shading -->
          <ellipse cx="70" cy="63" rx="8" ry="5.5" fill="#ffffff" opacity="0.9"/>
          <!-- Eyes — heavy-lidded calm at idle, round + awake otherwise -->
          <g class="duo-eyes">
            <ellipse cx={68 + eyeShiftX * 0.6} cy={67 + eyeShiftY * 0.4} rx="3.2"
              ry={blinkOpen ? (displayState === "idle" && !hovering ? 2.4 : 4) : 0.4}
              fill="#5d4430" stroke="#3c2c1e" stroke-width="0.6"/>
            <circle cx={67.2 + eyeShiftX * 0.5} cy={65.8 + eyeShiftY * 0.3} r={blinkOpen ? 0.9 : 0} fill="#ffffff" opacity="0.9"/>
            <ellipse cx={84 + eyeShiftX * 0.6} cy={67 + eyeShiftY * 0.4} rx="3.2"
              ry={blinkOpen ? (displayState === "idle" && !hovering ? 2.4 : 4) : 0.4}
              fill="#5d4430" stroke="#3c2c1e" stroke-width="0.6"/>
            <circle cx={83.2 + eyeShiftX * 0.5} cy={65.8 + eyeShiftY * 0.3} r={blinkOpen ? 0.9 : 0} fill="#ffffff" opacity="0.9"/>
          </g>
          <!-- Nose + mouth -->
          <path d="M 74.6 74 L 76 76.4 L 77.4 74 Z" fill="#eda4a4" stroke="#d68a8a" stroke-width="0.4"/>
          <path d="M 76 76.4 L 76 78" fill="none" stroke="#c9c2b2" stroke-width="0.7" stroke-linecap="round"/>
          <path d="M 72.5 78.6 Q 76 80.8 79.5 78.6" fill="none" stroke="#c9c2b2" stroke-width="0.7" stroke-linecap="round"/>
          <!-- Whiskers -->
          <g stroke="#cfc8b8" stroke-width="0.6" stroke-linecap="round">
            <line x1="56" y1="71" x2="70" y2="73"/>
            <line x1="55" y1="76" x2="70" y2="76"/>
            <line x1="82" y1="73" x2="96" y2="71"/>
            <line x1="82" y1="76" x2="97" y2="76"/>
          </g>
          <!-- Thought dots — white cat does the thinking -->
          {#if displayState === "thinking"}
            <g class="duo-think-dots" fill="#fbfaf6" stroke="#c9c2b2" stroke-width="0.7">
              <circle cx="94" cy="42" r="2.2"/>
              <circle cx="101" cy="33" r="3.1"/>
              <circle cx="110" cy="22" r="4.2"/>
            </g>
          {/if}
        </g>
      </g>

      <!-- ─── ORANGE TABBY — upright, eager ──────────────────────────── -->
      <g class="duo-orange">
        <!-- Tail — long, expressive -->
        <g class="duo-orange-tail">
          <path d="M 186 110 C 202 104, 208 88, 200 74" fill="none" stroke="#e8913f" stroke-width="6.5" stroke-linecap="round"/>
          <path d="M 200 74 C 198 70, 194 68, 190 70" fill="none" stroke="#cf7427" stroke-width="6" stroke-linecap="round"/>
        </g>

        <g class="duo-orange-body">
          <!-- Body — upright pear -->
          <ellipse cx="162" cy="99" rx="27" ry="26" fill="url(#duo-orange-grad)" stroke="#c96f28" stroke-width="1"/>
          <!-- Side stripes -->
          <g fill="none" stroke="#cf7427" stroke-width="2.4" stroke-linecap="round" opacity="0.8">
            <path d="M 180 86 Q 186 92 184 100"/>
            <path d="M 183 98 Q 188 104 185 111"/>
            <path d="M 142 90 Q 138 97 141 105"/>
          </g>
          <!-- Chest patch -->
          <ellipse cx="156" cy="106" rx="13" ry="14" fill="#fdf6ec" opacity="0.95"/>
          <!-- Front legs / typing paws -->
          {#if displayState === "writing"}
            <g class="duo-paw-l">
              <rect x="147" y="104" width="7" height="18" rx="3.5" fill="#f0a050" stroke="#c96f28" stroke-width="0.7"/>
              <ellipse cx="150.5" cy="122" rx="5.5" ry="3.4" fill="#fdf6ec" stroke="#c96f28" stroke-width="0.7"/>
            </g>
            <g class="duo-paw-r">
              <rect x="160" y="104" width="7" height="18" rx="3.5" fill="#f0a050" stroke="#c96f28" stroke-width="0.7"/>
              <ellipse cx="163.5" cy="122" rx="5.5" ry="3.4" fill="#fdf6ec" stroke="#c96f28" stroke-width="0.7"/>
            </g>
          {:else}
            <rect x="147" y="102" width="7" height="20" rx="3.5" fill="#f0a050" stroke="#c96f28" stroke-width="0.7"/>
            <rect x="160" y="102" width="7" height="20" rx="3.5" fill="#f0a050" stroke="#c96f28" stroke-width="0.7"/>
            <ellipse cx="150.5" cy="122" rx="5.5" ry="3.4" fill="#fdf6ec" stroke="#c96f28" stroke-width="0.7"/>
            <ellipse cx="163.5" cy="122" rx="5.5" ry="3.4" fill="#fdf6ec" stroke="#c96f28" stroke-width="0.7"/>
          {/if}
        </g>

        <g class="duo-orange-head">
          <!-- Ears — big kitten ears -->
          <g class="duo-ears duo-orange-ears">
            <path d="M 147 46 L 141 22 L 161 38 Z" fill="#ef9c4e" stroke="#c96f28" stroke-width="0.9"/>
            <path d="M 148 42 L 145 28 L 158 38 Z" fill="#f0b3a4" opacity="0.85"/>
            <path d="M 179 46 L 185 22 L 165 38 Z" fill="#ef9c4e" stroke="#c96f28" stroke-width="0.9"/>
            <path d="M 178 42 L 181 28 L 168 38 Z" fill="#f0b3a4" opacity="0.85"/>
          </g>
          <!-- Head -->
          <circle cx="163" cy="58" r="21" fill="url(#duo-orange-grad)" stroke="#c96f28" stroke-width="1"/>
          <!-- Forehead tabby stripes (the classic M) -->
          <g fill="none" stroke="#cf7427" stroke-width="2" stroke-linecap="round" opacity="0.85">
            <path d="M 156 41 L 155 48"/>
            <path d="M 163 39 L 163 47"/>
            <path d="M 170 41 L 171 48"/>
          </g>
          <!-- Muzzle patch -->
          <ellipse cx="163" cy="67" rx="11.5" ry="8" fill="#fdf6ec"/>
          <!-- Eyes — big, round, curious (kitten energy) -->
          <g class="duo-eyes">
            <ellipse cx={154 + eyeShiftX * 0.8} cy={56 + eyeShiftY * 0.5} rx="4"
              ry={blinkOpen ? 4.4 : 0.5} fill="#b97f33" stroke="#7a4d1c" stroke-width="0.7"/>
            <circle cx={154 + eyeShiftX * 0.8} cy={56.4 + eyeShiftY * 0.5} r={blinkOpen ? 2.1 : 0} fill="#1c1208"/>
            <circle cx={152.8 + eyeShiftX * 0.6} cy={54.6 + eyeShiftY * 0.4} r={blinkOpen ? 1.1 : 0} fill="#ffffff" opacity="0.95"/>
            <ellipse cx={172 + eyeShiftX * 0.8} cy={56 + eyeShiftY * 0.5} rx="4"
              ry={blinkOpen ? 4.4 : 0.5} fill="#b97f33" stroke="#7a4d1c" stroke-width="0.7"/>
            <circle cx={172 + eyeShiftX * 0.8} cy={56.4 + eyeShiftY * 0.5} r={blinkOpen ? 2.1 : 0} fill="#1c1208"/>
            <circle cx={170.8 + eyeShiftX * 0.6} cy={54.6 + eyeShiftY * 0.4} r={blinkOpen ? 1.1 : 0} fill="#ffffff" opacity="0.95"/>
          </g>
          <!-- Nose + mouth -->
          <path d="M 161.4 63.5 L 163 66 L 164.6 63.5 Z" fill="#e58f86" stroke="#c97168" stroke-width="0.4"/>
          <path d="M 163 66 L 163 68" fill="none" stroke="#b98a55" stroke-width="0.7" stroke-linecap="round"/>
          {#if displayState === "pasting"}
            <path d="M 158 68.5 Q 163 73 168 68.5" fill="none" stroke="#a5713a" stroke-width="1" stroke-linecap="round"/>
          {:else}
            <path d="M 159.5 69 Q 163 71.2 166.5 69" fill="none" stroke="#b98a55" stroke-width="0.7" stroke-linecap="round"/>
          {/if}
          <!-- Whiskers -->
          <g stroke="#e8d8c0" stroke-width="0.7" stroke-linecap="round">
            <line x1="140" y1="62" x2="153" y2="64"/>
            <line x1="139" y1="67" x2="153" y2="67"/>
            <line x1="173" y1="64" x2="186" y2="62"/>
            <line x1="173" y1="67" x2="187" y2="67"/>
          </g>
        </g>
      </g>

      <!-- ─── Paw bump — pasting celebration ─────────────────────────── -->
      {#if displayState === "pasting"}
        <g class="duo-bump">
          <!-- White cat's paw reaches right, orange's reaches left -->
          <path d="M 92 88 Q 104 82 113 79" fill="none" stroke="#fbfaf6" stroke-width="7" stroke-linecap="round"/>
          <path d="M 92 88 Q 104 82 113 79" fill="none" stroke="#d8d2c4" stroke-width="8.2" stroke-linecap="round" opacity="0.3"/>
          <path d="M 140 84 Q 128 81 119 79" fill="none" stroke="#f0a050" stroke-width="7" stroke-linecap="round"/>
          <!-- Sparkles at the meeting point -->
          <g class="duo-sparkles" fill="#e8b54a">
            <path d="M 116 66 L 117.6 70 L 121.6 71.6 L 117.6 73.2 L 116 77.2 L 114.4 73.2 L 110.4 71.6 L 114.4 70 Z"/>
            <path d="M 128 84 L 129 86.4 L 131.4 87.4 L 129 88.4 L 128 90.8 L 127 88.4 L 124.6 87.4 L 127 86.4 Z"/>
            <path d="M 104 80 L 104.8 82 L 106.8 82.8 L 104.8 83.6 L 104 85.6 L 103.2 83.6 L 101.2 82.8 L 103.2 82 Z"/>
          </g>
        </g>
      {/if}

      <!-- ─── Phew drop ──────────────────────────────────────────────── -->
      {#if phewActive}
        <g class="phew-drop">
          <path d="M 196 44 Q 194 38 196 32 Q 198 38 196 44 Z" fill="#7cb6ff" stroke="#1d1d1f" stroke-width="0.8"/>
          <text x="193" y="28" font-size="6" fill="#1d1d1f" font-family="ui-sans-serif, sans-serif">phew</text>
        </g>
      {/if}
    </svg>
  {:else if skin === "duo-hd"}
    <!-- ═══════════════════════════════════════════════════════════════════
         KHAUMANI & INDY ✦ — the REMASTERED duo. A from-scratch, higher-
         fidelity take on the user's two real cats, driven by a single SCENE-
         DIRECTOR timeline (one 22s master clock) so the cats genuinely move
         around instead of wiggling in place:

           idle/scene loop (22s):
             ~0–52%   both rest — Khaumani loafs & breathes, Indy sits and
                      sways, tails flick, slow ambient blinks, dust motes drift
             ~55–66%  INDY POUNCES — crouch (squash) → leap left toward Khaumani
                      (stretch in air) → land beside her (squash). A real arc.
             ~62–74%  KHAUMANI reacts — turns her head toward Indy and gives a
                      slow "cat-love" blink; ear flick on the landing
             ~76–84%  Indy hops back to its spot and settles
             ~84–100% groom + rest, loop
           listening: both snap alert — ears perk, Indy sits bolt upright with
                      an attentive bob, Khaumani lifts her head, pupils dilate
           thinking:  Indy head-tilts with a thought-dot trail; Khaumani watches
           writing:   Indy bats at the air (playful "typing"), tail lashing;
                      Khaumani supervises, gaze down
           pasting:   happy double-hop + nose-BOOP between them + sparkles

         High-fidelity details vs the original "duo": gradient fur volume with
         belly/back shading, fur tufts on cheeks & chest, mackerel tabby
         striping + ringed tail on Indy, slit-pupil green eyes (Khaumani) and
         big round amber eyes (Indy) with dual catchlights, blurred contact
         shadows, and a warm sunbeam ledge they sit on.
         ═══════════════════════════════════════════════════════════════════ -->
    <svg
      class="character hd-skin"
      viewBox="0 0 240 156"
      xmlns="http://www.w3.org/2000/svg"
      data-state={displayState}
      data-mode={mode}
      aria-hidden="true"
    >
      <defs>
        <radialGradient id="hd-sun" cx="0.46" cy="0.4" r="0.62">
          <stop offset="0%" stop-color="#ffe6b6" stop-opacity="0.62"/>
          <stop offset="58%" stop-color="#ffd99c" stop-opacity="0.18"/>
          <stop offset="100%" stop-color="#ffd99c" stop-opacity="0"/>
        </radialGradient>
        <linearGradient id="hd-ledge" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stop-color="#ecca94"/>
          <stop offset="20%" stop-color="#d3a96d"/>
          <stop offset="100%" stop-color="#a87c47"/>
        </linearGradient>
        <linearGradient id="hd-cushion" x1="0.5" y1="0" x2="0.5" y2="1">
          <stop offset="0%" stop-color="#e3b3a4"/>
          <stop offset="100%" stop-color="#c98e7d"/>
        </linearGradient>
        <linearGradient id="hd-khao-fur" x1="0.5" y1="0" x2="0.5" y2="1">
          <stop offset="0%" stop-color="#ffffff"/>
          <stop offset="64%" stop-color="#f4efe6"/>
          <stop offset="100%" stop-color="#e2dccb"/>
        </linearGradient>
        <radialGradient id="hd-khao-belly" cx="0.5" cy="0.38" r="0.62">
          <stop offset="0%" stop-color="#ffffff"/>
          <stop offset="100%" stop-color="#ffffff" stop-opacity="0"/>
        </radialGradient>
        <linearGradient id="hd-indy-fur" x1="0.38" y1="0" x2="0.62" y2="1">
          <stop offset="0%" stop-color="#f9b96e"/>
          <stop offset="58%" stop-color="#ef9846"/>
          <stop offset="100%" stop-color="#d87d2f"/>
        </linearGradient>
        <radialGradient id="hd-eye-green" cx="0.5" cy="0.38" r="0.62">
          <stop offset="0%" stop-color="#c4e493"/>
          <stop offset="52%" stop-color="#8eb95f"/>
          <stop offset="100%" stop-color="#5d8a3c"/>
        </radialGradient>
        <radialGradient id="hd-eye-amber" cx="0.5" cy="0.38" r="0.62">
          <stop offset="0%" stop-color="#ffd57e"/>
          <stop offset="52%" stop-color="#dc9c40"/>
          <stop offset="100%" stop-color="#a4661f"/>
        </radialGradient>
        <filter id="hd-soft" x="-60%" y="-60%" width="220%" height="220%">
          <feGaussianBlur stdDeviation="2.6"/>
        </filter>
      </defs>

      <!-- ─── Backdrop: warm sun glow + drifting motes ─────────────────── -->
      <ellipse class="hd-sunglow" cx="110" cy="62" rx="120" ry="78" fill="url(#hd-sun)"/>
      <g class="hd-motes" fill="#fff4d8">
        <circle class="hd-mote hd-mote-1" cx="150" cy="40" r="1.5" opacity="0.7"/>
        <circle class="hd-mote hd-mote-2" cx="92" cy="30" r="1.1" opacity="0.6"/>
        <circle class="hd-mote hd-mote-3" cx="186" cy="54" r="1.3" opacity="0.5"/>
      </g>

      <!-- ─── Windowsill ledge they sit on ────────────────────────────── -->
      <g class="hd-ledge">
        <rect x="0" y="130" width="240" height="22" rx="4" fill="url(#hd-ledge)"/>
        <rect x="0" y="130" width="240" height="3.2" rx="1.6" fill="#f3dcae" opacity="0.85"/>
        <rect x="0" y="139" width="240" height="1.2" fill="#8c6536" opacity="0.4"/>
      </g>

      <!-- ─── Cushion under Khaumani ──────────────────────────────────── -->
      <g class="hd-cushion">
        <ellipse cx="74" cy="131" rx="52" ry="9" fill="url(#hd-cushion)"/>
        <ellipse cx="74" cy="129.5" rx="48" ry="6.4" fill="#e9c3b6" opacity="0.6"/>
        <path d="M 30 131 Q 74 124 118 131" fill="none" stroke="#b87e6d" stroke-width="0.8" opacity="0.5"/>
      </g>

      <!-- contact shadows -->
      <ellipse cx="76" cy="135" rx="48" ry="6.6" fill="#5a4326" opacity="0.18" filter="url(#hd-soft)"/>
      <ellipse class="hd-indy-castshadow" cx="168" cy="137" rx="28" ry="5.4" fill="#5a4326" opacity="0.2" filter="url(#hd-soft)"/>

      <!-- ═══ KHAUMANI — serene white cat, loafing (left) ═══ -->
      <g class="hd-khao">
        <!-- Tail wrapped around the front of the loaf -->
        <g class="hd-khao-tail">
          <path d="M 116 120 Q 92 140 52 134 Q 36 131 32 122"
                fill="none" stroke="#ece6d8" stroke-width="9" stroke-linecap="round"/>
          <path d="M 116 120 Q 92 140 52 134 Q 36 131 32 122"
                fill="none" stroke="#d7d0bf" stroke-width="10.4" stroke-linecap="round" opacity="0.22"/>
          <path d="M 36 122 Q 33 118 34 113" fill="none" stroke="#cfc8b6" stroke-width="3.4" stroke-linecap="round" opacity="0.5"/>
        </g>

        <g class="hd-khao-body">
          <!-- Loaf -->
          <ellipse cx="74" cy="112" rx="46" ry="24" fill="url(#hd-khao-fur)" stroke="#d9d3c4" stroke-width="1"/>
          <!-- Back shadow + belly light -->
          <path d="M 30 106 Q 40 92 74 90 Q 108 92 118 106" fill="none" stroke="#ded8c8" stroke-width="2.4" opacity="0.6"/>
          <ellipse cx="70" cy="116" rx="30" ry="13" fill="url(#hd-khao-belly)"/>
          <!-- Chest fur tufts -->
          <g fill="#ffffff" opacity="0.9">
            <path d="M 58 128 l 3 6 l 3 -6 z"/>
            <path d="M 66 129 l 2.6 5.4 l 2.6 -5.4 z"/>
            <path d="M 74 129 l 2.6 5.4 l 2.6 -5.4 z"/>
            <path d="M 82 128 l 3 6 l 3 -6 z"/>
          </g>
          <!-- Tucked front paws -->
          <ellipse cx="64" cy="132" rx="8" ry="4" fill="#ffffff" stroke="#ddd7c9" stroke-width="0.7"/>
          <ellipse cx="84" cy="132" rx="8" ry="4" fill="#ffffff" stroke="#ddd7c9" stroke-width="0.7"/>
          <path d="M 64 130.5 l 0 3 M 67 130.5 l 0 3" stroke="#e7e1d2" stroke-width="0.5"/>
          <path d="M 84 130.5 l 0 3 M 87 130.5 l 0 3" stroke="#e7e1d2" stroke-width="0.5"/>
        </g>

        <g class="hd-khao-head">
          <!-- Ears -->
          <g class="hd-khao-ear-l">
            <path d="M 64 70 L 58 44 L 79 62 Z" fill="url(#hd-khao-fur)" stroke="#d9d3c4" stroke-width="0.9"/>
            <path d="M 65 66 L 61 50 L 76 62 Z" fill="#f3c9c5" opacity="0.85"/>
          </g>
          <g class="hd-khao-ear-r">
            <path d="M 94 68 L 100 42 L 79 60 Z" fill="url(#hd-khao-fur)" stroke="#d9d3c4" stroke-width="0.9"/>
            <path d="M 93 64 L 97 48 L 82 60 Z" fill="#f3c9c5" opacity="0.85"/>
          </g>
          <!-- Head -->
          <circle cx="79" cy="84" r="22" fill="url(#hd-khao-fur)" stroke="#d9d3c4" stroke-width="1"/>
          <!-- Cheek fur tufts -->
          <g fill="#ffffff" opacity="0.8">
            <path d="M 58 86 l -5 -2.4 l 4.4 -3 z"/>
            <path d="M 59 92 l -5 0.6 l 4 -3.6 z"/>
            <path d="M 100 86 l 5 -2.4 l -4.4 -3 z"/>
            <path d="M 99 92 l 5 0.6 l -4 -3.6 z"/>
          </g>
          <ellipse cx="71" cy="78" rx="9" ry="6" fill="#ffffff" opacity="0.7"/>
          <!-- Eyes — calm green, slit pupils (heavy-lidded at rest) -->
          <g class="hd-khao-eyes">
            <ellipse cx={70 + eyeShiftX * 0.5} cy={84 + eyeShiftY * 0.4} rx="4.2" ry={hdKhOpen} fill="url(#hd-eye-green)" stroke="#4d7030" stroke-width="0.6"/>
            <ellipse cx={70 + eyeShiftX * 0.7} cy={84 + eyeShiftY * 0.5} rx={hdKhPup} ry={hdKhOpen * 0.92} fill="#241a10"/>
            <circle cx={68.4 + eyeShiftX * 0.6} cy={82 + eyeShiftY * 0.3} r={hdKhOpen > 1 ? 1.1 : 0} fill="#ffffff" opacity="0.95"/>
            <circle cx={71.4 + eyeShiftX * 0.6} cy={85.4 + eyeShiftY * 0.3} r={hdKhOpen > 1 ? 0.55 : 0} fill="#ffffff" opacity="0.7"/>
            <ellipse cx={88 + eyeShiftX * 0.5} cy={84 + eyeShiftY * 0.4} rx="4.2" ry={hdKhOpen} fill="url(#hd-eye-green)" stroke="#4d7030" stroke-width="0.6"/>
            <ellipse cx={88 + eyeShiftX * 0.7} cy={84 + eyeShiftY * 0.5} rx={hdKhPup} ry={hdKhOpen * 0.92} fill="#241a10"/>
            <circle cx={86.4 + eyeShiftX * 0.6} cy={82 + eyeShiftY * 0.3} r={hdKhOpen > 1 ? 1.1 : 0} fill="#ffffff" opacity="0.95"/>
            <circle cx={89.4 + eyeShiftX * 0.6} cy={85.4 + eyeShiftY * 0.3} r={hdKhOpen > 1 ? 0.55 : 0} fill="#ffffff" opacity="0.7"/>
          </g>
          <!-- Slow "cat-love" blink lids (driven by scene timeline) -->
          <g class="hd-khao-lids" fill="url(#hd-khao-fur)">
            <rect class="hd-lid" x="65.4" y="78.4" width="9.2" height="6" rx="3"/>
            <rect class="hd-lid" x="83.4" y="78.4" width="9.2" height="6" rx="3"/>
          </g>
          <!-- Nose + mouth -->
          <path d="M 76.6 90 L 79 92.6 L 81.4 90 Z" fill="#efa6a6" stroke="#d68a8a" stroke-width="0.4"/>
          <path d="M 79 92.6 L 79 94.6" fill="none" stroke="#cbc4b3" stroke-width="0.7" stroke-linecap="round"/>
          <path d="M 74.8 95.4 Q 79 98 83.2 95.4" fill="none" stroke="#cbc4b3" stroke-width="0.7" stroke-linecap="round"/>
          <!-- Whiskers -->
          <g stroke="#d2cbba" stroke-width="0.6" stroke-linecap="round" fill="none">
            <path d="M 56 87 Q 64 87.6 71 89"/>
            <path d="M 55 92 Q 64 92 71 92.4"/>
            <path d="M 87 89 Q 94 87.6 102 87"/>
            <path d="M 87 92.4 Q 94 92 103 92"/>
          </g>
        </g>
      </g>

      <!-- ═══ INDY — eager orange tabby kitten (right), the one that pounces ═══ -->
      <g class="hd-indy-jumper">
        <g class="hd-indy-squashbox">
          <!-- Tail — long, ringed, expressive (behind body) -->
          <g class="hd-indy-tail">
            <path d="M 190 122 C 210 116 216 96 206 80" fill="none" stroke="#e8913f" stroke-width="7" stroke-linecap="round"/>
            <g stroke="#c9701f" stroke-width="2" stroke-linecap="round" opacity="0.75">
              <path d="M 197 118 q 3 -1 5 -3"/>
              <path d="M 203 108 q 3 -1 4 -3.4"/>
              <path d="M 206 96 q 3 -0.6 3.6 -3"/>
            </g>
            <path d="M 206 80 C 204 76 200 75 197 78" fill="none" stroke="#cf7427" stroke-width="6.4" stroke-linecap="round"/>
          </g>

          <g class="hd-indy-body">
            <!-- Body — upright pear -->
            <ellipse cx="168" cy="108" rx="27" ry="28" fill="url(#hd-indy-fur)" stroke="#c96f28" stroke-width="1"/>
            <!-- Mackerel side stripes -->
            <g fill="none" stroke="#c9701f" stroke-width="2.6" stroke-linecap="round" opacity="0.8">
              <path d="M 186 90 Q 192 99 189 110"/>
              <path d="M 190 104 Q 194 112 190 121"/>
              <path d="M 147 94 Q 142 102 146 112"/>
              <path d="M 150 110 Q 146 118 150 124"/>
            </g>
            <!-- Chest/belly cream + fluff -->
            <ellipse cx="161" cy="116" rx="15" ry="16" fill="#fdf3e2" opacity="0.96"/>
            <g fill="#fdf3e2" opacity="0.95">
              <path d="M 154 130 l 3 6 l 3 -6 z"/>
              <path d="M 162 131 l 2.6 5.6 l 2.6 -5.6 z"/>
              <path d="M 170 130 l 3 6 l 3 -6 z"/>
            </g>
            <!-- Front legs / paws (typing-capable) -->
            <g class="hd-indy-paw-l">
              <rect x="153" y="116" width="8" height="20" rx="4" fill="#f0a050" stroke="#c96f28" stroke-width="0.7"/>
              <ellipse cx="157" cy="136" rx="6" ry="3.6" fill="#fdf3e2" stroke="#c96f28" stroke-width="0.7"/>
            </g>
            <g class="hd-indy-paw-r">
              <rect x="167" y="116" width="8" height="20" rx="4" fill="#f0a050" stroke="#c96f28" stroke-width="0.7"/>
              <ellipse cx="171" cy="136" rx="6" ry="3.6" fill="#fdf3e2" stroke="#c96f28" stroke-width="0.7"/>
            </g>
          </g>

          <g class="hd-indy-head">
            <!-- Big kitten ears -->
            <g class="hd-indy-ear-l">
              <path d="M 150 56 L 143 28 L 167 48 Z" fill="url(#hd-indy-fur)" stroke="#c96f28" stroke-width="0.9"/>
              <path d="M 151 52 L 147 34 L 162 48 Z" fill="#eeb3a1" opacity="0.9"/>
            </g>
            <g class="hd-indy-ear-r">
              <path d="M 190 56 L 197 28 L 173 48 Z" fill="url(#hd-indy-fur)" stroke="#c96f28" stroke-width="0.9"/>
              <path d="M 189 52 L 193 34 L 178 48 Z" fill="#eeb3a1" opacity="0.9"/>
            </g>
            <!-- Head -->
            <circle cx="170" cy="74" r="23" fill="url(#hd-indy-fur)" stroke="#c96f28" stroke-width="1"/>
            <!-- Forehead tabby "M" -->
            <g fill="none" stroke="#c9701f" stroke-width="2.1" stroke-linecap="round" opacity="0.88">
              <path d="M 162 54 L 161 63"/>
              <path d="M 170 52 L 170 62"/>
              <path d="M 178 54 L 179 63"/>
            </g>
            <!-- Cheek stripes -->
            <g fill="none" stroke="#c9701f" stroke-width="1.8" stroke-linecap="round" opacity="0.7">
              <path d="M 150 74 q -4 1 -6 3"/>
              <path d="M 190 74 q 4 1 6 3"/>
            </g>
            <!-- Muzzle cream -->
            <ellipse cx="170" cy="84" rx="13" ry="9" fill="#fdf3e2"/>
            <!-- Eyes — big, round, amber (kitten energy), dual catchlights -->
            <g class="hd-indy-eyes">
              <ellipse cx={160 + eyeShiftX * 0.8} cy={72 + eyeShiftY * 0.5} rx="4.6" ry={hdInOpen} fill="url(#hd-eye-amber)" stroke="#7a4d1c" stroke-width="0.7"/>
              <circle cx={160 + eyeShiftX} cy={72.4 + eyeShiftY * 0.6} r={hdInOpen > 1 ? hdInPup : 0} fill="#1c1208"/>
              <circle cx={158.4 + eyeShiftX * 0.8} cy={70 + eyeShiftY * 0.4} r={hdInOpen > 1 ? 1.3 : 0} fill="#ffffff" opacity="0.95"/>
              <circle cx={161.6 + eyeShiftX * 0.8} cy={74 + eyeShiftY * 0.4} r={hdInOpen > 1 ? 0.6 : 0} fill="#ffffff" opacity="0.7"/>
              <ellipse cx={180 + eyeShiftX * 0.8} cy={72 + eyeShiftY * 0.5} rx="4.6" ry={hdInOpen} fill="url(#hd-eye-amber)" stroke="#7a4d1c" stroke-width="0.7"/>
              <circle cx={180 + eyeShiftX} cy={72.4 + eyeShiftY * 0.6} r={hdInOpen > 1 ? hdInPup : 0} fill="#1c1208"/>
              <circle cx={178.4 + eyeShiftX * 0.8} cy={70 + eyeShiftY * 0.4} r={hdInOpen > 1 ? 1.3 : 0} fill="#ffffff" opacity="0.95"/>
              <circle cx={181.6 + eyeShiftX * 0.8} cy={74 + eyeShiftY * 0.4} r={hdInOpen > 1 ? 0.6 : 0} fill="#ffffff" opacity="0.7"/>
            </g>
            <!-- Nose + mouth -->
            <path d="M 167.8 80 L 170 82.8 L 172.2 80 Z" fill="#e58f86" stroke="#c97168" stroke-width="0.4"/>
            <path d="M 170 82.8 L 170 85" fill="none" stroke="#b98a55" stroke-width="0.7" stroke-linecap="round"/>
            {#if displayState === "pasting"}
              <path d="M 164 85.4 Q 170 91 176 85.4" fill="none" stroke="#a5713a" stroke-width="1.1" stroke-linecap="round"/>
            {:else}
              <path d="M 166 86 Q 170 88.6 174 86" fill="none" stroke="#b98a55" stroke-width="0.7" stroke-linecap="round"/>
            {/if}
            <!-- Whiskers -->
            <g stroke="#f0e2cc" stroke-width="0.7" stroke-linecap="round" fill="none">
              <path d="M 146 78 Q 154 78.6 160 80"/>
              <path d="M 145 83 Q 154 83 160 83.4"/>
              <path d="M 180 80 Q 187 78.6 194 78"/>
              <path d="M 180 83.4 Q 187 83 195 83"/>
            </g>
            <!-- Thought-dot trail (thinking) -->
            {#if displayState === "thinking"}
              <g class="hd-indy-thinkdots" fill="#fff7e6" stroke="#d8a85a" stroke-width="0.7">
                <circle cx="194" cy="50" r="2.2"/>
                <circle cx="202" cy="40" r="3.1"/>
                <circle cx="212" cy="28" r="4.2"/>
              </g>
            {/if}
          </g>
        </g>
      </g>

      <!-- ─── Pasting celebration: sparkles between the two ──────────────── -->
      {#if displayState === "pasting"}
        <g class="hd-sparkles" fill="#eec25a">
          <path d="M 120 70 L 122 75 L 127 77 L 122 79 L 120 84 L 118 79 L 113 77 L 118 75 Z"/>
          <path d="M 134 88 L 135.2 91 L 138.2 92.2 L 135.2 93.4 L 134 96.4 L 132.8 93.4 L 129.8 92.2 L 132.8 91 Z"/>
          <path d="M 108 86 L 109 88.4 L 111.4 89.4 L 109 90.4 L 108 92.8 L 107 90.4 L 104.6 89.4 L 107 88.4 Z"/>
        </g>
      {/if}

      <!-- ─── Phew drop ──────────────────────────────────────────────────── -->
      {#if phewActive}
        <g class="phew-drop">
          <path d="M 214 50 Q 212 44 214 38 Q 216 44 214 50 Z" fill="#7cb6ff" stroke="#1d1d1f" stroke-width="0.8"/>
          <text x="210" y="34" font-size="6" fill="#1d1d1f" font-family="ui-sans-serif, sans-serif">phew</text>
        </g>
      {/if}
    </svg>
  {:else if skin === "wave"}
    <!-- ═══════════════════════════════════════════════════════════════════
         WAVE BAR — a small Apple-style pill with a live waveform. NO text,
         NO bubbles, NO quips, ever. Slim white bars on a gray translucent
         pill; bars stay centred and cap at ~70% of the pill height. Heights
         are rAF-smoothed (waveHeights[]); colour shifts green on success,
         red-blinks on error. Scales with the S/M/L slider via --fscale.
         ═══════════════════════════════════════════════════════════════════ -->
    <div
      class="wave-pill"
      class:mac={isMacPlatform}
      class:success={waveSuccess}
      class:error={waveError}
      data-state={displayState}
      aria-hidden="true"
    >
      {#each waveHeights as h, i (i)}
        <span
          class="wave-bar"
          style="height: {Math.round((0.14 + 0.56 * Math.max(0, Math.min(1, h))) * 100)}%;"
        ></span>
      {/each}
    </div>
  {:else if skin === "siri"}
    <!-- ═══════════════════════════════════════════════════════════════════
         SIRI ORB — a tiny multicolour 3D orb. NO text, NO bubbles. Idle: a
         slow drifting gradient + gentle breath. Listening: blooms and scales
         with the live mic level. Thinking: faster swirl. Success: green ring.
         Error: red shudder. Default position is right-of-centre (see
         lib/floater-place.ts). Scales with the slider via --fscale.
         ═══════════════════════════════════════════════════════════════════ -->
    <div
      class="siri-orb"
      class:success={waveSuccess}
      class:error={waveError}
      data-state={displayState}
      style="--orb-level: {micLevel.toFixed(3)};"
      aria-hidden="true"
    >
      <span class="siri-core"></span>
      <span class="siri-gloss"></span>
      <span class="siri-ring"></span>
    </div>
  {/if}
</div>

{#if ctxMenuOpen}
  <FloaterContextMenu
    x={ctxMenuX}
    y={ctxMenuY}
    recState={flowState}
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
    /* Matches BOTTOM_PAD in the sizing math so the avatar's baseline lines up
       with where the window-size + bubble-anchor calculations expect it. */
    padding-bottom: 8px;
    cursor: grab;
  }

  .clippy-stage:active {
    cursor: grabbing;
  }

  /* Resize mask — hide instantly (no transition) so the pre-resize frame is
     already blank when the window moves, fade back in once the webview has
     laid out at the new size. See resizeFloaterCentered(). */
  .clippy-stage {
    opacity: 1;
    transition: opacity 140ms ease;
  }
  .clippy-stage.stage-hidden {
    opacity: 0;
    transition: none;
  }

  /* ── Enter / exit arrival animations ──────────────────────────────────
     The avatar always "arrives" warmly. Enter = fade + scale-up from 0.6
     with a slight overshoot pop; exit = fade + scale down to 0.75. Origin
     is bottom-center for characters (they sit at the window bottom) and
     center for the wave pill. These are `animation`s (not the `transition`
     the resize mask uses) so they don't fight the stage-hidden opacity mask. */
  .clippy-stage[data-arrive-origin="bottom"] {
    transform-origin: bottom center;
  }
  .clippy-stage[data-arrive-origin="center"] {
    transform-origin: center center;
  }
  .clippy-stage.arrive-enter {
    animation: arrive-enter 380ms cubic-bezier(0.34, 1.56, 0.64, 1) both;
  }
  .clippy-stage.arrive-exit {
    animation: arrive-exit 240ms ease-in both;
  }
  @keyframes arrive-enter {
    0%   { opacity: 0; transform: scale(0.6); }
    100% { opacity: 1; transform: scale(1); }
  }
  @keyframes arrive-exit {
    0%   { opacity: 1; transform: scale(1); }
    100% { opacity: 0; transform: scale(0.75); }
  }

  /* ── Wave bar skin ─────────────────────────────────────────────────────
     Small Apple-style stadium pill, gray translucent, with ~20 slim WHITE
     bars kept centred and capped well inside the pill height. Colour is
     driven by --wave-color (white → green on success → red on error). */
  .wave-pill {
    --wave-color: rgba(255, 255, 255, 0.92);
    position: relative;
    display: flex;
    flex: 0 0 auto; /* never shrink below the explicit pill width */
    align-items: center;
    justify-content: center;
    gap: calc(2px * var(--fscale, 1));
    width: calc(132px * var(--fscale, 1));
    height: calc(38px * var(--fscale, 1));
    padding: 0 calc(11px * var(--fscale, 1));
    box-sizing: border-box;
    border-radius: 999px;
    background: rgba(58, 60, 66, 0.34);
    border: 1px solid rgba(255, 255, 255, 0.14);
    backdrop-filter: blur(14px) saturate(1.1);
    -webkit-backdrop-filter: blur(14px) saturate(1.1);
    box-shadow: 0 6px 20px rgba(0, 0, 0, 0.24),
                inset 0 1px 0 rgba(255, 255, 255, 0.10);
    transition: background 260ms ease, border-color 260ms ease;
  }
  .wave-pill.mac {
    background: rgba(120, 124, 132, 0.20);
    border-color: rgba(120, 80, 30, 0.16);
    box-shadow: 0 6px 16px rgba(120, 80, 30, 0.16),
                inset 0 1px 0 rgba(255, 255, 255, 0.35);
  }
  .wave-bar {
    flex: 1 1 0;
    min-width: 0;
    max-width: calc(3px * var(--fscale, 1));
    border-radius: 999px;
    background: var(--wave-color);
    /* Height set inline (14..70% of the pill's inner height), centred. A short
       CSS transition on top of the rAF lerp keeps sub-frame motion silky. */
    transition: height 70ms linear, background 260ms ease;
    align-self: center;
  }
  /* Success: bars go Apple-green and breathe once. */
  .wave-pill.success { --wave-color: #34c759; }
  .wave-pill.success .wave-bar { animation: wave-breathe 900ms ease-out both; }
  @keyframes wave-breathe {
    0%   { filter: brightness(1); }
    35%  { filter: brightness(1.45); }
    100% { filter: brightness(1); }
  }
  /* Error: bars go Apple-red and blink three times. */
  .wave-pill.error { --wave-color: #ff453a; }
  .wave-pill.error .wave-bar { animation: wave-blink 900ms steps(1, end) both; }
  @keyframes wave-blink {
    0%, 100%          { opacity: 1; }
    16%, 50%, 84%     { opacity: 0.15; }
    33%, 66%          { opacity: 1; }
  }

  /* ── Siri orb skin ─────────────────────────────────────────────────────
     Tiny multicolour orb. A drifting conic gradient (siri-core) under a
     glass gloss, with a state ring on top. The whole orb scales with the
     live mic level while listening. */
  .siri-orb {
    position: relative;
    flex: 0 0 auto; /* never shrink below the explicit orb size */
    width: calc(56px * var(--fscale, 1));
    height: calc(56px * var(--fscale, 1));
    border-radius: 50%;
    /* Bloom + scale with the mic level; the base breath comes from the core. */
    transform: scale(calc(1 + 0.10 * var(--orb-level, 0)));
    transition: transform 90ms ease-out;
    box-shadow: 0 4px 16px rgba(70, 40, 120, 0.30);
    isolation: isolate;
  }
  .siri-core {
    position: absolute;
    inset: 0;
    border-radius: 50%;
    background: conic-gradient(
      from 0deg,
      #5ac8fa, #a06bff, #ff5fa2, #ff8a4c, #ffd84c, #5ae8c8, #5ac8fa
    );
    filter: saturate(1.2) brightness(1.05);
    animation: siri-spin 9s linear infinite;
  }
  .siri-gloss {
    position: absolute;
    inset: 0;
    border-radius: 50%;
    background:
      radial-gradient(60% 45% at 34% 26%, rgba(255,255,255,0.75), rgba(255,255,255,0) 60%),
      radial-gradient(120% 120% at 50% 120%, rgba(0,0,0,0.28), rgba(0,0,0,0) 55%);
    mix-blend-mode: screen;
    pointer-events: none;
  }
  .siri-ring {
    position: absolute;
    inset: -2px;
    border-radius: 50%;
    border: 2px solid transparent;
    pointer-events: none;
    transition: border-color 200ms ease, box-shadow 200ms ease;
  }
  /* Listening: brighter, faster swirl, a soft glow ring. */
  .siri-orb[data-state="listening"] .siri-core { animation-duration: 3.4s; filter: saturate(1.35) brightness(1.15); }
  .siri-orb[data-state="listening"] .siri-ring {
    border-color: rgba(255, 255, 255, 0.55);
    box-shadow: 0 0 calc(10px * var(--fscale,1)) rgba(150, 180, 255, calc(0.35 + 0.5 * var(--orb-level, 0)));
  }
  /* Thinking / writing: fastest swirl. */
  .siri-orb[data-state="thinking"] .siri-core,
  .siri-orb[data-state="writing"] .siri-core { animation-duration: 1.7s; }
  /* Success: green ring flash. */
  .siri-orb.success .siri-ring {
    border-color: #34c759;
    box-shadow: 0 0 12px rgba(52, 199, 89, 0.7);
    animation: siri-ring-breathe 900ms ease-out both;
  }
  @keyframes siri-ring-breathe {
    0% { opacity: 0.2; } 35% { opacity: 1; } 100% { opacity: 0.2; }
  }
  /* Error: red ring + a short shudder. */
  .siri-orb.error .siri-ring { border-color: #ff453a; box-shadow: 0 0 12px rgba(255, 69, 58, 0.7); }
  .siri-orb.error { animation: siri-shake 900ms ease-in-out both; }
  @keyframes siri-shake {
    0%, 100% { transform: translateX(0) scale(1); }
    15%, 45%, 75% { transform: translateX(-2px) scale(1); }
    30%, 60%, 90% { transform: translateX(2px) scale(1); }
  }
  @keyframes siri-spin {
    to { transform: rotate(360deg); }
  }
  @media (prefers-reduced-motion: reduce) {
    .siri-core { animation: none; }
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

  /* Common character behaviour — shared across the inline SVG skins.
     Raster packs own their scaling and state animation inside
     RasterAvatar.svelte; applying these legacy whole-character transforms
     to them made the bitmap roll into the floater edge. */
  .character:not(.raster-avatar) {
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
    height: calc(122px * var(--fscale, 1) * var(--state-scale, 1));
    filter: drop-shadow(0 1px 2px rgba(0, 0, 0, 0.25))
            drop-shadow(0 0 6px rgba(255, 255, 255, 0.4));
    transition: width 320ms ease, height 320ms ease;
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
  .character:not(.clippy-stylized):not(.raster-avatar)[data-state="listening"] {
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

  .character:not(.raster-avatar)[data-state="thinking"] {
    animation: thinking-tilt 1.4s ease-in-out infinite;
  }

  .character:not(.raster-avatar)[data-state="thinking"] .brows {
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

  .character:not(.raster-avatar)[data-state="writing"] {
    animation: writing-jitter 0.5s ease-in-out infinite;
  }

  @keyframes writing-jitter {
    0%, 100% { transform: translateX(0) rotate(0deg); }
    25% { transform: translateX(-1px) rotate(-1deg); }
    50% { transform: translateX(0) rotate(0deg); }
    75% { transform: translateX(1px) rotate(1deg); }
  }

  .character:not(.raster-avatar)[data-state="pasting"] {
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

  /* Speech bubble - anchored just above the character's head (--bubble-bottom,
     measured from the window bottom) and grows UPWARD as text gets longer, so
     it hugs the head at rest (no big gap) and never reaches the face. The box
     is sized per-skin with enough headroom above for a 2-line bubble. */
  .bubble {
    position: absolute;
    bottom: var(--bubble-bottom, 130px);
    top: auto;
    left: 50%;
    transform: translateX(-50%) translateY(6px) scale(0.92);
    /* Bubble scale is intentionally decoupled from avatar scale: small avatars
       need readable status text, large avatars need a tighter bubble. */
    box-sizing: border-box;
    max-width: calc(216px * var(--bubble-scale, 1));
    background: #fff;
    border: 1px solid rgba(0, 0, 0, 0.12);
    border-radius: calc(13px * var(--bubble-scale, 1));
    padding: calc(5px * var(--bubble-scale, 1)) calc(10px * var(--bubble-scale, 1));
    font-size: calc(10.5px * var(--bubble-scale, 1));
    color: #1d1d1f;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.12);
    opacity: 0;
    pointer-events: none;
    transition: opacity 200ms ease, transform 200ms cubic-bezier(0.34, 1.56, 0.64, 1);
    display: flex;
    align-items: center;
    gap: calc(6px * var(--bubble-scale, 1));
    font-family: ui-sans-serif, system-ui, -apple-system, "Segoe UI", sans-serif;
    z-index: 5;
  }
  /* Bubble text wraps inside the fixed-width bubble — HARD-CAPPED at two
     lines (the BUBBLE_BAND window-height math depends on this cap; anything
     longer ellipsizes). Copy in listenLabel() is written to fit. */
  .bubble-text {
    white-space: normal;
    line-height: 1.28;
    text-align: left;
    flex: 1 1 auto;
    min-width: 0;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .bubble.show {
    opacity: 1;
    transform: translateX(-50%) translateY(0) scale(1);
  }

  .bubble::after {
    content: "";
    position: absolute;
    bottom: calc(-5px * var(--bubble-scale, 1));
    left: 50%;
    transform: translateX(-50%) rotate(45deg);
    width: calc(8px * var(--bubble-scale, 1));
    height: calc(8px * var(--bubble-scale, 1));
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
  .bubble[data-skin="codex-fox"] {
    background: #f5fbff;
    color: #152033;
    border-color: rgba(72, 200, 255, 0.34);
    box-shadow: 0 4px 14px rgba(44, 148, 206, 0.18);
  }
  .bubble[data-skin="codex-fox"]::after {
    background: #f5fbff;
    border-right-color: rgba(72, 200, 255, 0.34);
    border-bottom-color: rgba(72, 200, 255, 0.34);
  }
  .bubble[data-skin="oru-gujia"] {
    background: #fff8ee;
    color: #2b2218;
    border-color: rgba(193, 125, 54, 0.24);
    box-shadow: 0 4px 12px rgba(120, 80, 30, 0.16);
  }
  .bubble[data-skin="oru-gujia"]::after {
    background: #fff8ee;
    border-right-color: rgba(193, 125, 54, 0.24);
    border-bottom-color: rgba(193, 125, 54, 0.24);
  }
  .bubble[data-skin="spark-buddy"] {
    background: #fffbe0;
    color: #302300;
    border-color: rgba(80, 214, 194, 0.42);
    box-shadow: 0 4px 14px rgba(80, 214, 194, 0.2);
  }
  .bubble[data-skin="spark-buddy"]::after {
    background: #fffbe0;
    border-right-color: rgba(80, 214, 194, 0.42);
    border-bottom-color: rgba(80, 214, 194, 0.42);
  }
  .bubble[data-skin="codex-fox"] .bubble-eq span,
  .bubble[data-skin="spark-buddy"] .bubble-eq span { background: #19c7d3; }
  .bubble[data-skin="oru-gujia"] .bubble-eq span { background: #d78633; }

  .bubble-text { font-weight: 500; }

  .bubble-eq {
    display: inline-flex;
    align-items: flex-end;
    gap: calc(2px * var(--bubble-scale, 1));
    height: calc(11px * var(--bubble-scale, 1));
  }

  .bubble-eq span {
    width: calc(2px * var(--bubble-scale, 1));
    background: #0a84ff;
    border-radius: calc(1px * var(--bubble-scale, 1));
    animation: eq-bar 0.7s ease-in-out infinite;
  }

  .bubble-eq span:nth-child(1) { animation-delay: 0s; height: calc(4px * var(--bubble-scale, 1)); }
  .bubble-eq span:nth-child(2) { animation-delay: 0.15s; height: calc(8px * var(--bubble-scale, 1)); }
  .bubble-eq span:nth-child(3) { animation-delay: 0.30s; height: calc(11px * var(--bubble-scale, 1)); }
  .bubble-eq span:nth-child(4) { animation-delay: 0.45s; height: calc(6px * var(--bubble-scale, 1)); }

  @keyframes eq-bar {
    0%, 100% { transform: scaleY(0.4); }
    50% { transform: scaleY(1); }
  }

  .bubble-dots {
    display: inline-flex;
    gap: calc(2px * var(--bubble-scale, 1));
  }

  .bubble-dots span {
    width: calc(4px * var(--bubble-scale, 1));
    height: calc(4px * var(--bubble-scale, 1));
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
    font-size: calc(11px * var(--bubble-scale, 1));
  }

  .bubble-emoji {
    display: inline-block;
    font-size: calc(13px * var(--bubble-scale, 1));
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
    max-width: calc(200px * var(--bubble-scale, 1));
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
    height: calc(168px * var(--fscale, 1) * var(--state-scale, 1));
    transition: width 320ms ease, height 320ms ease;
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
  .bubble[data-skin="cat"] {
    background: #2B2B2B;
    color: #e0e0e0;
    border-color: rgba(127, 255, 0, 0.2);
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
  }
  .bubble[data-skin="cat"]::after {
    background: #2B2B2B;
    border-right-color: rgba(127, 255, 0, 0.2);
    border-bottom-color: rgba(127, 255, 0, 0.2);
  }
  /* Override EQ bar color for dark bubble */
  .bubble[data-skin="cat"] .bubble-eq span { background: #7FFF00; }
  .bubble[data-skin="cat"] .bubble-dots span { background: #999; }

  /* ═══════════════════════════════════════════════════════════════════════
     KHAUMANI & INDY (duo) — animations
     ═══════════════════════════════════════════════════════════════════════ */

  .duo-skin {
    pointer-events: none;
    width: calc(198px * var(--fscale, 1) * var(--state-scale, 1));
    height: calc(117px * var(--fscale, 1) * var(--state-scale, 1));
    transition: width 320ms ease, height 320ms ease;
    filter: drop-shadow(0 4px 8px rgba(80, 60, 30, 0.18));
  }
  /* The generic .character idle-bob rotates the whole scene — looks wrong
     on a two-character tableau. The cats animate individually instead. */
  .character.duo-skin,
  .character.duo-skin[data-state="listening"],
  .character.duo-skin[data-state="thinking"],
  .character.duo-skin[data-state="writing"] {
    animation: none;
  }

  /* ── Idle life ── */
  .duo-white-body {
    transform-origin: 55px 113px;
    animation: duo-white-breathe 4.4s ease-in-out infinite;
  }
  .duo-white-head {
    transform-origin: 76px 88px;
    animation: duo-white-head-idle 4.4s ease-in-out infinite;
    transition: transform 300ms cubic-bezier(0.34, 1.4, 0.64, 1);
  }
  /* Right ear flicks once in a while — also answers Indy's 16s lean-over. */
  .duo-white-ear-r {
    transform-origin: 86px 52px;
    animation: duo-ear-flick 16s ease-in-out infinite;
  }
  .duo-white-tail {
    transform-origin: 30px 104px;
    animation: duo-tail-curl 9s ease-in-out infinite;
  }
  .duo-orange-body {
    transform-origin: 162px 124px;
    animation: duo-orange-sway 3.6s ease-in-out infinite;
  }
  .duo-orange-head {
    transform-origin: 163px 76px;
    animation: duo-orange-curious 9s ease-in-out infinite;
    transition: transform 280ms cubic-bezier(0.34, 1.4, 0.64, 1);
  }
  /* Indy periodically leans toward Khaumani — the "bother the supervisor"
     beat. Same 16s clock as the white ear-flick so they read as one event. */
  .duo-orange {
    transform-origin: 162px 124px;
    animation: duo-bother 16s ease-in-out infinite;
  }
  .duo-orange-tail {
    transform-origin: 186px 110px;
    animation: duo-orange-tail-sway 4.2s ease-in-out infinite;
  }

  @keyframes duo-white-breathe {
    0%, 100% { transform: scale(1, 1) translateY(0); }
    50%      { transform: scale(1.012, 0.985) translateY(-0.6px); }
  }
  @keyframes duo-white-head-idle {
    0%, 100% { transform: translateY(0); }
    50%      { transform: translateY(-1.2px); }
  }
  @keyframes duo-ear-flick {
    0%, 88%, 96%, 100% { transform: rotate(0deg); }
    90%                { transform: rotate(-16deg); }
    93%                { transform: rotate(6deg); }
  }
  @keyframes duo-tail-curl {
    0%, 84%, 100% { transform: rotate(0deg); }
    90%           { transform: rotate(-5deg) translateY(-1px); }
  }
  @keyframes duo-orange-sway {
    0%, 100% { transform: translateY(0) rotate(0deg); }
    50%      { transform: translateY(-1.4px) rotate(0.6deg); }
  }
  @keyframes duo-orange-curious {
    0%, 64%, 80%, 100% { transform: rotate(0deg); }
    70%                { transform: rotate(7deg) translateY(-1px); }
  }
  @keyframes duo-bother {
    0%, 84%, 95%, 100% { transform: translateX(0) rotate(0deg); }
    88%, 91%           { transform: translateX(-7px) rotate(-3deg); }
  }
  @keyframes duo-orange-tail-sway {
    0%, 100% { transform: rotate(0deg); }
    50%      { transform: rotate(-7deg); }
  }

  /* ── Listening: both perk up ── */
  .duo-skin[data-state="listening"] .duo-ears {
    transform-box: fill-box;
    transform-origin: 50% 90%;
    animation: duo-ears-perk 0.4s cubic-bezier(0.34, 1.56, 0.64, 1) both;
  }
  .duo-skin[data-state="listening"] .duo-orange,
  .duo-skin[data-state="listening"] .duo-white-ear-r {
    animation: none; /* pause the idle bother/flick clocks while attentive */
  }
  .duo-skin[data-state="listening"] .duo-orange-body {
    animation: duo-alert-bob 1.2s ease-in-out infinite;
  }
  .duo-skin[data-state="listening"] .duo-orange-head {
    animation: duo-alert-head 1.4s ease-in-out infinite;
  }
  .duo-skin[data-state="listening"] .duo-white-head {
    animation: none;
    transform: rotate(-3deg) translateY(-1.5px);
  }
  @keyframes duo-ears-perk {
    0%   { transform: scaleY(0.82) translateY(2px); }
    60%  { transform: scaleY(1.1); }
    100% { transform: scaleY(1) translateY(0); }
  }
  @keyframes duo-alert-bob {
    0%, 100% { transform: translateY(0); }
    50%      { transform: translateY(-2.5px); }
  }
  @keyframes duo-alert-head {
    0%, 100% { transform: translateY(0) rotate(0deg); }
    30%      { transform: translateY(-3px) rotate(-2deg); }
    70%      { transform: translateY(-2px) rotate(2deg); }
  }

  /* ── Thinking: white squints + dots, orange tilts ── */
  .duo-skin[data-state="thinking"] .duo-white-head {
    animation: duo-think-tilt 2.2s ease-in-out infinite;
  }
  .duo-skin[data-state="thinking"] .duo-orange-head {
    animation: duo-think-tilt-orange 2.2s ease-in-out infinite;
  }
  .duo-think-dots {
    animation: duo-dots-pop 420ms cubic-bezier(0.34, 1.56, 0.64, 1) both;
    transform-origin: 100px 35px;
  }
  @keyframes duo-think-tilt {
    0%, 100% { transform: rotate(0deg); }
    50%      { transform: rotate(4deg) translateY(-1px); }
  }
  @keyframes duo-think-tilt-orange {
    0%, 100% { transform: rotate(0deg); }
    50%      { transform: rotate(-6deg); }
  }
  @keyframes duo-dots-pop {
    0%   { transform: scale(0); opacity: 0; }
    60%  { transform: scale(1.12); opacity: 1; }
    100% { transform: scale(1); opacity: 1; }
  }

  /* ── Writing: orange types, white supervises ── */
  .duo-skin[data-state="writing"] .duo-orange-body {
    animation: duo-write-focus 0.6s ease-in-out infinite;
  }
  .duo-skin[data-state="writing"] .duo-paw-l {
    animation: duo-paw-tap 0.3s ease-in-out infinite;
  }
  .duo-skin[data-state="writing"] .duo-paw-r {
    animation: duo-paw-tap 0.3s ease-in-out infinite 0.15s;
  }
  .duo-skin[data-state="writing"] .duo-white-head {
    animation: none;
    transform: rotate(7deg) translateY(1px); /* gazing down at the worker */
  }
  .duo-skin[data-state="writing"] .duo-orange-tail {
    animation: duo-orange-tail-sway 0.9s ease-in-out infinite;
  }
  @keyframes duo-write-focus {
    0%, 100% { transform: translateY(0); }
    50%      { transform: translateY(-1px); }
  }
  @keyframes duo-paw-tap {
    0%, 100% { transform: translateY(0); }
    50%      { transform: translateY(-4px); }
  }

  /* ── Pasting: paw bump + sparkles ── */
  .duo-bump {
    transform-box: fill-box;
    transform-origin: center;
    animation: duo-bump-pop 480ms cubic-bezier(0.34, 1.56, 0.64, 1) both;
  }
  .duo-sparkles {
    animation: duo-sparkle-fade 900ms ease-out 200ms both;
  }
  .duo-skin[data-state="pasting"] .duo-white-body,
  .duo-skin[data-state="pasting"] .duo-orange-body {
    animation: duo-paste-bounce 0.5s cubic-bezier(0.34, 1.56, 0.64, 1) both;
  }
  @keyframes duo-bump-pop {
    0%   { transform: scale(0.4); opacity: 0; }
    60%  { transform: scale(1.08); opacity: 1; }
    100% { transform: scale(1); opacity: 1; }
  }
  @keyframes duo-sparkle-fade {
    0%   { transform: translateY(0); opacity: 1; }
    100% { transform: translateY(-7px); opacity: 0; }
  }
  @keyframes duo-paste-bounce {
    0%   { transform: translateY(0) scale(1); }
    40%  { transform: translateY(-7px) scale(1.04, 0.96); }
    100% { transform: translateY(0) scale(1); }
  }

  /* Duo bubble — same warm cream family as the fox (the duo lives in the
     same cottage). */
  .bubble[data-skin="duo"],
  .bubble[data-skin="duo-hd"] {
    background: #faf6ec;
    color: #2b2218;
    border-color: rgba(120, 80, 30, 0.18);
    box-shadow: 0 4px 12px rgba(120, 80, 30, 0.18);
  }
  .bubble[data-skin="duo"]::after,
  .bubble[data-skin="duo-hd"]::after {
    background: #faf6ec;
    border-right-color: rgba(120, 80, 30, 0.18);
    border-bottom-color: rgba(120, 80, 30, 0.18);
  }

  /* ═══════════════════════════════════════════════════════════════════════
     KHAUMANI & INDY ✦ (duo-hd) — the remastered duo. SCENE-DIRECTOR model:
     one 22s master clock (.hd-* idle animations all share it) choreographs a
     looping vignette — rest → Indy pounces left toward Khaumani → she reacts
     with a slow blink → Indy hops back → groom → rest. Pipeline states
     ([data-state]) pause the scene clock and take over with their own beats.
     All motion is transforms only (GPU-cheap). Origins are in user units to
     match the rest of this file's convention.
     ═══════════════════════════════════════════════════════════════════════ */
  .hd-skin {
    pointer-events: none;
    width: calc(200px * var(--fscale, 1) * var(--state-scale, 1));
    height: calc(130px * var(--fscale, 1) * var(--state-scale, 1));
    transition: width 320ms ease, height 320ms ease;
    filter: drop-shadow(0 5px 9px rgba(80, 60, 30, 0.2));
  }
  /* The generic whole-body idle-bob looks wrong on a two-cat tableau. */
  .character.hd-skin,
  .character.hd-skin[data-state="listening"],
  .character.hd-skin[data-state="thinking"],
  .character.hd-skin[data-state="writing"] {
    animation: none;
  }

  /* ── Ambient backdrop ── */
  .hd-sunglow { animation: hd-sun-pulse 7s ease-in-out infinite; transform-box: fill-box; transform-origin: center; }
  .hd-mote-1 { animation: hd-mote-a 9s linear infinite; }
  .hd-mote-2 { animation: hd-mote-b 11s linear infinite 1.5s; }
  .hd-mote-3 { animation: hd-mote-a 13s linear infinite 3s; }
  @keyframes hd-sun-pulse { 0%, 100% { opacity: 0.92; } 50% { opacity: 1; } }
  @keyframes hd-mote-a { 0% { transform: translate(0,0); opacity: 0; } 20% { opacity: 0.7; } 100% { transform: translate(-13px, 22px); opacity: 0; } }
  @keyframes hd-mote-b { 0% { transform: translate(0,0); opacity: 0; } 25% { opacity: 0.6; } 100% { transform: translate(10px, 26px); opacity: 0; } }

  /* ── Khaumani idle life ── */
  .hd-khao-body { transform-origin: 74px 134px; animation: hd-khao-breathe 4.6s ease-in-out infinite; }
  .hd-khao-tail { transform-origin: 34px 120px; animation: hd-khao-tail 8s ease-in-out infinite; }
  .hd-khao-head { transform-origin: 79px 104px; animation: hd-khao-react 22s ease-in-out infinite; }
  .hd-khao-ear-r { transform-origin: 96px 62px; animation: hd-khao-ear 22s ease-in-out infinite; }
  .hd-khao-lids { transform-box: fill-box; transform-origin: 50% 0%; animation: hd-khao-loveblink 22s ease-in-out infinite; }
  @keyframes hd-khao-breathe { 0%, 100% { transform: scale(1, 1); } 50% { transform: scale(1.012, 0.985) translateY(-0.5px); } }
  @keyframes hd-khao-tail { 0%, 100% { transform: rotate(0deg); } 50% { transform: rotate(-3deg); } }
  @keyframes hd-khao-react {
    0%, 57%, 78%, 100% { transform: rotate(0deg) translateY(0); }
    61% { transform: rotate(5deg); }
    70% { transform: rotate(6deg) translateY(-1px); }
  }
  @keyframes hd-khao-ear {
    0%, 63%, 69%, 90%, 95%, 100% { transform: rotate(0deg); }
    66% { transform: rotate(-15deg); }
    67.5% { transform: rotate(5deg); }
    92% { transform: rotate(-11deg); }
    93.5% { transform: rotate(4deg); }
  }
  @keyframes hd-khao-loveblink {
    0%, 64%, 72%, 100% { transform: scaleY(0.05); }
    66.5%, 69.5% { transform: scaleY(1); }
  }

  /* ── Indy idle life + the POUNCE (scene clock) ── */
  .hd-indy-jumper { animation: hd-pounce 22s ease-in-out infinite; }
  .hd-indy-squashbox { transform-origin: 168px 136px; animation: hd-squash 22s ease-in-out infinite; }
  .hd-indy-castshadow { transform-box: fill-box; transform-origin: center; animation: hd-indy-shadow 22s ease-in-out infinite; }
  .hd-indy-body { transform-origin: 168px 136px; animation: hd-indy-breathe 3.4s ease-in-out infinite; }
  .hd-indy-head { transform-origin: 170px 94px; animation: hd-indy-head-idle 9s ease-in-out infinite; transition: transform 260ms cubic-bezier(0.34, 1.4, 0.64, 1); }
  .hd-indy-tail { transform-origin: 196px 122px; animation: hd-indy-tail 4s ease-in-out infinite; }
  @keyframes hd-pounce {
    0%, 52% { transform: translate(0, 0); }
    55% { transform: translate(3px, 4px); }
    58% { transform: translate(-22px, -12px); }
    61% { transform: translate(-44px, -7px); }
    64% { transform: translate(-54px, 2px); }
    66%, 76% { transform: translate(-54px, -1px); }
    79% { transform: translate(-26px, -10px); }
    82% { transform: translate(-2px, 2px); }
    84%, 100% { transform: translate(0, 0); }
  }
  @keyframes hd-squash {
    0%, 53%, 67%, 78%, 84%, 100% { transform: scale(1, 1); }
    55% { transform: scale(1.1, 0.86); }
    58% { transform: scale(0.9, 1.16); }
    61% { transform: scale(0.92, 1.12); }
    64% { transform: scale(1.12, 0.84); }
    79% { transform: scale(0.9, 1.14); }
    82% { transform: scale(1.1, 0.88); }
  }
  @keyframes hd-indy-shadow {
    0%, 53%, 67%, 76%, 84%, 100% { transform: translateX(0) scale(1); }
    58% { transform: translateX(-22px) scale(0.74); }
    64% { transform: translateX(-54px) scale(0.92); }
    66% { transform: translateX(-54px) scale(1); }
    79% { transform: translateX(-26px) scale(0.8); }
    82% { transform: translateX(-2px) scale(1); }
  }
  @keyframes hd-indy-breathe { 0%, 100% { transform: scale(1, 1); } 50% { transform: scale(1.014, 0.984) translateY(-0.5px); } }
  @keyframes hd-indy-head-idle {
    0%, 60%, 82%, 100% { transform: rotate(0deg); }
    71% { transform: rotate(-7deg) translateY(-1px); }
  }
  @keyframes hd-indy-tail { 0%, 100% { transform: rotate(0deg); } 50% { transform: rotate(-8deg); } }

  /* ── Listening: both snap alert ── */
  .hd-skin[data-state="listening"] .hd-indy-jumper,
  .hd-skin[data-state="listening"] .hd-indy-squashbox,
  .hd-skin[data-state="listening"] .hd-indy-castshadow,
  .hd-skin[data-state="listening"] .hd-khao-ear-r { animation: none; }
  .hd-skin[data-state="listening"] .hd-khao-head { animation: none; transform: translateY(-2px) rotate(-2deg); }
  .hd-skin[data-state="listening"] .hd-khao-lids,
  .hd-skin[data-state="thinking"] .hd-khao-lids,
  .hd-skin[data-state="writing"] .hd-khao-lids,
  .hd-skin[data-state="pasting"] .hd-khao-lids { animation: none; transform: scaleY(0.05); }
  .hd-skin[data-state="listening"] .hd-khao-ear-l,
  .hd-skin[data-state="listening"] .hd-khao-ear-r,
  .hd-skin[data-state="listening"] .hd-indy-ear-l,
  .hd-skin[data-state="listening"] .hd-indy-ear-r {
    transform-box: fill-box; transform-origin: 50% 100%;
    animation: hd-perk 0.4s cubic-bezier(0.34, 1.56, 0.64, 1) both;
  }
  .hd-skin[data-state="listening"] .hd-indy-body { animation: hd-alert-bob 1.2s ease-in-out infinite; }
  .hd-skin[data-state="listening"] .hd-indy-head { animation: hd-alert-head 1.4s ease-in-out infinite; }
  .hd-skin[data-state="listening"] .hd-indy-tail { animation: hd-indy-tail 1.6s ease-in-out infinite; }
  @keyframes hd-perk { 0% { transform: scaleY(0.82) translateY(2px); } 60% { transform: scaleY(1.1); } 100% { transform: scaleY(1); } }
  @keyframes hd-alert-bob { 0%, 100% { transform: translateY(0); } 50% { transform: translateY(-2.5px); } }
  @keyframes hd-alert-head { 0%, 100% { transform: translateY(0) rotate(0deg); } 30% { transform: translateY(-3px) rotate(-2deg); } 70% { transform: translateY(-2px) rotate(2deg); } }

  /* ── Thinking: Indy head-tilts, Khaumani watches ── */
  .hd-skin[data-state="thinking"] .hd-indy-jumper,
  .hd-skin[data-state="thinking"] .hd-indy-squashbox,
  .hd-skin[data-state="thinking"] .hd-indy-castshadow,
  .hd-skin[data-state="thinking"] .hd-khao-ear-r { animation: none; }
  .hd-skin[data-state="thinking"] .hd-indy-head { animation: hd-think-tilt 2.2s ease-in-out infinite; }
  .hd-skin[data-state="thinking"] .hd-khao-head { animation: none; transform: rotate(4deg); }
  .hd-indy-thinkdots { animation: hd-dots-pop 420ms cubic-bezier(0.34, 1.56, 0.64, 1) both; transform-origin: 200px 40px; }
  @keyframes hd-think-tilt { 0%, 100% { transform: rotate(0deg); } 50% { transform: rotate(-7deg); } }
  @keyframes hd-dots-pop { 0% { transform: scale(0); opacity: 0; } 60% { transform: scale(1.12); opacity: 1; } 100% { transform: scale(1); opacity: 1; } }

  /* ── Writing: Indy bats at the air, Khaumani supervises ── */
  .hd-skin[data-state="writing"] .hd-indy-jumper,
  .hd-skin[data-state="writing"] .hd-indy-squashbox,
  .hd-skin[data-state="writing"] .hd-indy-castshadow { animation: none; }
  .hd-skin[data-state="writing"] .hd-indy-body { animation: hd-write-focus 0.6s ease-in-out infinite; }
  .hd-skin[data-state="writing"] .hd-indy-paw-l { transform-box: fill-box; transform-origin: 50% 0%; animation: hd-paw-tap 0.28s ease-in-out infinite; }
  .hd-skin[data-state="writing"] .hd-indy-paw-r { transform-box: fill-box; transform-origin: 50% 0%; animation: hd-paw-tap 0.28s ease-in-out infinite 0.14s; }
  .hd-skin[data-state="writing"] .hd-indy-tail { animation: hd-indy-tail 0.85s ease-in-out infinite; }
  .hd-skin[data-state="writing"] .hd-indy-head { animation: none; transform: rotate(-3deg) translateY(1px); }
  .hd-skin[data-state="writing"] .hd-khao-head { animation: none; transform: rotate(7deg) translateY(1px); }
  @keyframes hd-write-focus { 0%, 100% { transform: translateY(0); } 50% { transform: translateY(-1px); } }
  @keyframes hd-paw-tap { 0%, 100% { transform: translateY(0); } 50% { transform: translateY(-5px); } }

  /* ── Pasting: happy double-hop + nose-boop + sparkles ── */
  .hd-skin[data-state="pasting"] .hd-indy-squashbox,
  .hd-skin[data-state="pasting"] .hd-indy-castshadow,
  .hd-skin[data-state="pasting"] .hd-khao-head,
  .hd-skin[data-state="pasting"] .hd-khao-ear-r { animation: none; }
  .hd-skin[data-state="pasting"] .hd-indy-jumper { animation: hd-boop 0.62s ease-out both; }
  .hd-skin[data-state="pasting"] .hd-khao-body { animation: hd-paste-hop 0.55s cubic-bezier(0.34, 1.56, 0.64, 1) both; }
  .hd-skin[data-state="pasting"] .hd-indy-body { animation: hd-paste-hop 0.55s cubic-bezier(0.34, 1.56, 0.64, 1) both 0.06s; }
  .hd-sparkles { animation: hd-sparkle 900ms ease-out 150ms both; }
  @keyframes hd-boop { 0% { transform: translate(0, 0); } 45% { transform: translate(-30px, 1px); } 100% { transform: translate(0, 0); } }
  @keyframes hd-paste-hop { 0% { transform: translateY(0) scale(1); } 40% { transform: translateY(-8px) scale(1.04, 0.96); } 100% { transform: translateY(0) scale(1); } }
  @keyframes hd-sparkle { 0% { transform: translateY(0) scale(0.6); opacity: 0; } 30% { opacity: 1; } 100% { transform: translateY(-8px) scale(1.1); opacity: 0; } }

  /* X dismiss button was removed — double-click Clippy to open the main
     window instead. Hide via tray → Toggle Clippy. */
</style>
