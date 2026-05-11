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
  let state = $state<ClippyState>("idle");
  let displayState = $state<ClippyState>("idle");
  let mode = $state<Mode>("light");
  let blinkOpen = $state(true);
  let lookDir = $state<"left" | "right" | "center">("center");

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
    listen<string>("wispr:state", (e) => {
      const next = mapFlow(e.payload);
      console.log("[clippy] wispr:state", e.payload, "→", next);
      state = next;
      if (next === "pasting") {
        setTimeout(() => {
          state = "idle";
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
      clearInterval(blinkTimer);
      clearInterval(lookTimer);
      window.removeEventListener("mouseup", onMove);
    };
  });

  async function hideMe() {
    await getCurrentWindow().hide();
  }

  let eyeShiftX = $derived(
    lookDir === "left" ? -1.8 : lookDir === "right" ? 1.8 : 0,
  );

  // Themed labels per skin.
  let labels = $derived.by(() => {
    if (skin === "chippy") {
      return { listening: "crunching…", thinking: "thinking", writing: "seasoning", writingIcon: "🧂", pasting: "done!" };
    }
    return { listening: "listening…", thinking: "thinking", writing: "polishing", writingIcon: "✏️", pasting: "done!" };
  });
</script>

<div class="clippy-stage" data-tauri-drag-region role="button" tabindex="0" aria-label="Floater — drag to move">
  {#if skin === "stylized" || skin === "chippy"}
    <!-- Subtle floor shadow that pulses on listening -->
    <div class="shadow" class:pulse={displayState === "listening"}></div>

    <!-- Speech bubble (only over hand-built SVG variants — real Clippy has its own balloon).
         Uses displayState so it stays in sync with the visible animation. -->
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

  {#if skin === "stylized"}
    <!-- Stylized paperclip with rich state-specific animations:
         - listening: turns toward viewer, big ear pops out, alert sway
         - phew transition: brief sweat-drop right after listening ends
         - thinking advanced: brain bubble overhead
         - writing: paper slides in beside Clippy, pen scribbles
         - pasting: paper flies away, Clippy bounces -->
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
          <g class="ear">
            <!-- Drop shadow under the ear for depth -->
            <ellipse cx="6" cy="86" rx="14" ry="3" fill="rgba(0,0,0,0.18)" />
            <!-- Outer ear curve — wider extension, dips lower than mid-body -->
            <path
              d="M 30 40
                 C 0 36, -10 60, 0 80
                 C 8 94, 24 90, 32 76 Z"
              fill="#ffe0c0"
              stroke="#1d1d1f"
              stroke-width="2.5"
              stroke-linejoin="round"
            />
            <!-- Highlight on the upper ridge -->
            <path
              d="M 22 44 C 8 42, 0 58, 4 72"
              fill="none"
              stroke="#fff4e0"
              stroke-width="2"
              stroke-linecap="round"
              opacity="0.7"
            />
            <!-- Inner ear canal shading -->
            <path
              d="M 20 54 C 12 58, 12 72, 22 74"
              fill="none"
              stroke="#c8916a"
              stroke-width="2"
              stroke-linecap="round"
            />
            <!-- Tiny inner detail -->
            <path
              d="M 16 64 Q 10 66, 14 72"
              fill="none"
              stroke="#a76b48"
              stroke-width="1.2"
              stroke-linecap="round"
            />
          </g>
        {/if}

        <!-- Paperclip body -->
        <g class="body" stroke="#1d1d1f" stroke-width="6" fill="none" stroke-linecap="round" stroke-linejoin="round">
          <path d="M 50 30 C 50 18, 70 18, 70 30 L 70 110 C 70 132, 38 132, 38 110 L 38 50 C 38 38, 58 38, 58 50 L 58 100" />
        </g>
        <g class="brows" stroke="#1d1d1f" stroke-width="3.5" stroke-linecap="round" fill="none">
          <path d="M 36 36 Q 42 32, 48 36" />
          <path d="M 60 36 Q 66 32, 72 36" />
        </g>
        <g class="eyes">
          <ellipse cx="44" cy="50" rx="6" ry={blinkOpen ? 7 : 0.5} fill="#ffffff" stroke="#1d1d1f" stroke-width="2" />
          <circle cx={44 + eyeShiftX} cy="51" r={blinkOpen ? 2.4 : 0} fill="#1d1d1f" />
          <ellipse cx="64" cy="50" rx="6" ry={blinkOpen ? 7 : 0.5} fill="#ffffff" stroke="#1d1d1f" stroke-width="2" />
          <circle cx={64 + eyeShiftX} cy="51" r={blinkOpen ? 2.4 : 0} fill="#1d1d1f" />
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
  {:else if skin === "chippy"}
    <!-- Chippy — friendly potato chip. Cleaner silhouette, warmer palette,
         a touch of crunch with chip "ribs" along the saddle curve. -->
    <svg
      class="character chippy"
      viewBox="0 0 140 140"
      xmlns="http://www.w3.org/2000/svg"
      data-state={displayState}
      aria-hidden="true"
    >
      <defs>
        <linearGradient id="chip-fill-v2" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stop-color="#fbe3a0" />
          <stop offset="50%" stop-color="#e8b15c" />
          <stop offset="100%" stop-color="#b6792a" />
        </linearGradient>
        <radialGradient id="chip-hilite" cx="42%" cy="32%" r="38%">
          <stop offset="0%" stop-color="#fff6d8" stop-opacity="0.9" />
          <stop offset="100%" stop-color="#fff6d8" stop-opacity="0" />
        </radialGradient>
        <filter id="chip-drop" x="-20%" y="-20%" width="140%" height="140%">
          <feDropShadow dx="0" dy="2" stdDeviation="2" flood-color="#7a4d12" flood-opacity="0.4"/>
        </filter>
      </defs>

      <!-- White halo for dark backgrounds -->
      <path
        d="M 28 70 C 22 38, 60 24, 70 40 C 80 24, 118 38, 112 70 C 118 102, 80 116, 70 100 C 60 116, 22 102, 28 70 Z"
        fill="none"
        stroke="#ffffff"
        stroke-width="9"
        opacity="0.95"
      />

      <!-- Chip body — saddle-curve silhouette -->
      <g class="chip-body" filter="url(#chip-drop)">
        <path
          d="M 28 70 C 22 38, 60 24, 70 40 C 80 24, 118 38, 112 70 C 118 102, 80 116, 70 100 C 60 116, 22 102, 28 70 Z"
          fill="url(#chip-fill-v2)"
          stroke="#7a4d12"
          stroke-width="2.5"
          stroke-linejoin="round"
        />

        <!-- Highlight wash, top -->
        <path
          d="M 32 60 C 32 38, 60 30, 70 44 C 80 30, 108 38, 108 60"
          fill="none"
          stroke="url(#chip-hilite)"
          stroke-width="16"
          stroke-linecap="round"
          opacity="0.85"
          style="mix-blend-mode: screen;"
        />

        <!-- Saddle crease (Pringles fold) -->
        <path d="M 30 74 Q 70 60, 110 74" fill="none" stroke="#a06010" stroke-width="2" stroke-linecap="round" opacity="0.55" />
        <!-- Subtle parallel crease for a "crispy" feel -->
        <path d="M 36 80 Q 70 70, 104 80" fill="none" stroke="#a06010" stroke-width="1" stroke-linecap="round" opacity="0.32" />

        <!-- Salt grains -->
        <circle cx="46" cy="86" r="1.6" fill="#ffffff" opacity="0.95" />
        <circle cx="98" cy="84" r="1.4" fill="#ffffff" opacity="0.9" />
        <circle cx="62" cy="98" r="1.2" fill="#ffffff" opacity="0.85" />
        <circle cx="84" cy="48" r="1.3" fill="#ffffff" opacity="0.9" />
        <circle cx="54" cy="58" r="1.0" fill="#ffffff" opacity="0.85" />
      </g>

      <!-- Eyebrows (deep brown, slightly thicker) -->
      <g class="brows" stroke="#4a2208" stroke-width="3.2" stroke-linecap="round" fill="none">
        <path class="brow brow-l" d="M 50 62 Q 58 56, 66 62" />
        <path class="brow brow-r" d="M 78 62 Q 86 56, 94 62" />
      </g>

      <!-- Eyes — friendly, slightly larger -->
      <g class="eyes">
        <ellipse cx="58" cy="76" rx="6" ry={blinkOpen ? 7 : 0.6} fill="#ffffff" stroke="#4a2208" stroke-width="2" />
        <circle cx={58 + eyeShiftX} cy="77" r={blinkOpen ? 2.5 : 0} fill="#1d1d1f" />
        <circle cx={57 + eyeShiftX} cy="75.5" r={blinkOpen ? 0.8 : 0} fill="#ffffff" />

        <ellipse cx="82" cy="76" rx="6" ry={blinkOpen ? 7 : 0.6} fill="#ffffff" stroke="#4a2208" stroke-width="2" />
        <circle cx={82 + eyeShiftX} cy="77" r={blinkOpen ? 2.5 : 0} fill="#1d1d1f" />
        <circle cx={81 + eyeShiftX} cy="75.5" r={blinkOpen ? 0.8 : 0} fill="#ffffff" />
      </g>

      <!-- Smile -->
      <path
        class="mouth"
        d="M 64 90 Q 70 96, 76 90"
        fill="none"
        stroke="#4a2208"
        stroke-width="2.2"
        stroke-linecap="round"
      />
    </svg>
  {/if}

  <!-- Hide button -->
  <div class="controls">
    <button class="ctrl-btn hide-btn" onclick={hideMe} title="Hide (use sidebar to switch skin)">×</button>
  </div>
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

  /* Body turns slightly toward the user when listening — like the user said,
     "turns to me." Combined with the ear popping out of the left side. */
  .clippy-stylized .body-group {
    transform-origin: 50% 90%;
    transition: transform 280ms cubic-bezier(0.32, 1.4, 0.4, 1);
  }
  .clippy-stylized[data-state="listening"] .body-group {
    transform: rotate(-8deg) translateX(2px);
  }

  /* Big ear pops in with a bounce, then "scans" gently while listening as if
     leaning forward to catch every word. */
  .clippy-stylized .ear {
    transform-origin: 30px 60px;
    animation:
      ear-pop 360ms cubic-bezier(0.34, 1.56, 0.64, 1) both,
      ear-listen 1.6s ease-in-out 360ms infinite;
  }
  @keyframes ear-pop {
    0%   { transform: scale(0) rotate(-30deg) translate(10px, 0); opacity: 0; }
    60%  { transform: scale(1.18) rotate(10deg) translate(-1px, 3px); opacity: 1; }
    100% { transform: scale(1.08) rotate(6deg) translate(0px, 2px);  opacity: 1; }
  }
  @keyframes ear-listen {
    0%, 100% { transform: scale(1.08) rotate(6deg) translate(0px, 2px); }
    50%      { transform: scale(1.12) rotate(11deg) translate(-1px, 4px); }
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

  .clippy-classic-svg {
    width: 124px;
    height: 162px;
    filter: drop-shadow(0 2px 4px rgba(0, 0, 0, 0.35))
            drop-shadow(0 0 8px rgba(255, 255, 255, 0.45));
  }

  .chippy {
    width: 124px;
    height: 124px;
    filter: drop-shadow(0 0 6px rgba(255, 255, 255, 0.35));
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

  /* Chippy's smile widens on pasting */
  .chippy[data-state="pasting"] .mouth {
    d: path("M 60 88 Q 70 100, 80 88");
  }

  /* Speech bubble — pinned to the top of the window with explicit space
     below so it never overlaps Clippy's head. The taller floater window
     (190x230) gives the bubble its own dedicated band at the top. */
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

  @keyframes pencil-wiggle {
    0%, 100% { transform: rotate(-12deg); }
    50% { transform: rotate(8deg); }
  }

  /* Hide button */
  .controls {
    position: absolute;
    top: 4px;
    right: 4px;
    opacity: 0;
    transition: opacity 200ms ease;
    z-index: 2;
  }

  .clippy-stage:hover .controls {
    opacity: 1;
  }

  .ctrl-btn {
    background: rgba(255, 255, 255, 0.92);
    border: 1px solid rgba(0, 0, 0, 0.1);
    color: #6e6e73;
    width: 20px;
    height: 20px;
    border-radius: 10px;
    font-size: 14px;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    pointer-events: auto;
    line-height: 1;
  }

  .ctrl-btn:hover {
    background: #fff;
    color: #1d1d1f;
  }
</style>
