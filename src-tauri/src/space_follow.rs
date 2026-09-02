//! Make the floater FOLLOW the user the instant they switch Spaces.
//!
//! The floater is pinned with `CanJoinAllSpaces | FullScreenAuxiliary` at
//! level 25 (see [`crate::pin_floater`]), which is the correct static recipe
//! for an overlay that rides over every Space including other apps' fullscreen
//! ones. In theory that pin alone is enough and this module should be
//! unnecessary.
//!
//! In practice it was not, and this is the bug the reporter actually hit: start
//! a dictation on Desktop 1, four-finger-swipe into a fullscreen app on its own
//! Space, and the avatar stayed behind on Desktop 1 instead of coming along. It
//! only re-appeared on the active Space when the 30-second watchdog next
//! re-asserted the pin — far too slow to feel like "it follows me".
//!
//! Two things conspire to produce that:
//!
//!   1. `CanJoinAllSpaces` is honoured lazily. When a window with that bit is
//!      also `alwaysOnTop`/transparent + `macOSPrivateApi` (this one is), the
//!      WindowServer does not always re-composite it onto a *fullscreen* Space
//!      at the moment of the switch — it needs a nudge (an `orderFront`) once
//!      the new Space is active.
//!   2. The only nudges the app had were launch, Dock-reopen, and a 30s timer.
//!      None of them fire on a Space switch, so between swipe and the next
//!      watchdog tick the floater is simply on the wrong Space.
//!
//! The fix is event-driven: `NSWorkspace` posts
//! `NSWorkspaceActiveSpaceDidChangeNotification` on its OWN notification centre
//! (not the default `NSNotificationCenter`) every time the active Space
//! changes, including four-finger swipes and moving into/out of a fullscreen
//! app. We observe it and, on each one, re-pin + order the floater onto the
//! Space that just became active. The webview keeps all of its own state
//! (recording / transcribing / done), so the avatar reappears on the new Space
//! showing exactly what it was showing before — which is the "retain its state"
//! part of the report.
//!
//! Cost: nothing until a Space actually changes, then two main-thread
//! Objective-C message sends. No polling.

