<script lang="ts">
  import type { Skin } from "./skin-store.svelte";

  type AvatarState = "idle" | "listening" | "thinking" | "writing" | "pasting";

  let {
    skin,
    state,
    mode,
    hovering,
    blinkOpen,
    eyeShiftX,
    eyeShiftY,
    phewActive,
  }: {
    skin: Skin;
    state: AvatarState;
    mode: string;
    hovering: boolean;
    blinkOpen: boolean;
    eyeShiftX: number;
    eyeShiftY: number;
    phewActive: boolean;
  } = $props();
</script>

{#if skin === "codex-fox"}
  <svg
    class="character rich-avatar codex-fox-skin"
    viewBox="0 0 160 160"
    xmlns="http://www.w3.org/2000/svg"
    data-state={state}
    data-mode={mode}
    data-hover={hovering ? "1" : "0"}
    aria-hidden="true"
  >
    <defs>
      <linearGradient id="wf-codex-orange" x1="0.3" y1="0" x2="0.72" y2="1">
        <stop offset="0%" stop-color="#ffbb61" />
        <stop offset="45%" stop-color="#f28a34" />
        <stop offset="100%" stop-color="#c96522" />
      </linearGradient>
      <linearGradient id="wf-codex-cream" x1="0.35" y1="0" x2="0.65" y2="1">
        <stop offset="0%" stop-color="#fff8e8" />
        <stop offset="100%" stop-color="#f0d9b5" />
      </linearGradient>
      <linearGradient id="wf-codex-scarf" x1="0" y1="0" x2="1" y2="1">
        <stop offset="0%" stop-color="#263347" />
        <stop offset="100%" stop-color="#111827" />
      </linearGradient>
      <filter id="wf-codex-glow" x="-30%" y="-30%" width="160%" height="160%">
        <feDropShadow dx="0" dy="0" stdDeviation="2.6" flood-color="#48c8ff" flood-opacity="0.75" />
      </filter>
    </defs>

    <ellipse class="avatar-shadow" cx="80" cy="145" rx="39" ry="8" fill="rgba(25, 20, 18, 0.18)" />

    <g class="codex-tail">
      <path d="M 48 113 C 14 105 21 66 56 74 C 28 82 33 105 58 104 C 71 103 77 93 74 84 C 95 93 79 125 48 113 Z" fill="url(#wf-codex-orange)" stroke="#944819" stroke-width="1.4" />
      <path d="M 37 105 C 24 93 29 79 48 76 C 32 86 36 101 55 103 C 49 109 43 110 37 105 Z" fill="url(#wf-codex-cream)" opacity="0.95" />
    </g>

    <g class="codex-body">
      <ellipse cx="82" cy="108" rx="35" ry="38" fill="url(#wf-codex-orange)" stroke="#9a4b19" stroke-width="1.5" />
      <ellipse cx="83" cy="121" rx="18" ry="21" fill="url(#wf-codex-cream)" opacity="0.96" />
      <path class="codex-scarf" d="M 49 88 Q 81 105 115 88 L 108 111 Q 80 126 53 111 Z" fill="url(#wf-codex-scarf)" stroke="#0b1220" stroke-width="1.1" />
      <path d="M 53 91 Q 81 104 111 91" fill="none" stroke="#50cfff" stroke-width="2.1" filter="url(#wf-codex-glow)" />
      <circle cx="81" cy="104" r="6" fill="#192233" stroke="#59d7ff" stroke-width="2" filter="url(#wf-codex-glow)" />
      <circle cx="81" cy="104" r="2.3" fill="#bff7ff" />
    </g>

    <g class="codex-head">
      <path class="codex-ear codex-ear-l" d="M 52 50 L 45 13 L 73 42 Z" fill="url(#wf-codex-orange)" stroke="#9a4b19" stroke-width="1.4" />
      <path d="M 55 44 L 50 22 L 68 42 Z" fill="#ffd8bf" opacity="0.86" />
      <path class="codex-ear codex-ear-r" d="M 108 50 L 116 13 L 87 42 Z" fill="url(#wf-codex-orange)" stroke="#9a4b19" stroke-width="1.4" />
      <path d="M 105 44 L 111 22 L 92 42 Z" fill="#ffd8bf" opacity="0.86" />
      <circle cx="80" cy="63" r="34" fill="url(#wf-codex-orange)" stroke="#9a4b19" stroke-width="1.5" />
      <path d="M 52 69 Q 80 88 108 69 Q 101 98 80 101 Q 59 98 52 69 Z" fill="url(#wf-codex-cream)" opacity="0.98" />
      <path d="M 64 45 Q 80 33 96 45" fill="none" stroke="#d87321" stroke-width="2.4" stroke-linecap="round" opacity="0.5" />
      <g class="codex-eyes">
        <ellipse cx={68 + eyeShiftX * 0.7} cy={61 + eyeShiftY * 0.45} rx="6.4" ry={blinkOpen ? 7.6 : 0.7} fill="#24180f" />
        <circle cx={66 + eyeShiftX * 0.55} cy={58.5 + eyeShiftY * 0.35} r={blinkOpen ? 2.1 : 0} fill="#fff" opacity="0.92" />
        <ellipse cx={92 + eyeShiftX * 0.7} cy={61 + eyeShiftY * 0.45} rx="6.4" ry={blinkOpen ? 7.6 : 0.7} fill="#24180f" />
        <circle cx={90 + eyeShiftX * 0.55} cy={58.5 + eyeShiftY * 0.35} r={blinkOpen ? 2.1 : 0} fill="#fff" opacity="0.92" />
      </g>
      <path d="M 77.3 72 L 80 75.4 L 82.7 72 Z" fill="#4a2a1c" />
      <path d="M 80 75.4 Q 76 80 70 77.5 M 80 75.4 Q 84 80 90 77.5" fill="none" stroke="#6b3b22" stroke-width="1.3" stroke-linecap="round" />
    </g>

    {#if state === "listening"}
      <g class="codex-waves" fill="none" stroke="#53d7ff" stroke-width="2.4" stroke-linecap="round" filter="url(#wf-codex-glow)">
        <path d="M 23 61 Q 15 70 23 79" />
        <path d="M 31 57 Q 20 70 31 83" />
        <path d="M 137 61 Q 145 70 137 79" />
      </g>
    {:else if state === "thinking"}
      <g class="codex-think" fill="none" stroke="#62d8ff" stroke-width="1.8" stroke-linecap="round" filter="url(#wf-codex-glow)">
        <circle cx="116" cy="30" r="3" fill="#e6fbff" />
        <path d="M 128 16 L 130.4 22 L 136.5 24.4 L 130.4 26.8 L 128 33 L 125.6 26.8 L 119.5 24.4 L 125.6 22 Z" fill="#e6fbff" />
      </g>
    {:else if state === "writing"}
      <g class="codex-pencil">
        <rect x="99" y="113" width="29" height="22" rx="4" fill="#fffaf0" stroke="#8f7b62" stroke-width="1" />
        <path d="M 105 121 H 122 M 105 127 H 118" stroke="#9fb2c4" stroke-width="1" stroke-linecap="round" />
        <path d="M 101 105 L 119 123" stroke="#2764b4" stroke-width="4" stroke-linecap="round" />
        <path d="M 117 121 L 123 127" stroke="#4b2d18" stroke-width="4" stroke-linecap="round" />
      </g>
    {:else if state === "pasting"}
      <g class="codex-success" fill="#58d6ff" filter="url(#wf-codex-glow)">
        <path d="M 119 58 L 121 63 L 126 65 L 121 67 L 119 72 L 117 67 L 112 65 L 117 63 Z" />
        <path d="M 35 92 L 36.5 96 L 40.5 97.5 L 36.5 99 L 35 103 L 33.5 99 L 29.5 97.5 L 33.5 96 Z" />
      </g>
    {/if}

    {#if phewActive}
      <path class="avatar-phew" d="M 113 50 Q 110 42 113 35 Q 116 42 113 50 Z" fill="#70c9ff" stroke="#1d1d1f" stroke-width="0.8" />
    {/if}
  </svg>
{:else if skin === "oru-gujia"}
  <svg
    class="character rich-avatar oru-gujia-skin"
    viewBox="0 0 220 150"
    xmlns="http://www.w3.org/2000/svg"
    data-state={state}
    data-mode={mode}
    data-hover={hovering ? "1" : "0"}
    aria-hidden="true"
  >
    <defs>
      <linearGradient id="wf-gujia-white" x1="0.35" y1="0" x2="0.6" y2="1">
        <stop offset="0%" stop-color="#ffffff" />
        <stop offset="100%" stop-color="#e9e3d7" />
      </linearGradient>
      <linearGradient id="wf-oru-orange" x1="0.28" y1="0" x2="0.75" y2="1">
        <stop offset="0%" stop-color="#ffc276" />
        <stop offset="100%" stop-color="#df8232" />
      </linearGradient>
      <radialGradient id="wf-cat-eye" cx="0.4" cy="0.38" r="0.62">
        <stop offset="0%" stop-color="#d5cf78" />
        <stop offset="100%" stop-color="#5d6f39" />
      </radialGradient>
    </defs>

    <ellipse class="avatar-shadow" cx="110" cy="140" rx="67" ry="8" fill="rgba(25, 20, 18, 0.16)" />
    <rect class="cat-console" x="33" y="125" width="150" height="12" rx="6" fill="#f4f1ea" stroke="#cbc3b5" stroke-width="1" />
    <rect x="45" y="130" width="126" height="2.8" rx="1.4" fill="#2f3138" opacity="0.8" />

    <g class="gujia">
      <ellipse class="gujia-body" cx="76" cy="103" rx="43" ry="25" fill="url(#wf-gujia-white)" stroke="#d3ccbd" stroke-width="1.2" />
      <path class="gujia-tail" d="M 41 113 Q 67 135 106 119" fill="none" stroke="#f8f6ef" stroke-width="9" stroke-linecap="round" />
      <ellipse cx="82" cy="123" rx="8" ry="4" fill="#fff" stroke="#d3ccbd" stroke-width="0.7" />
      <ellipse cx="101" cy="122" rx="8" ry="4" fill="#fff" stroke="#d3ccbd" stroke-width="0.7" />
      <g class="gujia-head">
        <path class="gujia-ear-l" d="M 64 64 L 58 35 L 80 56 Z" fill="url(#wf-gujia-white)" stroke="#d3ccbd" stroke-width="1" />
        <path d="M 66 58 L 62 43 L 76 56 Z" fill="#f4c7c4" opacity="0.78" />
        <path class="gujia-ear-r" d="M 105 65 L 112 35 L 89 56 Z" fill="url(#wf-gujia-white)" stroke="#d3ccbd" stroke-width="1" />
        <path d="M 103 58 L 108 43 L 93 56 Z" fill="#f4c7c4" opacity="0.78" />
        <circle cx="85" cy="77" r="25" fill="url(#wf-gujia-white)" stroke="#d3ccbd" stroke-width="1.2" />
        <path d="M 78 49 L 80 57 M 88 48 L 88 57 M 97 51 L 94 58" stroke="#d9d2c5" stroke-width="1.4" stroke-linecap="round" opacity="0.75" />
        <g class="cat-eyes">
          <ellipse cx={76 + eyeShiftX * 0.45} cy={75 + eyeShiftY * 0.35} rx="4.4" ry={blinkOpen ? 5 : 0.5} fill="url(#wf-cat-eye)" stroke="#3c3426" stroke-width="0.7" />
          <ellipse cx={75.5 + eyeShiftX * 0.5} cy={75 + eyeShiftY * 0.35} rx="1.3" ry={blinkOpen ? 3.4 : 0} fill="#16120d" />
          <ellipse cx={94 + eyeShiftX * 0.45} cy={75 + eyeShiftY * 0.35} rx="4.4" ry={blinkOpen ? 5 : 0.5} fill="url(#wf-cat-eye)" stroke="#3c3426" stroke-width="0.7" />
          <ellipse cx={93.5 + eyeShiftX * 0.5} cy={75 + eyeShiftY * 0.35} rx="1.3" ry={blinkOpen ? 3.4 : 0} fill="#16120d" />
        </g>
        <path d="M 83.4 83.2 L 85 85.7 L 86.6 83.2 Z" fill="#d89191" />
        <path d="M 85 85.7 Q 80.8 89 76.6 87.2 M 85 85.7 Q 89.2 89 93.4 87.2" fill="none" stroke="#b79b87" stroke-width="0.8" stroke-linecap="round" />
      </g>
      {#if state === "thinking"}
        <g class="gujia-thought" fill="#fff" stroke="#cbc3b5" stroke-width="0.7">
          <circle cx="113" cy="50" r="2.5" />
          <circle cx="122" cy="40" r="3.5" />
          <circle cx="133" cy="28" r="4.8" />
        </g>
      {/if}
    </g>

    <g class="oru">
      <path class="oru-tail" d="M 178 118 C 207 110 206 74 184 71" fill="none" stroke="#e8943d" stroke-width="8" stroke-linecap="round" />
      <path d="M 197 84 Q 190 78 181 79" fill="none" stroke="#cf7427" stroke-width="3.2" stroke-linecap="round" />
      <ellipse class="oru-body" cx="151" cy="106" rx="31" ry="30" fill="url(#wf-oru-orange)" stroke="#c76b25" stroke-width="1.2" />
      <ellipse cx="144" cy="113" rx="13" ry="16" fill="#fff1da" opacity="0.92" />
      <g class="oru-stripes" fill="none" stroke="#cc7428" stroke-width="2.2" stroke-linecap="round" opacity="0.78">
        <path d="M 172 93 Q 180 99 178 108" />
        <path d="M 175 107 Q 181 114 177 121" />
        <path d="M 130 96 Q 123 104 128 114" />
      </g>
      <g class="oru-paws">
        <ellipse class="oru-paw-l" cx="137" cy="128" rx="7" ry="4.2" fill="#fff1da" stroke="#c76b25" stroke-width="0.7" />
        <ellipse class="oru-paw-r" cx="156" cy="128" rx="7" ry="4.2" fill="#fff1da" stroke="#c76b25" stroke-width="0.7" />
      </g>
      <g class="oru-head">
        <path class="oru-ear-l" d="M 132 67 L 124 35 L 152 59 Z" fill="url(#wf-oru-orange)" stroke="#c76b25" stroke-width="1" />
        <path d="M 134 60 L 129 44 L 148 58 Z" fill="#f2b1a2" opacity="0.85" />
        <path class="oru-ear-r" d="M 174 67 L 183 35 L 155 59 Z" fill="url(#wf-oru-orange)" stroke="#c76b25" stroke-width="1" />
        <path d="M 172 60 L 178 44 L 159 58 Z" fill="#f2b1a2" opacity="0.85" />
        <circle cx="153" cy="78" r="25" fill="url(#wf-oru-orange)" stroke="#c76b25" stroke-width="1.2" />
        <g class="oru-stripes" fill="none" stroke="#cc7428" stroke-width="2.1" stroke-linecap="round" opacity="0.84">
          <path d="M 145 57 L 144 66" />
          <path d="M 153 55 L 153 65" />
          <path d="M 161 57 L 162 66" />
        </g>
        <ellipse cx="153" cy="88" rx="13" ry="9" fill="#fff1da" />
        <g class="cat-eyes">
          <ellipse cx={144 + eyeShiftX * 0.65} cy={77 + eyeShiftY * 0.45} rx="4.8" ry={blinkOpen ? 5.6 : 0.5} fill="#b77d31" stroke="#744817" stroke-width="0.8" />
          <circle cx={144 + eyeShiftX * 0.72} cy={77.3 + eyeShiftY * 0.45} r={blinkOpen ? 2.6 : 0} fill="#160f08" />
          <circle cx={142.7 + eyeShiftX * 0.45} cy={75.3 + eyeShiftY * 0.3} r={blinkOpen ? 1.2 : 0} fill="#fff" />
          <ellipse cx={162 + eyeShiftX * 0.65} cy={77 + eyeShiftY * 0.45} rx="4.8" ry={blinkOpen ? 5.6 : 0.5} fill="#b77d31" stroke="#744817" stroke-width="0.8" />
          <circle cx={162 + eyeShiftX * 0.72} cy={77.3 + eyeShiftY * 0.45} r={blinkOpen ? 2.6 : 0} fill="#160f08" />
          <circle cx={160.7 + eyeShiftX * 0.45} cy={75.3 + eyeShiftY * 0.3} r={blinkOpen ? 1.2 : 0} fill="#fff" />
        </g>
        <path d="M 151.3 84.2 L 153 86.8 L 154.7 84.2 Z" fill="#df8580" />
        <path d="M 153 86.8 Q 148.8 90 144.4 88.5 M 153 86.8 Q 157.2 90 161.6 88.5" fill="none" stroke="#9e6432" stroke-width="0.8" stroke-linecap="round" />
      </g>
    </g>

    {#if state === "listening"}
      <g class="cat-waves" fill="none" stroke="#5f879f" stroke-width="2" stroke-linecap="round">
        <path d="M 120 70 Q 128 78 120 86" />
        <path d="M 189 68 Q 198 78 189 88" />
      </g>
    {:else if state === "writing"}
      <g class="cat-writing" stroke-linecap="round">
        <rect x="108" y="121" width="55" height="14" rx="3" fill="#fffaf0" stroke="#b7aa9a" stroke-width="0.8" />
        <path d="M 115 127 H 154 M 115 131 H 146" stroke="#9fb2c4" stroke-width="0.9" />
      </g>
    {:else if state === "pasting"}
      <g class="cat-success" fill="#eab64c">
        <path d="M 112 71 L 114 76 L 119 78 L 114 80 L 112 85 L 110 80 L 105 78 L 110 76 Z" />
        <path d="M 129 62 L 130.2 65 L 133.2 66.2 L 130.2 67.4 L 129 70.4 L 127.8 67.4 L 124.8 66.2 L 127.8 65 Z" />
      </g>
    {/if}

    {#if phewActive}
      <path class="avatar-phew" d="M 188 59 Q 185 51 188 44 Q 191 51 188 59 Z" fill="#70c9ff" stroke="#1d1d1f" stroke-width="0.8" />
    {/if}
  </svg>
{:else if skin === "spark-buddy"}
  <svg
    class="character rich-avatar spark-buddy-skin"
    viewBox="0 0 150 160"
    xmlns="http://www.w3.org/2000/svg"
    data-state={state}
    data-mode={mode}
    data-hover={hovering ? "1" : "0"}
    aria-hidden="true"
  >
    <defs>
      <linearGradient id="wf-spark-gold" x1="0.3" y1="0" x2="0.72" y2="1">
        <stop offset="0%" stop-color="#ffe878" />
        <stop offset="55%" stop-color="#ffc928" />
        <stop offset="100%" stop-color="#e89d11" />
      </linearGradient>
      <linearGradient id="wf-spark-belly" x1="0.4" y1="0" x2="0.65" y2="1">
        <stop offset="0%" stop-color="#fff5b5" />
        <stop offset="100%" stop-color="#f8d76e" />
      </linearGradient>
      <filter id="wf-spark-glow" x="-40%" y="-40%" width="180%" height="180%">
        <feDropShadow dx="0" dy="0" stdDeviation="3" flood-color="#50ffe2" flood-opacity="0.8" />
      </filter>
    </defs>

    <ellipse class="avatar-shadow" cx="75" cy="145" rx="36" ry="8" fill="rgba(36, 24, 0, 0.18)" />
    <g class="spark-tail">
      <path d="M 112 111 L 135 100 L 126 119 L 142 118 L 112 142 L 120 122 Z" fill="url(#wf-spark-gold)" stroke="#b36d00" stroke-width="1.3" />
      <path d="M 119 114 L 131 108 L 124 120 L 133 119 L 119 132 L 123 121 Z" fill="#fff083" opacity="0.9" />
    </g>

    <g class="spark-body">
      <ellipse cx="75" cy="98" rx="36" ry="43" fill="url(#wf-spark-gold)" stroke="#b36d00" stroke-width="1.4" />
      <ellipse cx="75" cy="116" rx="19" ry="21" fill="url(#wf-spark-belly)" opacity="0.92" />
      <path class="spark-ear spark-ear-l" d="M 48 55 L 30 15 L 59 32 L 52 8 L 78 54 Z" fill="url(#wf-spark-gold)" stroke="#b36d00" stroke-width="1.4" />
      <path class="spark-ear spark-ear-r" d="M 102 55 L 120 15 L 91 32 L 98 8 L 72 54 Z" fill="url(#wf-spark-gold)" stroke="#b36d00" stroke-width="1.4" />
      <path d="M 46 33 L 58 41 L 54 26" fill="none" stroke="#fff28b" stroke-width="5" stroke-linecap="round" opacity="0.9" />
      <path d="M 104 33 L 92 41 L 96 26" fill="none" stroke="#fff28b" stroke-width="5" stroke-linecap="round" opacity="0.9" />
      <circle cx="75" cy="72" r="31" fill="url(#wf-spark-gold)" stroke="#b36d00" stroke-width="1.4" />
      <g class="spark-eyes">
        <ellipse cx={64 + eyeShiftX * 0.7} cy={70 + eyeShiftY * 0.45} rx="6.1" ry={blinkOpen ? 7.6 : 0.7} fill="#211609" />
        <circle cx={62 + eyeShiftX * 0.45} cy={67.5 + eyeShiftY * 0.3} r={blinkOpen ? 1.9 : 0} fill="#fff" />
        <ellipse cx={86 + eyeShiftX * 0.7} cy={70 + eyeShiftY * 0.45} rx="6.1" ry={blinkOpen ? 7.6 : 0.7} fill="#211609" />
        <circle cx={84 + eyeShiftX * 0.45} cy={67.5 + eyeShiftY * 0.3} r={blinkOpen ? 1.9 : 0} fill="#fff" />
      </g>
      <ellipse cx="50" cy="82" rx="5" ry="3.2" fill="#55ffe4" opacity="0.88" filter="url(#wf-spark-glow)" />
      <ellipse cx="100" cy="82" rx="5" ry="3.2" fill="#55ffe4" opacity="0.88" filter="url(#wf-spark-glow)" />
      <path d="M 70.5 80 Q 75 84 79.5 80" fill="none" stroke="#6e470a" stroke-width="1.5" stroke-linecap="round" />
    </g>

    {#if state === "listening"}
      <g class="spark-wave" stroke="#5dffe8" stroke-width="2.4" fill="none" stroke-linecap="round" filter="url(#wf-spark-glow)">
        <path d="M 24 82 H 32 L 37 72 L 43 93 L 49 78 L 55 82" />
        <path d="M 96 82 H 104 L 109 72 L 115 93 L 121 78 L 128 82" />
      </g>
    {:else if state === "thinking"}
      <g class="spark-orbit" fill="none" stroke="#5dffe8" stroke-width="1.9" stroke-linecap="round" filter="url(#wf-spark-glow)">
        <ellipse cx="75" cy="39" rx="35" ry="9" />
        <path d="M 53 23 L 57 23 L 54 30 L 60 30 L 51 42 L 54 33 L 49 33 Z" fill="#ffd41f" stroke="#d39400" />
        <path d="M 101 24 L 105 24 L 102 31 L 108 31 L 99 43 L 102 34 L 97 34 Z" fill="#ffd41f" stroke="#d39400" />
      </g>
    {:else if state === "writing"}
      <g class="spark-tablet">
        <rect x="84" y="111" width="36" height="24" rx="5" fill="#384452" stroke="#19212b" stroke-width="1" />
        <path d="M 90 126 Q 101 116 114 126" fill="none" stroke="#5dffe8" stroke-width="2.2" stroke-linecap="round" filter="url(#wf-spark-glow)" />
      </g>
    {:else if state === "pasting"}
      <g class="spark-confetti" fill="#5dffe8" filter="url(#wf-spark-glow)">
        <path d="M 31 69 L 33 74 L 38 76 L 33 78 L 31 83 L 29 78 L 24 76 L 29 74 Z" />
        <path d="M 116 61 L 118 66 L 123 68 L 118 70 L 116 75 L 114 70 L 109 68 L 114 66 Z" />
      </g>
    {/if}

    {#if phewActive}
      <path class="avatar-phew" d="M 109 58 Q 106 50 109 43 Q 112 50 109 58 Z" fill="#70c9ff" stroke="#1d1d1f" stroke-width="0.8" />
    {/if}
  </svg>
{/if}

<style>
  .rich-avatar {
    overflow: visible;
    pointer-events: none;
    transition: width 320ms ease, height 320ms ease, filter 180ms ease;
  }

  .codex-fox-skin {
    width: calc(148px * var(--fscale, 1) * var(--state-scale, 1));
    height: calc(142px * var(--fscale, 1) * var(--state-scale, 1));
    filter: drop-shadow(0 7px 12px rgba(73, 48, 21, 0.22));
  }

  .oru-gujia-skin {
    width: calc(210px * var(--fscale, 1) * var(--state-scale, 1));
    height: calc(138px * var(--fscale, 1) * var(--state-scale, 1));
    filter: drop-shadow(0 6px 10px rgba(68, 45, 24, 0.18));
    animation: none;
  }

  .spark-buddy-skin {
    width: calc(138px * var(--fscale, 1) * var(--state-scale, 1));
    height: calc(140px * var(--fscale, 1) * var(--state-scale, 1));
    filter: drop-shadow(0 7px 12px rgba(107, 77, 0, 0.2));
  }

  .avatar-shadow {
    animation: avatar-shadow-breathe 3.4s ease-in-out infinite;
  }
  @keyframes avatar-shadow-breathe {
    0%, 100% { transform: scaleX(1); opacity: 1; }
    50% { transform: scaleX(0.86); opacity: 0.65; }
  }

  .codex-head,
  .spark-body,
  .oru-head,
  .gujia-head {
    transform-box: fill-box;
    transform-origin: center bottom;
    transition: transform 240ms cubic-bezier(0.34, 1.4, 0.64, 1);
  }

  .codex-fox-skin[data-state="idle"] .codex-head,
  .spark-buddy-skin[data-state="idle"] .spark-body {
    animation: rich-breathe 3.5s ease-in-out infinite;
  }
  .codex-fox-skin[data-hover="1"][data-state="idle"] .codex-head,
  .spark-buddy-skin[data-hover="1"][data-state="idle"] .spark-body {
    transform: rotate(3deg) translateY(-2px);
  }
  @keyframes rich-breathe {
    0%, 100% { transform: translateY(0) rotate(0deg); }
    50% { transform: translateY(-2px) rotate(1.4deg); }
  }

  .codex-tail,
  .spark-tail,
  .oru-tail,
  .gujia-tail {
    transform-box: fill-box;
    transform-origin: 50% 80%;
    animation: tail-swish 4.4s ease-in-out infinite;
  }
  @keyframes tail-swish {
    0%, 100% { transform: rotate(0deg); }
    50% { transform: rotate(-5deg); }
  }

  .codex-ear,
  .spark-ear,
  .oru-ear-l,
  .oru-ear-r,
  .gujia-ear-l,
  .gujia-ear-r {
    transform-box: fill-box;
    transform-origin: 50% 80%;
  }

  .codex-fox-skin[data-state="listening"] .codex-ear,
  .spark-buddy-skin[data-state="listening"] .spark-ear,
  .oru-gujia-skin[data-state="listening"] .oru-ear-l,
  .oru-gujia-skin[data-state="listening"] .oru-ear-r,
  .oru-gujia-skin[data-state="listening"] .gujia-ear-l,
  .oru-gujia-skin[data-state="listening"] .gujia-ear-r {
    animation: ear-perk 0.9s ease-in-out infinite;
  }
  @keyframes ear-perk {
    0%, 100% { transform: rotate(0deg) translateY(0); }
    50% { transform: rotate(-4deg) translateY(-2px); }
  }

  .codex-fox-skin[data-state="listening"],
  .spark-buddy-skin[data-state="listening"] {
    animation: listen-bounce 1.25s ease-in-out infinite;
  }
  @keyframes listen-bounce {
    0%, 100% { transform: translateY(0); }
    50% { transform: translateY(-3px); }
  }

  .codex-fox-skin[data-state="thinking"] .codex-head,
  .spark-buddy-skin[data-state="thinking"] .spark-body,
  .oru-gujia-skin[data-state="thinking"] .gujia-head {
    animation: think-tilt 1.8s ease-in-out infinite;
  }
  @keyframes think-tilt {
    0%, 100% { transform: rotate(-2.5deg); }
    50% { transform: rotate(3deg) translateY(-1px); }
  }

  .codex-fox-skin[data-state="writing"] .codex-pencil,
  .spark-buddy-skin[data-state="writing"] .spark-tablet {
    animation: write-tap 0.42s ease-in-out infinite alternate;
    transform-box: fill-box;
    transform-origin: center;
  }
  .oru-gujia-skin[data-state="writing"] .oru-paw-l {
    animation: paw-tap 0.28s ease-in-out infinite alternate;
  }
  .oru-gujia-skin[data-state="writing"] .oru-paw-r {
    animation: paw-tap 0.28s ease-in-out 0.14s infinite alternate;
  }
  @keyframes write-tap {
    from { transform: translateY(0) rotate(-1deg); }
    to { transform: translateY(2px) rotate(2deg); }
  }
  @keyframes paw-tap {
    from { transform: translateY(0); }
    to { transform: translateY(-4px); }
  }

  .codex-fox-skin[data-state="pasting"],
  .spark-buddy-skin[data-state="pasting"],
  .oru-gujia-skin[data-state="pasting"] .oru,
  .oru-gujia-skin[data-state="pasting"] .gujia {
    animation: paste-pop 0.7s cubic-bezier(0.34, 1.56, 0.64, 1) both;
  }
  @keyframes paste-pop {
    0% { transform: translateY(0) scale(1); }
    42% { transform: translateY(-10px) scale(1.05, 0.96); }
    100% { transform: translateY(0) scale(1); }
  }

  .codex-waves path,
  .cat-waves path,
  .spark-wave path {
    animation: wave-pulse 900ms ease-in-out infinite;
  }
  .codex-waves path:nth-child(2),
  .cat-waves path:nth-child(2),
  .spark-wave path:nth-child(2) {
    animation-delay: 160ms;
  }
  @keyframes wave-pulse {
    0%, 100% { opacity: 0.35; transform: translateX(0); }
    50% { opacity: 1; transform: translateX(2px); }
  }

  .codex-think,
  .spark-orbit,
  .gujia-thought {
    animation: orbit-drift 2.1s ease-in-out infinite;
    transform-box: fill-box;
    transform-origin: center;
  }
  @keyframes orbit-drift {
    0%, 100% { transform: translateY(0) rotate(0deg); }
    50% { transform: translateY(-3px) rotate(4deg); }
  }

  .codex-success,
  .cat-success,
  .spark-confetti {
    animation: sparkle-rise 0.95s ease-out infinite;
    transform-box: fill-box;
    transform-origin: center;
  }
  @keyframes sparkle-rise {
    0% { opacity: 0; transform: translateY(6px) scale(0.75); }
    35% { opacity: 1; transform: translateY(0) scale(1); }
    100% { opacity: 0; transform: translateY(-9px) scale(1.15); }
  }

  .oru {
    transform-box: fill-box;
    transform-origin: 50% 90%;
    animation: oru-fidget 3.2s ease-in-out infinite;
  }
  .gujia {
    transform-box: fill-box;
    transform-origin: 50% 90%;
    animation: gujia-calm 4.8s ease-in-out infinite;
  }
  .oru-gujia-skin[data-hover="1"][data-state="idle"] .oru {
    animation: oru-fidget 1.2s ease-in-out infinite;
  }
  @keyframes oru-fidget {
    0%, 100% { transform: rotate(0deg) translateY(0); }
    35% { transform: rotate(-1.8deg) translateY(-1px); }
    70% { transform: rotate(1.6deg); }
  }
  @keyframes gujia-calm {
    0%, 100% { transform: translateY(0); }
    50% { transform: translateY(-1px); }
  }

  .oru-gujia-skin[data-state="listening"] .oru,
  .oru-gujia-skin[data-state="listening"] .gujia {
    animation: cat-alert 1.1s ease-in-out infinite;
  }
  @keyframes cat-alert {
    0%, 100% { transform: translateY(0) rotate(0deg); }
    50% { transform: translateY(-3px) rotate(1deg); }
  }

  .spark-buddy-skin[data-hover="1"] {
    filter:
      drop-shadow(0 7px 12px rgba(107, 77, 0, 0.22))
      drop-shadow(0 0 10px rgba(80, 255, 226, 0.34));
  }

  .avatar-phew {
    animation: phew-drop 850ms ease-out both;
  }
  @keyframes phew-drop {
    0% { opacity: 0; transform: translateY(-5px) scale(0.8); }
    35% { opacity: 1; }
    100% { opacity: 0; transform: translateY(7px) scale(1); }
  }
</style>
