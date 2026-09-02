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
  import SpritePet from "$lib/SpritePet.svelte";
  import { PET_SKINS, isPetSkin, petIdFromSkin, type PetAnimName } from "$lib/pets";
  import RasterAvatar from "$lib/RasterAvatar.svelte";
  import { RASTER_AVATAR_ART, isRasterAvatarSkin } from "$lib/avatar-packs";
  import { api, type FlowSnapshot } from "$lib/api";

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
  type Mode = "light" | "advanced" | "drafting";

  // `flowState` is the *actual* flow state from Rust (changes fast during pipeline).
  // `displayState` is what Clippy is currently animating — it lags `flowState`
  // through a queue so each post-listening animation gets at least MIN_DWELL_MS
  // of airtime regardless of how fast the backend finishes transcribing /
  // cleaning / injecting.
  // The backend can legitimately spend 120s in STT plus a cleanup request.
  // This watchdog sits beyond that budget and is diagnostic only: it asks the
  // backend for a fresh snapshot and never invents an idle transition.
  const WATCHDOG_MS = 300_000;
  let watchdogTimer: ReturnType<typeof setTimeout> | null = null;
  let lastRevision = -1;
  let activeSessionId: string | null = null;
  let lastNoticeKey = "";
  function armWatchdog() {
    if (watchdogTimer) clearTimeout(watchdogTimer);
    const armedRevision = lastRevision;
    watchdogTimer = setTimeout(async () => {
      watchdogTimer = null;
      console.warn("[clippy] lifecycle watchdog fired; requesting backend snapshot");
      try {
        applyFlowSnapshot(await api.getFlowSnapshot());
      } catch (e) {
        console.warn("[clippy] snapshot resync failed", e);
      }
      if (lastRevision === armedRevision && flowState !== "idle" && flowState !== "listening") {
        showToast("Still working on the previous dictation…", "info", 5000);
        armWatchdog();
      }
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
  const ERROR_TOAST_MIN_MS = 15_000;
  function showToast(msg: string, kind: "info" | "error" = "info", durationMs = 3000) {
    toastMessage = msg;
    toastKind = kind;
    if (toastTimer) clearTimeout(toastTimer);
    const visibleFor = kind === "error" ? Math.max(durationMs, ERROR_TOAST_MIN_MS) : durationMs;
    toastTimer = setTimeout(() => {
      toastMessage = "";
      toastTimer = null;
    }, visibleFor);
  }
  function dismissToast() {
    if (toastTimer) clearTimeout(toastTimer);
    toastTimer = null;
    toastMessage = "";
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
    off:           { w: 116, h: 116, head: 110 },
    // Minimal skins have no normal status-bubble anchor. Terminal errors use
    // their rendered height as an explicit temporary anchor (see boxFor).
    // Wave bar: small Apple-style pill (~Clippy width). Siri orb: tiny circle.
    wave:          { w: 120, h: 32,  head: 0 },
    siri:          { w: 58,  h: 58,  head: 0 },
    ...RASTER_AVATAR_ART,
  };
  // Terminal pets: 192×208 frames at SpritePet's 0.58 zoom → ~111×121.
  // One shared box; keep in sync with PET_ZOOM in SpritePet.svelte.
  for (const s of PET_SKINS) ART[s] = { w: 112, h: 122, head: 112 };
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

  // Minimal skins (wave bar, Siri orb): no normal status/info bubbles, quips,
  // or floor shadow. Terminal errors are the sole readable-bubble exception.
  const MINIMAL_SKINS = new Set(["wave", "siri"]);
  function isMinimalSkin(s: string): boolean {
    return MINIMAL_SKINS.has(s);
  }

  function boxFor(
    skin: string,
    talking: boolean,
    avatarScale: number,
    bubbleScale: number,
    bubbleBand = BUBBLE_BAND,
    showMinimalError = false,
    bubbleW = BUBBLE_W,
  ): Size {
    const a = ART[skin] ?? ART.fox;
    const rest: Size = {
      w: Math.ceil((a.w + 2 * SIDE_PAD) * avatarScale),
      h: Math.ceil((a.h + BOTTOM_PAD + TOP_MARGIN) * avatarScale),
    };
    // Minimal skins normally stay in the tight REST box. An explicit terminal
    // error gets a bounded reading box above the minimal art.
    if (isMinimalSkin(skin) && !showMinimalError) return rest;
    if (!talking) return rest;
    const bubbleAnchor = isMinimalSkin(skin) && showMinimalError
      ? a.h + BOTTOM_PAD
      : a.head;
    return {
      w: Math.max(rest.w, Math.ceil(bubbleW * bubbleScale)),
      h: Math.max(
        rest.h,
        Math.ceil(bubbleAnchor * avatarScale + bubbleGapFor(avatarScale) + bubbleBand * bubbleScale),
      ),
    };
  }

  // A bubble is "up" when either the state bubble or a transient toast is
  // showing. Growing happens immediately (the window must be tall before the
  // bubble's 200ms fade-in paints); shrinking waits a beat so the fade-OUT
  // finishes inside the still-tall window and quick state churn (e.g.
  // thinking→writing) never causes a shrink-grow stutter.
  // Minimal skins suppress normal status/info bubbles. Terminal errors are the
  // exception: they must remain readable regardless of the selected avatar.
  let terminalErrorUp = $derived(toastMessage !== "" && toastKind === "error");
  let bubbleUp = $derived(
    terminalErrorUp ||
      (!isMinimalSkin(skin) &&
        (toastMessage !== "" || hoverQuip !== "" || (skin !== "off" && displayState !== "idle"))),
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
  // Terminal errors get a monitor-friendly, bounded reading area. Reserve the
  // whole safe band up front instead of guessing line count from character
  // count: proportional fonts, URLs, and long tokens make that guess unsafe.
  // CSS scrolls only the text beyond the cap, so every byte remains reachable
  // without letting an always-on-top floater grow without bound.
  // BUBBLE_W (226) is deliberately narrow for STATUS bubbles — the user does
  // not want a wide window during normal dictation. Error prose is a different
  // job: at 226px a real sentence wrapped to 2-3 words per line, ran ~10 lines,
  // and STILL scrolled. Errors therefore get their own, much wider box; it only
  // exists while an error is actually on screen, so the resting floater is
  // unaffected.
  const ERROR_BUBBLE_W = 430;
  const ERROR_TEXT_MAX_H = 260;
  const ERROR_BUBBLE_BAND = ERROR_TEXT_MAX_H + 28;
  let bubbleBand = $derived(terminalErrorUp ? ERROR_BUBBLE_BAND : BUBBLE_BAND);
  let bubbleWidth = $derived(terminalErrorUp ? ERROR_BUBBLE_W : BUBBLE_W);

  // Where the bubble's tail sits (logical px from window bottom), scaled.
  // The bubble anchors here and grows upward. v2.0.0 keeps a minimum physical
  // gap so the larger small-size bubble no longer lands on the avatar's face.
  let bubbleBottom = $derived.by(() => {
    const art = ART[skin] ?? ART.fox;
    const anchor = isMinimalSkin(skin) && terminalErrorUp
      ? art.h + BOTTOM_PAD
      : art.head;
    return anchor * fscale + bubbleGap;
  });

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
    const box = boxFor(
      skin,
      fixedBox || talking || terminalErrorUp,
      fscale,
      bubbleScale,
      bubbleBand,
      terminalErrorUp,
      bubbleWidth,
    );
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
    const map = m === "light" ? ANIMS_LIGHT : ANIMS_ADVANCED;
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

  function snapshotDisplay(snapshot: FlowSnapshot): ClippyState {
    if (snapshot.phase === "starting" || snapshot.phase === "recording") {
      return "listening";
    }
    if (snapshot.phase === "stopping") return "thinking";
    if (snapshot.phase !== "processing") return "idle";
    if (snapshot.stage === "cleaning") return "writing";
    if (snapshot.stage === "injecting") return "pasting";
    return "thinking";
  }

  function flushTransientStory(target: ClippyState) {
    displayQueue = [];
    if (displayTimer) {
      clearTimeout(displayTimer);
      displayTimer = null;
    }
    if (toastTimer) {
      clearTimeout(toastTimer);
      toastTimer = null;
    }
    toastMessage = "";
    displayState = target;
    flowState = target;
    denoising = false;
    activeApp = "";
    sttProvider = "";
    llmProvider = "";
  }

  function applyFlowSnapshot(snapshot: FlowSnapshot) {
    if (!snapshot || !Number.isFinite(snapshot.revision) || snapshot.revision <= lastRevision) {
      return;
    }

    const next = snapshotDisplay(snapshot);
    const isNewSession =
      snapshot.session_id !== null && snapshot.session_id !== activeSessionId;
    lastRevision = snapshot.revision;

    if (isNewSession) {
      activeSessionId = snapshot.session_id;
      lastNoticeKey = "";
      flushTransientStory(next);
    } else {
      flowState = next;
    }

    if (snapshot.mode) mode = snapshot.mode;
    denoising = snapshot.phase === "processing" && snapshot.stage === "denoising";
    micWaiting =
      (snapshot.phase === "starting" ||
        snapshot.phase === "recording" ||
        snapshot.phase === "stopping") &&
      snapshot.mic === "waking";
    micReadyMs = snapshot.mic_ready_ms;

    if (next === "idle" || next === "listening") {
      sttProvider = "";
      llmProvider = "";
      disarmWatchdog();
    } else {
      armWatchdog();
    }

    if (snapshot.phase === "failed") triggerWaveError();

    const noticeKey = snapshot.notice
      ? `${snapshot.session_id ?? "none"}:${snapshot.revision}:${snapshot.notice.code}`
      : "";
    if (snapshot.notice && noticeKey !== lastNoticeKey) {
      lastNoticeKey = noticeKey;
      showToast(
        snapshot.notice.summary,
        snapshot.notice.severity,
        snapshot.notice.severity === "error" ? 7000 : 3500,
      );
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

    let unlistenSnapshot: (() => void) | undefined;
    let unlistenMsg: (() => void) | undefined;
    let unlistenActiveApp: (() => void) | undefined;
    let unlistenSttProv: (() => void) | undefined;
    let unlistenLlmProv: (() => void) | undefined;
    let unlistenWarn: (() => void) | undefined;
    let unlistenLevel: (() => void) | undefined;
    let unlistenFarewell: (() => void) | undefined;
    let unlistenSpaceChanged: (() => void) | undefined;

    // Subscribe first, then rehydrate. If an event wins the race, revision
    // filtering makes the older command response a harmless no-op.
    listen<FlowSnapshot>("wispr:flow_snapshot", (e) => {
      applyFlowSnapshot(e.payload);
    }).then((u) => {
      unlistenSnapshot = u;
      void api.getFlowSnapshot()
        .then(applyFlowSnapshot)
        .catch((e) => console.warn("[clippy] initial flow snapshot failed", e));
    });

    listen<string>("wispr:clippy_message", (e) => {
      console.log("[clippy] wispr:clippy_message", e.payload);
      showToast(e.payload, "info", 3000);
    }).then((u) => (unlistenMsg = u));
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
    // an error-styled toast WITHOUT resetting pipeline state. This is not a
    // terminal snapshot notice: the run actually succeeded and delivered text.
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
    // App is quitting (tray → Quit). Rust emits this and waits ~420ms before
    // app.exit() so the avatar's per-skin EXIT animation finishes on screen —
    // the same warm farewell whether it's auto-hide or the final goodbye.
    listen("wispr:farewell", () => {
      void playExit();
    }).then((u) => (unlistenFarewell = u));

    // The user swiped to another Space and Rust has just moved the floater
    // onto it. Play the gentle "landed here" settle so the hop reads as
    // deliberate — the avatar following you — rather than a silent teleport.
    listen("wispr:space_changed", () => {
      playLand();
    }).then((u) => (unlistenSpaceChanged = u));

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
    // Resync state the instant the floater regains visibility/focus.
    //
    // Platform split for the repaint: on WINDOWS, visibilitychange fires on
    // minimize/restore and resume — a WebView2 surface that died in sleep
    // needs the force_repaint nudge here, and the window was hidden anyway so
    // the nudge is invisible. On MACOS the floater now rides along on every
    // Space switch, so "became visible" fires on every arrival — and
    // force_repaint is a hide()→show() cycle, which made the fox visibly
    // blink out and back on each swipe (user-reported, twice). macOS keeps
    // its dead-surface cover from the clock-gap watcher above, the Rust
    // resume detector, and the 30s watchdog's stale-heartbeat repaint.
    const IS_MAC = navigator.userAgent.includes("Macintosh");
    const resyncFlow = () => {
      void api.getFlowSnapshot()
        .then(applyFlowSnapshot)
        .catch((e) => console.warn("[clippy] flow snapshot resync failed", e));
    };
    const onVisible = () => {
      if (document.visibilityState === "visible") {
        if (!IS_MAC) recoverFloater("became visible");
        resyncFlow();
      }
    };
    const onFocus = () => {
      if (!IS_MAC) recoverFloater("regained focus");
      resyncFlow();
    };
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
      unlistenSnapshot?.();
      unlistenMsg?.();
      unlistenActiveApp?.();
      unlistenSttProv?.();
      unlistenLlmProv?.();
      unlistenWarn?.();
      unlistenLevel?.();
      unlistenFarewell?.();
      unlistenSpaceChanged?.();
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
  // True only while the Rust denoise stage is actually running (between the
  // "denoising" and "transcribing" state events).
  let denoising = $state(false);

  // ── Mic handover ("hold on") ────────────────────────────────────────────
  // True from hotkey-down until the audio layer reports its first buffer.
  // `micWaitElapsed` drives escalating copy: a fast mic never gets past the
  // first tier, a Bluetooth mic sits in the later ones long enough for the
  // user to understand that the delay is the DEVICE, not the app hanging.
  let micWaiting = $state(false);
  let micReadyMs = $state<number | null>(null);
  let micWaitElapsed = $state(0);
  $effect(() => {
    if (!micWaiting || displayState !== "listening") return;
    micWaitElapsed = 0;
    const t = setInterval(() => { micWaitElapsed += 1; }, 1000);
    return () => clearInterval(t);
  });

  function waitLabel(secs: number): string {
    if (secs < 2) return "hold on — waking the mic…";
    if (secs < 4) return "hold on — mic still starting…";
    if (secs < 8) return "still connecting the mic — don't start yet";
    return "mic is taking a while — check it's on and connected";
  }

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
    // Waiting on the mic takes over the listening label entirely — saying
    // "listening…" while no audio is arriving is the lie this whole feature
    // exists to stop telling.
    listening: micWaiting
      ? waitLabel(micWaitElapsed)
      : listenLabel(listenElapsed, activeApp),
    thinking: denoising
      ? "clearing noise…"
      : sttProvider ? `transcribing · ${prettyProvider(sttProvider)}` : runLines.think,
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
  const IDLE_QUIPS_CODEX_FOX = [
    "Codex fox online",
    "blue scarf, clean paste",
    "say it. I'll tidy the edges",
    "ears calibrated for accents",
    "F8 and I pounce",
  ];
  const IDLE_QUIPS_ORU_GUJIA = [
    "Gujia supervises. Uru touches everything",
    "two cats, one keyboard",
    "Uru heard F8 and arrived upside down",
    "Gujia has reviewed this silence",
    "cream tax payable in words",
    "we caught the thought. mostly",
  ];
  const IDLE_QUIPS_PETS = [
    "beep. ready when you are",
    "your words feed the pet",
    "I run on dictation",
    "press F8, I'll do a little dance",
    "pixel ears, real listening",
    "idle animation, active heart",
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
      isPetSkin(skin) ? IDLE_QUIPS_PETS :
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
  // A minimal Wispr-Flow-style pill. No routine text/bubbles/quips; terminal
  // errors temporarily use the shared accessible error bubble.
  // Levels arrive from Rust via `wispr:level` (f32 0..1, ~90ms while
  // recording). Each bar expands/contracts INDIVIDUALLY around the pill's
  // vertical center — height = live mic level × that bar's own slowly-
  // drifting modulation — so the bars dance like Wispr Flow's, rather than
  // scrolling a history left→right.
  const WAVE_BARS = 12;
  // Per-tick smoothing: fast attack so speech onsets snap, slower release so
  // bars fall gracefully instead of flickering.
  const WAVE_ATTACK = 0.5;
  const WAVE_RELEASE = 0.16;
  // How long the success (green) breathe lasts, and the error (red) blink.
  const WAVE_SUCCESS_MS = 900;
  const WAVE_ERROR_MS = 900;

  // Per-bar "personality": each bar mixes two sine drifts with its own
  // frequencies and phases, so no two bars ever move in lockstep. Purely
  // decorative, so fresh randoms per launch are fine.
  const WAVE_SEEDS = Array.from({ length: WAVE_BARS }, () => ({
    f1: 110 + Math.random() * 90,
    f2: 210 + Math.random() * 160,
    p1: Math.random() * Math.PI * 2,
    p2: Math.random() * Math.PI * 2,
  }));

  // Displayed bar heights, 0..1 (0 = resting dot, 1 = full allowed height).
  let waveHeights = $state<number[]>(new Array(WAVE_BARS).fill(0));
  // Newest raw level from Rust (0..1); micLevel chases it with attack/decay.
  let liveLevel = 0;
  let waveSuccessUntil = 0;
  let waveRaf: number | null = null;
  // Smoothed 0..1 mic level — drives the bars' amplitude and the Siri orb's
  // bloom. Decays to 0 when no levels arrive so both settle between dictations.
  let micLevel = $state(0);

  function pushLevel(v: number) {
    liveLevel = Math.max(0, Math.min(1, v));
  }

  // Compute the target bar heights from the current display state. Runs each
  // rAF tick so listening/processing/idle motion stays animated without timers.
  function computeWaveTargets(now: number): number[] {
    const s = displayState;
    const out = new Array(WAVE_BARS);
    if (s === "listening") {
      // Voice-reactive: every bar is the smoothed mic level shaped by its own
      // drifting modulation, center-weighted so the pill reads as a waveform.
      for (let i = 0; i < WAVE_BARS; i++) {
        const pos = i / (WAVE_BARS - 1);
        const seed = WAVE_SEEDS[i];
        const drift =
          0.5 + 0.5 * Math.sin(now / seed.f1 + seed.p1) * Math.sin(now / seed.f2 + seed.p2);
        const envelope = 0.35 + 0.65 * Math.sin(pos * Math.PI);
        // tanh soft-knee: quiet speech stays lively, loud peaks compress
        // instead of pinning every bar at the ceiling.
        out[i] = Math.tanh(micLevel * (0.4 + 1.2 * drift) * envelope * 2.4);
      }
      return out;
    }
    if (s === "thinking" || s === "writing") {
      // Processing: a soft pulse travels left→right on loop — clearly "working",
      // clearly not a level meter.
      const c = (now / 1100) % 1;
      for (let i = 0; i < WAVE_BARS; i++) {
        const pos = i / (WAVE_BARS - 1);
        let d = Math.abs(pos - c);
        if (d > 0.5) d = 1 - d; // wrap so the pulse loops seamlessly
        out[i] = 0.1 + 0.55 * Math.exp(-(d * d) / (2 * 0.1 * 0.1));
      }
      return out;
    }
    // idle / pasting: rest as a row of still dots with a barely-there breath.
    const breathe = 0.02 + 0.03 * (0.5 + 0.5 * Math.sin(now / 1400));
    return out.fill(breathe);
  }

  function waveTick(now: number) {
    // Smoothed mic level: chase the newest Rust level while listening
    // (fast up, slow down), decay to 0 otherwise.
    const raw = displayState === "listening" ? liveLevel : 0;
    micLevel = micLevel + (raw - micLevel) * (raw > micLevel ? WAVE_ATTACK : 0.1);
    const targets = computeWaveTargets(now);
    const next = new Array(WAVE_BARS);
    for (let i = 0; i < WAVE_BARS; i++) {
      const cur = waveHeights[i] ?? 0;
      const k = targets[i] > cur ? WAVE_ATTACK : WAVE_RELEASE;
      next[i] = cur + (targets[i] - cur) * k;
    }
    waveHeights = next;
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

  // Error (red blink ×3) flag — fired from a failed flow snapshot.
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
      liveLevel = 0;
      waveHeights = new Array(WAVE_BARS).fill(0);
      micLevel = 0;
    }
  });

  // ── Terminal pets: floater state → sprite animation ─────────────────────
  // The Codex pet sheets map almost 1:1 onto our pipeline states. Error rides
  // the shared waveError flag (set by the failed snapshot) so a failed
  // run shows the sad row for a beat; hover while idle gets a playful hop.
  let petAnim = $derived<PetAnimName>(
    waveError ? "sad" :
    displayState === "listening" ? "waving" :
    displayState === "thinking" ? "waiting" :
    displayState === "writing" ? "typing" :
    displayState === "pasting" ? "celebration" :
    hovering ? "jumping" :
    "idle",
  );

  // ── Enter / exit arrival animations ─────────────────────────────────────
  // The avatar always "arrives" warmly (the old Clippy touch). In `auto` mode
  // these bracket each show/hide; in `always` mode the enter plays once on
  // first mount so the avatar pops in rather than blinking into existence.
  const ENTER_MS = 380;
  const EXIT_MS = 240;
  // "Landing" on a new Space: the window is ALREADY visible (Rust just moved
  // it to the Space the user swiped to), so this is a gentle settle — a small
  // scale/lift bounce — NOT the from-nothing "enter". Kept short so it reads as
  // "it hopped over here", not a full re-entrance.
  const LAND_MS = 460;
  // After the flow returns to idle, wait this long (letting the success/status
  // bubble finish) before playing the exit + hiding in `auto` mode.
  const AUTO_HIDE_GRACE_MS = 1800;

  let arriveClass = $state<"enter" | "exit" | "land" | "">("");
  let _enterTimer: ReturnType<typeof setTimeout> | null = null;
  function playEnter() {
    if (_enterTimer) clearTimeout(_enterTimer);
    arriveClass = "enter";
    _enterTimer = setTimeout(() => {
      arriveClass = "";
      _enterTimer = null;
    }, ENTER_MS);
  }
  // Play the "landed on a new Space" settle. Skipped if we're mid enter/exit
  // (those own the avatar and shouldn't be interrupted by a Space hop).
  function playLand() {
    if (arriveClass === "enter" || arriveClass === "exit") return;
    if (_enterTimer) clearTimeout(_enterTimer);
    arriveClass = "land";
    _enterTimer = setTimeout(() => {
      arriveClass = "";
      _enterTimer = null;
    }, LAND_MS);
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
      // show_floater, not getCurrentWindow().show(): on macOS a bare show()
      // leaves the window on whatever Space it was last pinned to, so the
      // avatar would appear on desktop 1 while the user dictates into a
      // fullscreen app on desktop 3. The Rust command shows AND re-pins to
      // all Spaces. NO setFocus anywhere — never steal focus.
      await invoke("show_floater");
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
      // "always" mode never hides the window, so nothing in this file used to
      // touch it once a dictation began — which left the macOS Space pin
      // resting on whatever the 30s watchdog last did. Re-pin when a dictation
      // starts so the avatar is on the desktop the user is on RIGHT NOW, not
      // the one the app launched on. show_floater is idempotent on an
      // already-visible window.
      if (vis === "always" && st !== "idle") {
        invoke("show_floater").catch(() => {});
      }
      return;
    }
    if (st !== "idle") {
      // Active pipeline → make sure we're shown (cancels any pending hide).
      void autoShow();
    } else if (toastMessage !== "" && _autoShown) {
      // Idle, but a notice is still on screen. AUTO_HIDE_GRACE_MS is 1.8s and
      // an error toast is pinned for at least ERROR_TOAST_MIN_MS (15s), so
      // hiding on flow state alone tore the bubble away mid-sentence — which is
      // exactly what "the warning looks truncated" was. Hold the floater open;
      // toastMessage is read on every idle pass, so this effect re-runs when
      // the toast clears and the hide is scheduled then.
      cancelPendingAutoHide();
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
  class:mic-waiting={micWaiting && displayState === "listening"}
  class:arrive-enter={arriveClass === "enter"}
  class:arrive-exit={arriveClass === "exit"}
  class:arrive-land={arriveClass === "land"}
  data-arrive-origin={isMinimalSkin(skin) ? "center" : "bottom"}
  data-arrive-skin={skin === "wave" ? "wave" : skin === "siri" ? "siri" : "character"}
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
  {#if toastMessage && (!isMinimalSkin(skin) || toastKind === "error")}
    <div
      class="bubble show"
      data-state={toastKind === "error" ? "toast-error" : "toast"}
      role={toastKind === "error" ? "alert" : "status"}
      aria-live={toastKind === "error" ? "assertive" : "polite"}
      aria-atomic="true"
    >
      <!-- Scrollable error prose intentionally enters the tab order so keyboard
           users can reach and scroll the complete message. -->
      <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
      <div
        class="bubble-text"
        tabindex={toastKind === "error" ? 0 : undefined}
        role="region"
        aria-label={toastKind === "error" ? "Full error message" : "Notification"}
      >{toastMessage}</div>
      <span class="bubble-emoji">{toastKind === "error" ? "⚠" : "📋"}</span>
      {#if toastKind === "error"}
        <button
          type="button"
          class="error-dismiss"
          aria-label="Dismiss error"
          onmousedown={(e) => e.stopPropagation()}
          onclick={(e) => { e.stopPropagation(); dismissToast(); }}
        >×</button>
      {/if}
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

  {#if skin === "stylized" || skin === "fox" || isRasterAvatarSkin(skin) || isPetSkin(skin) || skin === "cat" || skin === "real-clippy"}

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

  {:else if isPetSkin(skin)}
    <!-- ═══════════════════════════════════════════════════════════════════
         TERMINAL PETS — Codex CLI sprite-sheet companions (lib/pets.ts,
         SpritePet.svelte). Full character semantics: bubbles, quips, floor
         shadow, character enter/exit. State mapping via petAnim above.
         ═══════════════════════════════════════════════════════════════════ -->
    <SpritePet petId={petIdFromSkin(skin)} anim={petAnim} />
  {:else if skin === "wave"}
    <!-- ═══════════════════════════════════════════════════════════════════
         WAVE BAR — a small Apple-style pill. No routine text/bubbles/quips;
         ever. 12 slim white bars on a gray translucent pill; each bar rests
         as a dot and expands/contracts individually with the voice, capped
         at ~68% of the pill height. Heights are rAF-smoothed (waveHeights[]);
         colour shifts green on success, red-blinks ×3 on error. Scales with
         the S/M/L slider via --fscale.
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
          style="--bi: {i}; height: {Math.round((0.11 + 0.57 * Math.max(0, Math.min(1, h))) * 100)}%;"
        ></span>
      {/each}
    </div>
  {:else if skin === "siri"}
    <!-- ═══════════════════════════════════════════════════════════════════
         SIRI ORB — a tiny multicolour fluid orb. No routine text/bubbles. Six
         counter-rotating conic gradients under blur+contrast produce the
         Apple-style liquid colour blobs (technique from SmoothUI's SiriOrb);
         the glass layer clips the blurred core to a crisp circle and a
         backdrop-filtered center adds the liquid middle. Idle: slow drift.
         Listening: blooms + swirls faster with the live mic level. Thinking:
         fastest swirl. Success: green ring. Error: red shudder. Default
         position right-of-centre (lib/floater-place.ts); scales via --fscale.
         ═══════════════════════════════════════════════════════════════════ -->
    <div
      class="siri-orb"
      class:success={waveSuccess}
      class:error={waveError}
      data-state={displayState}
      style="--orb-level: {micLevel.toFixed(3)};"
      aria-hidden="true"
    >
      <span class="siri-glass">
        <span class="siri-core"></span>
      </span>
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

  /* ── Mic handover ("hold on") ──────────────────────────────────────────
     From hotkey-down until audio actually reaches the recorder. On a healthy
     built-in mic this is a sub-100ms flicker you'll never notice; on a
     Bluetooth mic it's the 2-10 seconds during which the app used to LOOK like
     it was listening while capturing nothing. A slow breathing dim reads as
     "not ready yet" without competing with the recording state that follows.
     Deliberately opacity-only: it composites on the GPU, applies to every skin
     including the wave/siri pills, and can't disturb the two-box sizing model
     (a transform here would fight the arrival animations). */
  .clippy-stage.mic-waiting {
    animation: mic-waiting-breathe 1.1s ease-in-out infinite;
  }
  @keyframes mic-waiting-breathe {
    0%, 100% { opacity: 0.45; }
    50% { opacity: 0.82; }
  }
  /* Respect the OS setting — a pulsing always-on-top window is exactly the
     kind of motion reduced-motion users are opting out of. Hold a steady dim
     so the waiting state is still legible. */
  @media (prefers-reduced-motion: reduce) {
    .clippy-stage.mic-waiting {
      animation: none;
      opacity: 0.6;
    }
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
  /* Each skin CLASS arrives its own way (the "SDK enter/exit hook" — see
     docs/AVATAR_SDK.md). Characters pop up from the ground; the wave pill
     stretches open from a sliver; the orb blooms open with a twist. All
     three share the timing contract: enter ≤ ENTER_MS, exit ≤ EXIT_MS. */
  .clippy-stage[data-arrive-skin="character"].arrive-enter {
    animation: arrive-enter 380ms cubic-bezier(0.34, 1.56, 0.64, 1) both;
  }
  .clippy-stage[data-arrive-skin="character"].arrive-exit {
    animation: arrive-exit 240ms ease-in both;
  }
  .clippy-stage[data-arrive-skin="wave"].arrive-enter {
    animation: arrive-enter-wave 380ms cubic-bezier(0.34, 1.56, 0.64, 1) both;
  }
  .clippy-stage[data-arrive-skin="wave"].arrive-exit {
    animation: arrive-exit-wave 240ms ease-in both;
  }
  .clippy-stage[data-arrive-skin="siri"].arrive-enter {
    animation: arrive-enter-siri 380ms cubic-bezier(0.34, 1.56, 0.64, 1) both;
  }
  .clippy-stage[data-arrive-skin="siri"].arrive-exit {
    animation: arrive-exit-siri 240ms ease-in both;
  }
  /* "Landed on a new Space" settle. The window is already visible, so NEVER
     touch opacity here — a fade-in on an already-visible fox reads as a blink
     or rendering glitch (user-reported). A pure hop: quick lift with a slight
     stretch, then settle. */
  .clippy-stage.arrive-land {
    animation: arrive-land 420ms cubic-bezier(0.34, 1.56, 0.64, 1) both;
  }
  @keyframes arrive-land {
    0%   { transform: translateY(0) scale(1); }
    35%  { transform: translateY(-8px) scale(1.05); }
    70%  { transform: translateY(1px) scale(0.99); }
    100% { transform: translateY(0) scale(1); }
  }
  @media (prefers-reduced-motion: reduce) {
    /* Respect the OS "reduce motion" setting: no hop at all — the fox simply
       is there, which is the calmest possible arrival. */
    .clippy-stage.arrive-land {
      animation: none;
    }
  }
  @keyframes arrive-enter {
    0%   { opacity: 0; transform: scale(0.6); }
    100% { opacity: 1; transform: scale(1); }
  }
  @keyframes arrive-exit {
    0%   { opacity: 1; transform: scale(1); }
    100% { opacity: 0; transform: scale(0.75); }
  }
  @keyframes arrive-enter-wave {
    0%   { opacity: 0; transform: scaleX(0.25) scaleY(0.55); }
    60%  { opacity: 1; transform: scaleX(1.05) scaleY(1); }
    100% { opacity: 1; transform: scale(1); }
  }
  @keyframes arrive-exit-wave {
    0%   { opacity: 1; transform: scale(1); }
    100% { opacity: 0; transform: scaleX(0.3) scaleY(0.5); }
  }
  @keyframes arrive-enter-siri {
    0%   { opacity: 0; transform: scale(0.15) rotate(-100deg); }
    65%  { opacity: 1; transform: scale(1.1) rotate(8deg); }
    100% { opacity: 1; transform: scale(1) rotate(0deg); }
  }
  @keyframes arrive-exit-siri {
    0%   { opacity: 1; transform: scale(1) rotate(0deg); }
    100% { opacity: 0; transform: scale(0.2) rotate(70deg); }
  }
  /* Wave enter garnish: the bars pop up one after another as the pill
     stretches open, left → right via their per-bar --bi index. */
  .clippy-stage.arrive-enter .wave-bar {
    animation: wave-bar-pop 300ms cubic-bezier(0.34, 1.56, 0.64, 1) both;
    animation-delay: calc(var(--bi, 0) * 16ms);
  }
  @keyframes wave-bar-pop {
    0%   { transform: scaleY(0.2); }
    100% { transform: scaleY(1); }
  }

  /* ── Wave bar skin ─────────────────────────────────────────────────────
     Small Apple-style stadium pill, gray translucent, with 12 slim WHITE
     bars. Each bar is a FIXED 3px wide and rests as a perfect dot
     (min-height = width); heights animate independently from the rAF loop.
     Colour is driven by --wave-color (white → green on success → red on
     error). */
  .wave-pill {
    --wave-color: rgba(255, 255, 255, 0.95);
    position: relative;
    display: flex;
    flex: 0 0 auto; /* never shrink below the explicit pill width */
    align-items: center;
    justify-content: center;
    gap: calc(4px * var(--fscale, 1));
    width: calc(120px * var(--fscale, 1));
    height: calc(32px * var(--fscale, 1));
    padding: 0 calc(12px * var(--fscale, 1));
    box-sizing: border-box;
    border-radius: 999px;
    background: rgba(44, 46, 52, 0.38);
    border: 1px solid rgba(255, 255, 255, 0.12);
    backdrop-filter: blur(14px) saturate(1.1);
    -webkit-backdrop-filter: blur(14px) saturate(1.1);
    box-shadow: 0 3px 12px rgba(0, 0, 0, 0.18),
                inset 0 1px 0 rgba(255, 255, 255, 0.08);
    transition: background 260ms ease, border-color 260ms ease;
  }
  .wave-pill.mac {
    background: rgba(120, 124, 132, 0.20);
    border-color: rgba(120, 80, 30, 0.16);
    box-shadow: 0 3px 12px rgba(120, 80, 30, 0.16),
                inset 0 1px 0 rgba(255, 255, 255, 0.35);
  }
  .wave-bar {
    flex: 0 0 calc(3px * var(--fscale, 1));
    width: calc(3px * var(--fscale, 1));
    /* Resting state is a perfect dot: min-height equals the bar width. */
    min-height: calc(3px * var(--fscale, 1));
    border-radius: 999px;
    background: var(--wave-color);
    /* Height set inline (11..68% of the pill's inner height), centred. A short
       CSS transition on top of the rAF lerp keeps sub-frame motion silky. */
    transition: height 60ms linear, background 260ms ease;
    align-self: center;
  }
  /* Listening: a whisper of glow so live bars read as "lit". */
  .wave-pill[data-state="listening"] .wave-bar {
    box-shadow: 0 0 calc(4px * var(--fscale, 1)) rgba(255, 255, 255, 0.28);
  }
  /* Success: bars go Apple-green and a pop ripples left → right. */
  .wave-pill.success { --wave-color: #34c759; }
  .wave-pill.success .wave-bar {
    animation: wave-success-pop 460ms cubic-bezier(0.34, 1.56, 0.64, 1) both;
    animation-delay: calc(var(--bi, 0) * 24ms);
    box-shadow: 0 0 calc(5px * var(--fscale, 1)) rgba(52, 199, 89, 0.5);
  }
  @keyframes wave-success-pop {
    0%   { transform: scaleY(1); filter: brightness(1); }
    40%  { transform: scaleY(1.55); filter: brightness(1.5); }
    100% { transform: scaleY(1); filter: brightness(1); }
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
     The Apple-style fluid orb. Six conic gradients rotate at DIFFERENT
     multiples of one animated --siri-angle (some counter-rotating), then
     blur+contrast melt them into organic colour blobs — the same trick
     Apple-quality CSS orbs use (ref: SmoothUI SiriOrb). The core bleeds
     6px past the circle so its blur clips crisply inside .siri-glass;
     a backdrop-filtered glass center gives the liquid middle, and the
     gloss adds the 3D specular. Swirl speed comes from --spin (per state);
     bloom/saturation ride --orb-level (live mic). */
  @property --siri-angle {
    syntax: "<angle>";
    inherits: false;
    initial-value: 0deg;
  }
  .siri-orb {
    /* Palette: near-white heart + Siri pink/blue/purple. */
    --sbg: #f4f3fb;
    --sc1: #ff5fa2;
    --sc2: #5ac8fa;
    --sc3: #a06bff;
    --spin: 11s;
    position: relative;
    flex: 0 0 auto; /* never shrink below the explicit orb size */
    width: calc(56px * var(--fscale, 1));
    height: calc(56px * var(--fscale, 1));
    border-radius: 50%;
    /* Bloom + scale with the mic level. */
    transform: scale(calc(1 + 0.12 * var(--orb-level, 0)));
    transition: transform 90ms ease-out;
    box-shadow: 0 2px 14px rgba(110, 80, 190, 0.35);
    isolation: isolate;
  }
  /* Clips the oversized blurred core to a crisp circle. */
  .siri-glass {
    position: absolute;
    inset: 0;
    border-radius: 50%;
    overflow: hidden;
    display: block;
  }
  .siri-core {
    position: absolute;
    inset: calc(-6px * var(--fscale, 1)); /* bleed so the blur never shows a soft outer edge */
    display: block;
    background:
      conic-gradient(from calc(var(--siri-angle) * 2) at 25% 70%, var(--sc3), transparent 20% 80%, var(--sc3)),
      conic-gradient(from calc(var(--siri-angle) * 2) at 45% 75%, var(--sc2), transparent 30% 60%, var(--sc2)),
      conic-gradient(from calc(var(--siri-angle) * -3) at 80% 20%, var(--sc1), transparent 40% 60%, var(--sc1)),
      conic-gradient(from calc(var(--siri-angle) * 2) at 15% 5%, var(--sc2), transparent 10% 90%, var(--sc2)),
      conic-gradient(from var(--siri-angle) at 20% 80%, var(--sc1), transparent 10% 90%, var(--sc1)),
      conic-gradient(from calc(var(--siri-angle) * -2) at 85% 10%, var(--sc3), transparent 20% 80%, var(--sc3));
    background-color: var(--sbg);
    box-shadow: inset var(--sbg) 0 0 calc(8px * var(--fscale, 1)) calc(2px * var(--fscale, 1));
    filter:
      blur(calc(4px * var(--fscale, 1)))
      contrast(1.6)
      saturate(calc(1.05 + 0.55 * var(--orb-level, 0)))
      brightness(calc(1 + 0.08 * var(--orb-level, 0)));
    animation: siri-angle var(--spin) linear infinite;
  }
  /* Liquid center: re-blur + re-contrast what's behind, faded out toward
     the rim, which melts the middle into the signature soft heart. */
  .siri-glass::after {
    content: "";
    position: absolute;
    inset: 0;
    border-radius: 50%;
    backdrop-filter: blur(calc(8px * var(--fscale, 1))) contrast(1.8);
    -webkit-backdrop-filter: blur(calc(8px * var(--fscale, 1))) contrast(1.8);
    mask-image: radial-gradient(black 25%, transparent 72%);
    -webkit-mask-image: radial-gradient(black 25%, transparent 72%);
  }
  @keyframes siri-angle {
    to { --siri-angle: 360deg; }
  }
  .siri-gloss {
    position: absolute;
    inset: 0;
    border-radius: 50%;
    background:
      radial-gradient(55% 40% at 33% 25%, rgba(255, 255, 255, 0.6), rgba(255, 255, 255, 0) 65%);
    box-shadow:
      inset 0 0 0 1px rgba(255, 255, 255, 0.25),
      inset 0 calc(-5px * var(--fscale, 1)) calc(10px * var(--fscale, 1)) rgba(70, 45, 140, 0.18);
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
  /* Listening: faster swirl + a soft mic-reactive halo. */
  .siri-orb[data-state="listening"] { --spin: 4.5s; }
  .siri-orb[data-state="listening"] .siri-ring {
    box-shadow: 0 0 calc(12px * var(--fscale, 1)) rgba(160, 130, 255, calc(0.3 + 0.55 * var(--orb-level, 0)));
  }
  /* Thinking / writing: fastest swirl. */
  .siri-orb[data-state="thinking"],
  .siri-orb[data-state="writing"] { --spin: 2.2s; }
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
    /* Must track ERROR_BUBBLE_W in the script (minus the bubble's own padding
       and border) — the window is sized from that constant, and a narrower
       max-width here just reintroduces the 3-words-per-line wrap. */
    max-width: calc(412px * var(--bubble-scale, 1));
    white-space: normal;
    align-items: flex-start;
    pointer-events: auto;
  }
  .bubble[data-state="toast-error"] .bubble-text {
    display: block;
    -webkit-line-clamp: unset;
    line-clamp: unset;
    max-height: calc(180px * var(--bubble-scale, 1));
    overflow-x: hidden;
    overflow-y: auto;
    overflow-wrap: anywhere;
    overscroll-behavior: contain;
    padding-right: calc(2px * var(--bubble-scale, 1));
  }
  .error-dismiss {
    flex: 0 0 auto;
    width: calc(18px * var(--bubble-scale, 1));
    height: calc(18px * var(--bubble-scale, 1));
    margin: 0;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.18);
    color: #fff;
    font: inherit;
    font-weight: 700;
    line-height: 1;
    cursor: pointer;
  }
  .error-dismiss:hover,
  .error-dismiss:focus-visible {
    background: rgba(255, 255, 255, 0.32);
    outline: 2px solid rgba(255, 255, 255, 0.9);
    outline-offset: 1px;
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

  /* X dismiss button was removed — double-click Clippy to open the main
     window instead. Hide via tray → Toggle Clippy. */
</style>