/// Start observing active-Space changes and re-pin the floater on each.
/// No-op off macOS, where Spaces do not exist.
pub fn spawn(app: tauri::AppHandle) {
    #[cfg(target_os = "macos")]
    {
        // Observer install touches AppKit singletons and must run on the main
        // thread. `run_on_main_thread` from the setup hook is the supported way
        // in; if it fails the app still works, just without instant following
        // (the 30s watchdog remains the fallback), so we log and move on.
        let app_for_main = app.clone();
        let _ = app.run_on_main_thread(move || {
            macos::install(app_for_main);
        });
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use std::sync::Once;

    use objc2::declare::ClassBuilder;
    use objc2::rc::Retained;
    use objc2::runtime::{AnyClass, AnyObject, Sel};
    use objc2::{msg_send, sel};
    use tauri::Manager;

    // The AppHandle the observer callback needs to reach the floater window.
    // Set once, on the main thread, before the observer is installed; only ever
    // read from the observer callback, which also runs on the main thread. A
    // plain static is sufficient because both accesses are main-thread-only.
    static mut APP_HANDLE: Option<tauri::AppHandle> = None;
    static INSTALL: Once = Once::new();

    /// Install the NSWorkspace active-Space observer. Idempotent.
    pub fn install(app: tauri::AppHandle) {
        INSTALL.call_once(|| unsafe {
            // SAFETY: main thread (guaranteed by the caller in space_follow::spawn),
            // written exactly once here before any observer callback can read it.
            APP_HANDLE = Some(app);

            let observer = make_observer();

            // NSWorkspace has its OWN notification centre. The active-Space
            // notification is posted there, NOT on the default NSNotificationCenter
            // — observing the wrong centre is the classic reason this "silently
            // never fires".
            let workspace: *mut AnyObject = {
                let cls = AnyClass::get(c"NSWorkspace").expect("NSWorkspace class");
                msg_send![cls, sharedWorkspace]
            };
            let center: *mut AnyObject = msg_send![workspace, notificationCenter];

            // Notification name string.
            let name: *mut AnyObject = {
                let cls = AnyClass::get(c"NSString").expect("NSString class");
                let s = c"NSWorkspaceActiveSpaceDidChangeNotification";
                msg_send![cls, stringWithUTF8String: s.as_ptr()]
            };

            let _: () = msg_send![
                center,
                addObserver: &*observer,
                selector: sel!(activeSpaceDidChange:),
                name: name,
                object: std::ptr::null::<AnyObject>(),
            ];

            // Leak the observer intentionally: it must outlive this function for
            // the lifetime of the app, and there is exactly one.
            let _ = Retained::into_raw(observer);

            tracing::info!("space_follow: observing NSWorkspaceActiveSpaceDidChangeNotification");
        });
    }

    /// Build a tiny Objective-C class with one method, `activeSpaceDidChange:`,
    /// that re-pins the floater. Registered once per process.
    fn make_observer() -> Retained<AnyObject> {
        static REGISTER: Once = Once::new();
        static mut CLASS: *const AnyClass = std::ptr::null();

        REGISTER.call_once(|| {
            let superclass = AnyClass::get(c"NSObject").expect("NSObject");
            let mut builder = ClassBuilder::new(c"WisprSpaceObserver", superclass)
                .expect("WisprSpaceObserver already registered?");

            // extern "C" callback matching the ObjC selector signature
            // `- (void)activeSpaceDidChange:(NSNotification *)note`.
            // Receiver taken as a raw pointer, not `&AnyObject`: objc2's
            // `MethodImplementation` needs a single concrete lifetime, and a
            // `&AnyObject` argument makes the fn higher-ranked over lifetimes,
            // which the trait is not implemented for.
            extern "C" fn on_change(_this: *mut AnyObject, _cmd: Sel, _note: *mut AnyObject) {
                on_active_space_changed();
            }

            unsafe {
                builder.add_method(
                    sel!(activeSpaceDidChange:),
                    on_change as extern "C" fn(*mut AnyObject, Sel, *mut AnyObject),
                );
                CLASS = builder.register();
            }
        });

        unsafe {
            // SAFETY: CLASS is set inside the Once above before this runs.
            let cls: &AnyClass = &*CLASS;
            let obj: *mut AnyObject = msg_send![cls, new];
            Retained::from_raw(obj).expect("new returned nil")
        }
    }

    /// The active Space just changed — get the floater onto it, verifiably.
    fn on_active_space_changed() {
        use std::sync::atomic::{AtomicBool, Ordering};
        /// Debounce: a fast four-finger swipe THROUGH several Spaces fires one
        /// notification per Space. Running the handler for each caused visible
        /// stutter (and stacked "land" animations). One in-flight handler is
        /// enough — it acts on whatever Space the user has settled on by the
        /// time its delay elapses.
        static IN_FLIGHT: AtomicBool = AtomicBool::new(false);

        // SAFETY: main thread (NSWorkspace notifications are delivered on the
        // main thread), APP_HANDLE was set before the observer was installed.
        let app = unsafe {
            let ptr = std::ptr::addr_of!(APP_HANDLE);
            match (*ptr).as_ref() {
                Some(a) => a.clone(),
                None => return,
            }
        };

        if IN_FLIGHT
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
            .is_err()
        {
            return; // a handler is already pending; it will see the final Space
        }

        // The rest is async: the notification arrives DURING the swipe
        // animation. Waiting lets the transition settle AND collapses a rapid
        // multi-Space swipe into one pass over the final Space.
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
            IN_FLIGHT.store(false, Ordering::Release);

            let Some(win) = app.get_webview_window("clippy") else {
                return;
            };
            // Only touch a floater that is meant to be on screen. Re-pinning a
            // hidden one, or ordering it front, would resurrect a window the
            // user (or a setting) had deliberately hidden.
            if !win.is_visible().unwrap_or(false) {
                return;
            }

            // A non-activating NSPanel with CanJoinAllSpaces is carried across
            // Spaces by the WindowServer NATIVELY — by the time this runs the
            // floater is normally already on the new Space. Re-pinning /
            // re-ordering it anyway forces the transparent surface to redraw,
            // which the user sees as appear→disappear→reappear flicker on every
            // switch. So: ask first, and only touch the window when macOS says
            // it actually failed to follow (the rare repair case).
            let on_active = crate::space_probe::sample_once(&app)
                .map(|s| s.on_active_space)
                // Probe unavailable → assume fine; the 30s watchdog remains the
                // safety net, and NOT flickering is the better default.
                .unwrap_or(true);

            if !on_active {
                tracing::info!(
                    "space_follow: floater missing from active Space — repairing (pin + order front)"
                );
                crate::pin_floater(&win);
                crate::macos_order_front(&win);
            }

            // Tell the floater's webview it just hopped Spaces so it can play
            // the gentle "landed here" animation. Purely cosmetic — a failed
            // emit (webview mid-reload) just means no animation that one time.
            use tauri::Emitter;
            let _ = win.emit("wispr:space_changed", ());
        });
    }
}
