# macOS code signing — making the Accessibility grant stick

**Problem this solves:** on macOS, the Accessibility permission (the one that
lets wispr-fox auto-paste) is tied to the app's *code-signing identity*. Our
nightly builds were **unsigned**, so macOS identified the app by a hash that
changes on every build. Result: **every update silently invalidated the
grant** — the banner kept saying "not enabled" and auto-paste stopped working
until you removed and re-added wispr-fox in System Settings.

**The fix:** sign every CI build with **one stable self-signed certificate**.
The signing identity never changes, so macOS keeps the Accessibility grant
across updates. You grant it **once** and it sticks.

- ✅ **No paid Apple Developer account** ($99/yr) needed.
- ✅ One-time setup (~5 minutes), then it's automatic forever.
- ⚠️ This does **not** notarize the app. Gatekeeper's first-launch step is
  unchanged: right-click → Open (or `xattr -dr com.apple.quarantine
  /Applications/wispr-fox.app`). That's a one-time-per-install thing and is
  separate from the Accessibility persistence this fixes.

Until the three secrets below exist, CI builds **unsigned exactly as before** —
so nothing breaks in the meantime.

---

## One-time setup (do this once, on your Mac)

### 1. Create a self-signed Code Signing certificate

1. Open **Keychain Access** (⌘-Space → "Keychain Access").
2. Menu bar → **Keychain Access → Certificate Assistant → Create a Certificate…**
3. Fill in:
   - **Name:** `wispr-fox self-signed` (remember this exact string — it's your
     `APPLE_SIGNING_IDENTITY`)
   - **Identity Type:** Self Signed Root
   - **Certificate Type:** **Code Signing**
   - (Optional) tick "Let me override defaults" and bump the validity to a few
     thousand days so it doesn't expire. Defaults are otherwise fine.
4. Click **Create** → Continue through the warning → Done. It lands in your
   **login** keychain.

### 2. Export it as a `.p12`

1. In Keychain Access, select the **login** keychain → **My Certificates**.
2. Find **`wispr-fox self-signed`**, expand it (there's a private key under it),
   right-click the certificate → **Export "wispr-fox self-signed"…**
3. Save as `wispr-fox-signing.p12`.
4. Set a password when prompted — **remember it**, it becomes
   `APPLE_CERTIFICATE_PASSWORD`.

### 3. Base64-encode the `.p12`

In Terminal:

```bash
base64 -i wispr-fox-signing.p12 | pbcopy
```

That copies the base64 blob to your clipboard (it's the value of
`APPLE_CERTIFICATE`).

### 4. Add three repository secrets

GitHub → the `wispr-fox` repo → **Settings → Secrets and variables → Actions →
New repository secret**. Add:

| Secret name | Value |
|---|---|
| `APPLE_CERTIFICATE` | the base64 blob from step 3 (paste from clipboard) |
| `APPLE_CERTIFICATE_PASSWORD` | the password you set in step 2 |
| `APPLE_SIGNING_IDENTITY` | `wispr-fox self-signed` (the exact name from step 1) |

### 5. Enable signing in CI, then cut a build

After the three secrets exist, **uncomment** the three signing lines in
`.github/workflows/release.yml` (the `APPLE_CERTIFICATE` /
`APPLE_CERTIFICATE_PASSWORD` / `APPLE_SIGNING_IDENTITY` block — they're kept
commented because tauri-action treats an *empty* identity as "please sign" and
then fails the macOS build). Commit, tag + push as usual. The macOS build now
signs with your cert. Once you install that build and grant Accessibility
**once**, every future signed build keeps the grant.

> If you ever rotate the cert (new identity), the grant resets one more time —
> so don't regenerate it casually. One cert, kept forever, is the whole point.

---

## How to verify it worked

On the Mac, after installing a signed build:

```bash
codesign -dv --verbose=4 /Applications/wispr-fox.app 2>&1 | grep Authority
```

You should see `Authority=wispr-fox self-signed` (not "code object is not
signed at all"). After that, granting Accessibility once should survive the
next update.

---

## Security notes

- The `.p12` and its password are **only** in GitHub Actions secrets (encrypted
  at rest, never printed in logs). Don't commit the `.p12` to the repo.
- A self-signed cert grants **no** trust to anyone else — it can't be used to
  impersonate an Apple developer or pass notarization. Its only job here is to
  give *this* app a stable identity on the *user's own* machine.
- Keep a backup of the `.p12` somewhere safe (a password manager). If you lose
  it, you can make a new one — users just re-grant Accessibility one final time.
