#!/usr/bin/env node
/**
 * model-check — prove every model wispr-fox offers actually works with YOUR keys.
 *
 * Why this exists: the model pickers are hardcoded lists. Providers retire model
 * ids without warning (Groq dropped `distil-whisper-large-v3-en`, ElevenLabs
 * dropped `scribe_v1`, and `llama-4-maverick` was never a real id at all). A
 * dead id in the list is invisible until a user picks it and a dictation fails.
 *
 * What it does:
 *   1. Reads the model ids straight out of `src/lib/provider-options.ts`, so
 *      this check can never drift from what the app actually ships.
 *   2. Asks each provider what it currently serves (`/models`).
 *   3. Sends one minimal real request per model and reports what came back.
 *
 * KEYS ARE NEVER PRINTED, LOGGED, OR WRITTEN ANYWHERE. They are read from the
 * app's own DPAPI-encrypted store (Windows) or the environment, held in memory
 * for the duration of the run, and used only against the provider's own API.
 *
 * Usage:
 *   node scripts/model-check.mjs              # test everything you have a key for
 *   node scripts/model-check.mjs groq gemini  # only these providers
 *   node scripts/model-check.mjs --list       # just show what each provider serves
 *
 * Exit code is non-zero if any shipped model id failed, so CI can gate on it.
 */

import { readFileSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");

// ── 1. The shipped model lists, read from the app's own source ──────────────

/** Pull `id: "..."` out of one `Record<string, ProviderModel[]>` literal. */
function parseModelTable(src, constName) {
  const start = src.indexOf(`export const ${constName}`);
  if (start === -1) throw new Error(`${constName} not found in provider-options.ts`);
  // The table ends at the first `};` at column 0 after the declaration.
  const end = src.indexOf("\n};", start);
  const body = src.slice(start, end);

  const table = {};
  // Each provider key introduces a `[ ... ]` array of model objects.
  const providerRe = /^\s{2}(\w+):\s*\[/gm;
  let m;
  const marks = [];
  while ((m = providerRe.exec(body))) marks.push({ name: m[1], from: m.index });
  marks.forEach((mark, i) => {
    const chunk = body.slice(mark.from, i + 1 < marks.length ? marks[i + 1].from : body.length);
    // Ignore ids inside `//` comments — retired models are documented there.
    const ids = [...chunk.matchAll(/^(?!\s*\/\/).*?\bid:\s*"([^"]+)"/gm)].map((x) => x[1]);
    if (ids.length) table[mark.name] = ids;
  });
  return table;
}

const providerOptions = readFileSync(join(ROOT, "src/lib/provider-options.ts"), "utf8");
const STT_MODELS = parseModelTable(providerOptions, "STT_MODELS");
const LLM_MODELS = parseModelTable(providerOptions, "LLM_MODELS");

// ── 2. Keys, read from the app's own store. Never printed. ──────────────────

/**
 * The app is keyring-first with a DPAPI-encrypted fallback file. On this
 * machine the keyring migration has been failing, so the fallback is what the
 * app actually reads — and it is also the only store reachable from a plain
 * script, since DPAPI decrypts under the current Windows user automatically.
 */
function readKeysWindows() {
  const ps = `
    $ErrorActionPreference = 'Stop'
    Add-Type -AssemblyName System.Security
    $p = Join-Path $env:APPDATA 'com.wispr-fox.app\\.keys.enc.json'
    if (-not (Test-Path $p)) { '{}' ; exit }
    $d = Get-Content $p -Raw | ConvertFrom-Json
    $out = @{}
    foreach ($n in $d.entries.PSObject.Properties.Name) {
      $b = [Convert]::FromBase64String($d.entries.$n)
      $out[$n] = [Text.Encoding]::UTF8.GetString(
        [Security.Cryptography.ProtectedData]::Unprotect($b, $null, 'CurrentUser'))
    }
    $out | ConvertTo-Json -Compress
  `;
  try {
    const raw = execFileSync("powershell", ["-NoProfile", "-NonInteractive", "-Command", ps], {
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
    });
    return JSON.parse(raw.trim() || "{}");
  } catch (e) {
    console.error("could not read the local key store:", e.message);
    return {};
  }
}

function loadKeys() {
  const stored = process.platform === "win32" ? readKeysWindows() : {};
  // Environment always wins, so this runs anywhere (CI, mac, a colleague's box).
  const pick = (entry, ...envNames) =>
    envNames.map((n) => process.env[n]).find(Boolean) || stored[entry] || null;
  return {
    groq: pick("groq_llm_key", "GROQ_API_KEY") || pick("groq_stt_key", "GROQ_API_KEY"),
    groqStt: pick("groq_stt_key", "GROQ_API_KEY"),
    gemini: pick("gemini_llm_key", "GEMINI_API_KEY", "GOOGLE_API_KEY"),
    openai: pick("openai_llm_key", "OPENAI_API_KEY") || pick("openai_stt_key", "OPENAI_API_KEY"),
    openaiStt: pick("openai_stt_key", "OPENAI_API_KEY"),
    deepgram: pick("deepgram_stt_key", "DEEPGRAM_API_KEY"),
    elevenlabs: pick("elevenlabs_stt_key", "ELEVENLABS_API_KEY"),
  };
}

// ── 3. A tiny synthetic WAV, so no real recording is ever uploaded ──────────

/** 1s of 16 kHz mono 16-bit tone. Enough for an STT endpoint to accept and
 *  answer; deliberately synthetic so no personal audio leaves the machine. */
function testWav() {
  const rate = 16000, secs = 1, n = rate * secs;
  const data = Buffer.alloc(n * 2);
  for (let i = 0; i < n; i++) {
    // Two stacked tones in the speech band at a modest level.
    const s = 0.25 * Math.sin((2 * Math.PI * 220 * i) / rate)
            + 0.15 * Math.sin((2 * Math.PI * 440 * i) / rate);
    data.writeInt16LE(Math.max(-1, Math.min(1, s)) * 32767, i * 2);
  }
  const head = Buffer.alloc(44);
  head.write("RIFF", 0); head.writeUInt32LE(36 + data.length, 4); head.write("WAVE", 8);
  head.write("fmt ", 12); head.writeUInt32LE(16, 16); head.writeUInt16LE(1, 20);
  head.writeUInt16LE(1, 22); head.writeUInt32LE(rate, 24); head.writeUInt32LE(rate * 2, 28);
  head.writeUInt16LE(2, 32); head.writeUInt16LE(16, 34);
  head.write("data", 36); head.writeUInt32LE(data.length, 40);
  return Buffer.concat([head, data]);
}
const WAV = testWav();

// ── 4. Classifying what came back ───────────────────────────────────────────

const OK = "ok", GONE = "gone", AUTH = "auth", LIMIT = "limit", PAID = "paid", ERR = "error";

const MARK = { [OK]: "PASS", [GONE]: "GONE", [AUTH]: "AUTH", [LIMIT]: "LIMIT", [PAID]: "PAID", [ERR]: "FAIL" };

/** Map an HTTP failure onto the distinction that actually matters to a user:
 *  is the model dead, is my key wrong, am I throttled, or must I pay? */
function classify(status, body) {
  const text = (body || "").toLowerCase();
  if (status === 200) return OK;
  if (status === 401 || status === 403) {
    return /quota|billing|balance|payment|not have access|permission/.test(text) ? PAID : AUTH;
  }
  if (status === 429) return /quota|billing|credit|exceeded your current/.test(text) ? PAID : LIMIT;
  if (status === 404) return GONE;
  if (status === 400 && /model|decommission|not found|does not exist|unsupported|invalid_model/.test(text)) return GONE;
  if (status === 402) return PAID;
  return ERR;
}

async function call(url, init) {
  try {
    const res = await fetch(url, { ...init, signal: AbortSignal.timeout(60_000) });
    const body = await res.text();
    return { status: res.status, body };
  } catch (e) {
    return { status: 0, body: String(e.message || e) };
  }
}

/** One-line reason, with any key-shaped token scrubbed for safety. */
function reason(status, body) {
  let msg = "";
  try {
    const j = JSON.parse(body);
    msg = j?.error?.message || j?.error?.status || j?.message || j?.detail?.message || "";
  } catch { msg = body.slice(0, 160); }
  msg = String(msg).replace(/\b(sk|gsk|xi|dg)[-_][A-Za-z0-9_-]{8,}/g, "<redacted>").replace(/\s+/g, " ");
  return `${status || "network"} ${msg}`.trim().slice(0, 150);
}

// ── 5. Per-provider probes ──────────────────────────────────────────────────

const CHAT_BODY = (model) => JSON.stringify({
  model,
  messages: [{ role: "user", content: "Reply with the single word: ok" }],
  max_completion_tokens: 16,
});

const probes = {
  groq: {
    key: (k) => k.groq,
    async list(key) {
      const r = await call("https://api.groq.com/openai/v1/models", {
        headers: { Authorization: `Bearer ${key}` },
      });
      if (r.status !== 200) return null;
      return JSON.parse(r.body).data.map((m) => m.id);
    },
    llm: (key, model) => call("https://api.groq.com/openai/v1/chat/completions", {
      method: "POST",
      headers: { Authorization: `Bearer ${key}`, "Content-Type": "application/json" },
      body: CHAT_BODY(model),
    }),
    stt(key, model) {
      const fd = new FormData();
      fd.append("file", new Blob([WAV], { type: "audio/wav" }), "test.wav");
      fd.append("model", model);
      return call("https://api.groq.com/openai/v1/audio/transcriptions", {
        method: "POST", headers: { Authorization: `Bearer ${key}` }, body: fd,
      });
    },
  },

  openai: {
    key: (k) => k.openai,
    async list(key) {
      const r = await call("https://api.openai.com/v1/models", {
        headers: { Authorization: `Bearer ${key}` },
      });
      if (r.status !== 200) return null;
      return JSON.parse(r.body).data.map((m) => m.id);
    },
    llm: (key, model) => call("https://api.openai.com/v1/chat/completions", {
      method: "POST",
      headers: { Authorization: `Bearer ${key}`, "Content-Type": "application/json" },
      body: CHAT_BODY(model),
    }),
    stt(key, model) {
      const fd = new FormData();
      fd.append("file", new Blob([WAV], { type: "audio/wav" }), "test.wav");
      fd.append("model", model);
      return call("https://api.openai.com/v1/audio/transcriptions", {
        method: "POST", headers: { Authorization: `Bearer ${key}` }, body: fd,
      });
    },
  },

  gemini: {
    key: (k) => k.gemini,
    async list(key) {
      const r = await call(
        `https://generativelanguage.googleapis.com/v1beta/models?key=${key}&pageSize=200`);
      if (r.status !== 200) return null;
      return JSON.parse(r.body).models.map((m) => m.name.replace(/^models\//, ""));
    },
    // Give thinking models enough budget to emit a token; a 200 with empty text
    // still proves the id is live, which is what this check is for.
    llm: (key, model) => call(
      `https://generativelanguage.googleapis.com/v1beta/models/${model}:generateContent?key=${key}`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          contents: [{ parts: [{ text: "Reply with the single word: ok" }] }],
          generationConfig: { maxOutputTokens: 2048 },
        }),
      }),
  },

  deepgram: {
    key: (k) => k.deepgram,
    async list(key) {
      // Deepgram has no /models list; prove the key and surface the balance,
      // which is the number that actually matters against the $200 credit.
      const r = await call("https://api.deepgram.com/v1/projects", {
        headers: { Authorization: `Token ${key}` },
      });
      if (r.status !== 200) return null;
      const projects = JSON.parse(r.body).projects || [];
      const out = [];
      for (const p of projects) {
        const b = await call(`https://api.deepgram.com/v1/projects/${p.project_id}/balances`, {
          headers: { Authorization: `Token ${key}` },
        });
        if (b.status === 200) {
          for (const bal of JSON.parse(b.body).balances || []) {
            out.push(`balance: $${Number(bal.amount).toFixed(2)} ${bal.units || ""}`.trim());
          }
        }
      }
      return out.length ? out : ["(key valid, no balance reported)"];
    },
    stt: (key, model) => call(`https://api.deepgram.com/v1/listen?model=${model}`, {
      method: "POST",
      headers: { Authorization: `Token ${key}`, "Content-Type": "audio/wav" },
      body: WAV,
    }),
  },

  elevenlabs: {
    key: (k) => k.elevenlabs,
    async list(key) {
      const r = await call("https://api.elevenlabs.io/v1/user/subscription", {
        headers: { "xi-api-key": key },
      });
      if (r.status !== 200) return null;
      const s = JSON.parse(r.body);
      return [`tier: ${s.tier}`, `credits: ${s.character_count}/${s.character_limit} used`];
    },
    stt(key, model) {
      const fd = new FormData();
      fd.append("file", new Blob([WAV], { type: "audio/wav" }), "test.wav");
      fd.append("model_id", model);
      return call("https://api.elevenlabs.io/v1/speech-to-text", {
        method: "POST", headers: { "xi-api-key": key }, body: fd,
      });
    },
  },
};

// ── 6. Run ──────────────────────────────────────────────────────────────────

const args = process.argv.slice(2);
const listOnly = args.includes("--list");
const only = args.filter((a) => !a.startsWith("--"));

const keys = loadKeys();
const results = [];

console.log("wispr-fox model check");
console.log("model ids read from src/lib/provider-options.ts · keys are never printed\n");

for (const provider of Object.keys(probes)) {
  if (only.length && !only.includes(provider)) continue;
  const probe = probes[provider];
  const key = probe.key(keys);

  if (!key) {
    console.log(`${provider.toUpperCase()}  — no key stored, skipped`);
    console.log(`  ${[...(STT_MODELS[provider] || []), ...(LLM_MODELS[provider] || [])].join(", ") || "(no models)"}\n`);
    continue;
  }

  console.log(`${provider.toUpperCase()}`);
  const served = await probe.list(key);
  if (served === null) {
    console.log("  could not list models — key rejected or endpoint unreachable");
  } else if (provider === "deepgram" || provider === "elevenlabs") {
    served.forEach((s) => console.log(`  ${s}`));
  } else {
    console.log(`  provider currently serves ${served.length} models`);
  }

  const shipped = [
    ...(STT_MODELS[provider] || []).map((id) => ({ id, kind: "stt" })),
    ...(LLM_MODELS[provider] || []).map((id) => ({ id, kind: "llm" })),
  ];

  for (const { id, kind } of shipped) {
    if (listOnly) {
      const known = served && !["deepgram", "elevenlabs"].includes(provider)
        ? (served.includes(id) ? "listed" : "NOT LISTED")
        : "-";
      console.log(`  ${kind}  ${id.padEnd(32)} ${known}`);
      continue;
    }
    const fn = probe[kind];
    if (!fn) continue;
    const { status, body } = await fn(key, id);
    const verdict = classify(status, body);
    results.push({ provider, kind, id, verdict });
    const note = verdict === OK ? "" : `  ${reason(status, body)}`;
    console.log(`  ${MARK[verdict].padEnd(5)} ${kind}  ${id.padEnd(32)}${note}`);
  }
  console.log("");
}

if (!listOnly) {
  const bad = results.filter((r) => r.verdict === GONE || r.verdict === ERR);
  const paid = results.filter((r) => r.verdict === PAID);
  const limited = results.filter((r) => r.verdict === LIMIT);
  console.log(`${results.filter((r) => r.verdict === OK).length}/${results.length} shipped models answered`);
  if (limited.length) console.log(`rate-limited (transient, re-run later): ${limited.map((r) => r.id).join(", ")}`);
  if (paid.length) console.log(`needs billing: ${paid.map((r) => r.id).join(", ")}`);
  if (bad.length) {
    console.log(`\nBROKEN — these ids are offered in the app but do not work:`);
    bad.forEach((r) => console.log(`  ${r.provider} ${r.kind} ${r.id}`));
    process.exit(1);
  }
}
