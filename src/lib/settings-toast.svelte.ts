// Shared toast state for Settings sub-pages. The Settings layout renders a
// single toast in its top-right; sub-pages call flash() to surface a
// short-lived confirmation ("Saved", "Deleted", etc).

let _msg = $state<string | null>(null);

export const toast = {
  get msg() {
    return _msg;
  },
};

let timer: ReturnType<typeof setTimeout> | null = null;

export function flash(msg: string) {
  _msg = msg;
  if (timer) clearTimeout(timer);
  timer = setTimeout(() => {
    _msg = null;
    timer = null;
  }, 1800);
}
