// Native `window.confirm` / `window.alert` replacements.
//
// WHY THIS FILE EXISTS: inside a Tauri webview the native modals are a
// deadlock. They block the webview's main thread while they are open, but
// Tauri's IPC bridge needs that same thread to service `invoke()` — so any
// code that confirms and then calls the backend can wedge the whole app with
// the dialog still on screen. The only way out is killing the process.
//
// This bit for real: "Rerun" in History called `confirm()` before invoking
// `rerun_transcription`, and hung every time on a recording that already had
// a transcript. Uploading the identical file worked, because the upload path
// never confirms anything.
//
// The plugin equivalents render the same OS dialog from the Rust side and
// return a promise, so the webview thread stays free.
//
// Do not reintroduce `window.confirm` / `window.alert` anywhere in this app.
import { confirm as pluginConfirm, message as pluginMessage } from "@tauri-apps/plugin-dialog";

/** Yes/no question. Returns false if the dialog itself fails, so a failure
 *  can never be read as consent to a destructive action. */
export async function askConfirm(text: string, title = "wispr-fox"): Promise<boolean> {
  try {
    return await pluginConfirm(text, { title, kind: "warning" });
  } catch (e) {
    console.error("confirm dialog failed", e);
    return false;
  }
}

/** Informational / error popup. Never throws — the caller is usually already
 *  on an error path and a failed popup must not mask the original problem. */
export async function showMessage(text: string, title = "wispr-fox"): Promise<void> {
  try {
    await pluginMessage(text, { title, kind: "error" });
  } catch (e) {
    console.error("message dialog failed", e, "original message:", text);
  }
}
