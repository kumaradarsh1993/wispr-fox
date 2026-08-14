//! macOS Touch Bar integration for wispr-fox.
//!
//! Provides native Touch Bar UI on 2016–2020 MacBook Pro models:
//!
//! | State      | Touch Bar layout                                   |
//! |------------|----------------------------------------------------|
//! | **Idle**   | `[🦊] [🦆] [🐱] [📎]  ┃  [💬 Light] [✏️ Draft]`  |
//! | **Recording** | `[🔴 0:12 ─ Listening]  ···  [⏹ Stop]`         |
//! | **Processing** | `[⏳ Transcribing · Groq]`                     |
//!
//! Attaches to NSApplication as the app-level fallback touch bar.
//! Visible whenever wispr-fox is the frontmost app.  On Macs without
//! Touch Bar hardware the API is a harmless no-op.
//!
//! All UI mutations dispatch to the main thread (AppKit requirement).

use std::cell::RefCell;
use std::ffi::CStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::time::Instant;

use objc2::rc::{Allocated, Retained};
use objc2::runtime::{AnyObject, NSObjectProtocol, ProtocolObject, Sel};
use objc2::{define_class, msg_send, ClassType, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSApplication, NSButton, NSColor, NSCustomTouchBarItem, NSTouchBar, NSTouchBarDelegate,
    NSTouchBarItem,
};
use objc2_foundation::{NSArray, NSObject, NSString};

use tauri::{AppHandle, Emitter, Listener};

use crate::flow::Flow;
use crate::settings::Mode;

// ---------------------------------------------------------------------------
// Global bridge — survives for the process lifetime, accessed by ObjC
// action methods which can't carry Rust closures.
// ---------------------------------------------------------------------------

struct Bridge {
    app: AppHandle,
    flow: Flow,
}

/// Written once at install time; read by button-action methods.
static BRIDGE: OnceLock<Bridge> = OnceLock::new();

/// Flipped to `true` while recording, drives the timer-update loop.
static RECORDING: AtomicBool = AtomicBool::new(false);

// Main-thread-only mutable state (thread_local is fine because all
// Touch Bar code dispatches to the main thread).
thread_local! {
    /// Retained strong ref — the delegate is a *weak* property on NSTouchBar,
    /// so we must prevent it from being deallocated.
    static DELEGATE: RefCell<Option<Retained<WisprTouchBarDelegate>>> = const { RefCell::new(None) };
    /// Retained ref to the NSTouchBar itself (also retained by NSApp, but
    /// we keep a handle for `setDefaultItemIdentifiers:` updates).
    static TB: RefCell<Option<Retained<NSTouchBar>>> = const { RefCell::new(None) };
    /// The timer button shown during recording — we update its title each
    /// second with the elapsed time.
    static TIMER_BTN: RefCell<Option<Retained<NSButton>>> = const { RefCell::new(None) };
    /// The status button shown during transcription/cleanup.
    static STATUS_BTN: RefCell<Option<Retained<NSButton>>> = const { RefCell::new(None) };
    /// When recording started (used to compute elapsed time).
    static REC_START: RefCell<Option<Instant>> = const { RefCell::new(None) };
}

// ---------------------------------------------------------------------------
// Item identifiers (reverse-DNS, one per Touch Bar slot)
// ---------------------------------------------------------------------------

const ID_LIGHT: &str = "com.wispr-fox.tb.light";
const ID_DRAFT: &str = "com.wispr-fox.tb.draft";
const ID_STOP: &str = "com.wispr-fox.tb.stop";
const ID_TIMER: &str = "com.wispr-fox.tb.timer";
const ID_STATUS: &str = "com.wispr-fox.tb.status";
const ID_FOX: &str = "com.wispr-fox.tb.skin.fox";
const ID_CAT: &str = "com.wispr-fox.tb.skin.cat";
const ID_CLIP: &str = "com.wispr-fox.tb.skin.clip";

// ---------------------------------------------------------------------------
// ObjC delegate class — bridges NSTouchBarDelegate to Rust
// ---------------------------------------------------------------------------

define_class!(
    // Subclass NSObject, main-thread-only (AppKit requirement).
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    struct WisprTouchBarDelegate;

    // Explicitly declare NSObjectProtocol conformance (required by NSTouchBarDelegate).
    unsafe impl NSObjectProtocol for WisprTouchBarDelegate {}

    // NSTouchBarDelegate protocol —
    // the only required method creates items on demand.
    unsafe impl NSTouchBarDelegate for WisprTouchBarDelegate {
        #[unsafe(method_id(touchBar:makeItemForIdentifier:))]
        fn touchBar_makeItemForIdentifier(
            &self,
            _touch_bar: &NSTouchBar,
            identifier: &NSString,
        ) -> Option<Retained<NSTouchBarItem>> {
            make_item(self, identifier)
        }
    }

    // Button-action selectors — one per Touch Bar button.
    // The button's `target` is set to the delegate instance and its
    // `action` to the corresponding selector.
    impl WisprTouchBarDelegate {
        #[unsafe(method(lightPressed:))]
        fn light_pressed(&self, _sender: &AnyObject) {
            trigger_mode(Mode::Light);
        }

        #[unsafe(method(draftPressed:))]
        fn draft_pressed(&self, _sender: &AnyObject) {
            trigger_mode(Mode::Drafting);
        }

        #[unsafe(method(stopPressed:))]
        fn stop_pressed(&self, _sender: &AnyObject) {
            trigger_stop();
        }

        #[unsafe(method(foxSkin:))]
        fn fox_skin(&self, _sender: &AnyObject) {
            switch_skin("fox");
        }

#[unsafe(method(catSkin:))]
        fn cat_skin(&self, _sender: &AnyObject) {
            switch_skin("cat");
        }

        #[unsafe(method(clipSkin:))]
        fn clip_skin(&self, _sender: &AnyObject) {
            switch_skin("stylized");
        }
    }
);

impl WisprTouchBarDelegate {
    /// Allocate and initialise a new delegate on the main thread.
    /// Uses ObjC `+new` (= alloc + init) — no custom init needed
    /// since the delegate has no instance variables.
    fn create(_mtm: MainThreadMarker) -> Retained<Self> {
        unsafe { msg_send![Self::class(), new] }
    }
}

// ---------------------------------------------------------------------------
// Item factory — called by the delegate for each identifier
// ---------------------------------------------------------------------------

/// Create the `NSTouchBarItem` for the given identifier string.
fn make_item(
    delegate: &WisprTouchBarDelegate,
    identifier: &NSString,
) -> Option<Retained<NSTouchBarItem>> {
    let id_str = identifier.to_string();

    match id_str.as_str() {
        // ── Idle: character skin buttons ──────────────────────────
        ID_FOX => Some(make_button(identifier, "🦊", c"foxSkin:", delegate, None)),
        ID_CAT => Some(make_button(identifier, "🐱", c"catSkin:", delegate, None)),
        ID_CLIP => Some(make_button(identifier, "📎", c"clipSkin:", delegate, None)),

        // ── Idle: mode buttons ───────────────────────────────────
        ID_LIGHT => Some(make_button(
            identifier,
            "💬 Light",
            c"lightPressed:",
            delegate,
            Some((0.2, 0.55, 0.95)), // blue
        )),
        ID_DRAFT => Some(make_button(
            identifier,
            "✏️ Draft",
            c"draftPressed:",
            delegate,
            Some((0.75, 0.35, 0.85)), // purple
        )),

        // ── Recording: timer ─────────────────────────────────────
        ID_TIMER => {
            let btn = make_ns_button("🔴 0:00", c"stopPressed:", delegate);
            TIMER_BTN.with(|cell| *cell.borrow_mut() = Some(btn.clone()));
            wrap_item(identifier, &btn)
                .map(|i| Retained::into_super(i))
        }

        // ── Recording: stop ──────────────────────────────────────
        ID_STOP => Some(make_button(
            identifier,
            "⏹ Stop",
            c"stopPressed:",
            delegate,
            Some((0.9, 0.25, 0.25)), // red
        )),

        // ── Processing: status label ─────────────────────────────
        ID_STATUS => {
            let btn = make_ns_button("⏳ Processing…", c"stopPressed:", delegate);
            unsafe {
                let _: () = msg_send![&*btn, setEnabled: false];
            }
            STATUS_BTN.with(|cell| *cell.borrow_mut() = Some(btn.clone()));
            wrap_item(identifier, &btn)
                .map(|i| Retained::into_super(i))
        }

        _ => {
            tracing::debug!("touchbar: unknown identifier '{id_str}'");
            None
        }
    }
}

/// Shortcut: create a button, wrap it in an NSCustomTouchBarItem,
/// upcast to NSTouchBarItem, and return.
fn make_button(
    identifier: &NSString,
    title: &str,
    action: &CStr,
    target: &WisprTouchBarDelegate,
    bezel_rgb: Option<(f64, f64, f64)>,
) -> Retained<NSTouchBarItem> {
    let btn = make_ns_button(title, action, target);
    if let Some((r, g, b)) = bezel_rgb {
        unsafe {
            let color: Retained<NSColor> = msg_send![
                NSColor::class(),
                colorWithRed: r,
                green: g,
                blue: b,
                alpha: 1.0_f64
            ];
            let _: () = msg_send![&*btn, setBezelColor: &*color];
        }
    }
    let item = wrap_item(identifier, &btn).expect("wrap_item should succeed");
    // NSCustomTouchBarItem → NSTouchBarItem (one level up)
    Retained::into_super(item)
}

/// Create a bare NSButton with title/target/action.
fn make_ns_button(
    title: &str,
    action: &CStr,
    target: &WisprTouchBarDelegate,
) -> Retained<NSButton> {
    let title_ns = NSString::from_str(title);
    let sel = Sel::register(action);
    unsafe {
        msg_send![
            NSButton::class(),
            buttonWithTitle: &*title_ns,
            target: target,
            action: sel
        ]
    }
}

/// Wrap an NSButton (or any NSView) in an NSCustomTouchBarItem.
fn wrap_item(
    identifier: &NSString,
    view: &NSButton,
) -> Option<Retained<NSCustomTouchBarItem>> {
    unsafe {
        let alloc: Allocated<NSCustomTouchBarItem> =
            msg_send![NSCustomTouchBarItem::class(), alloc];
        let item: Retained<NSCustomTouchBarItem> =
            msg_send![alloc, initWithIdentifier: identifier];
        let _: () = msg_send![&*item, setView: view];
        Some(item)
    }
}

// ---------------------------------------------------------------------------
// Action handlers — invoked from ObjC button selectors
// ---------------------------------------------------------------------------

/// Explicitly toggle recording in the given mode.
fn trigger_mode(mode: Mode) {
    let Some(bridge) = BRIDGE.get() else { return };
    bridge.flow.toggle_recording(&bridge.app, mode, false);
}

/// Stop the current recording without fabricating a physical shortcut edge.
fn trigger_stop() {
    let Some(bridge) = BRIDGE.get() else { return };
    bridge.flow.stop_recording(&bridge.app);
}

/// Emit a skin-change event.  Both the main window sidebar and the
/// Clippy floater listen for this and swap skins.
fn switch_skin(skin: &str) {
    let Some(bridge) = BRIDGE.get() else { return };
    let _ = bridge.app.emit("wispr:skin-change", skin.to_string());
}

// ---------------------------------------------------------------------------
// Touch Bar state management — swap items for each flow phase
// ---------------------------------------------------------------------------

/// Show idle layout: character skins + mode buttons.
fn set_idle_items() {
    RECORDING.store(false, Ordering::Relaxed);
    REC_START.with(|c| *c.borrow_mut() = None);

    TB.with(|cell| {
        let tb = cell.borrow();
        let Some(tb) = tb.as_ref() else { return };
        let ids = NSArray::from_retained_slice(&[
            NSString::from_str(ID_FOX),
            NSString::from_str(ID_CAT),
            NSString::from_str(ID_CLIP),
            NSString::from_str(ID_LIGHT),
            NSString::from_str(ID_DRAFT),
        ]);
        unsafe {
            let _: () = msg_send![&**tb, setDefaultItemIdentifiers: &*ids];
        }
    });
}

/// Show recording layout: timer + stop button.
fn set_recording_items() {
    RECORDING.store(true, Ordering::Relaxed);
    REC_START.with(|c| *c.borrow_mut() = Some(Instant::now()));

    TB.with(|cell| {
        let tb = cell.borrow();
        let Some(tb) = tb.as_ref() else { return };
        let ids = NSArray::from_retained_slice(&[
            NSString::from_str(ID_TIMER),
            NSString::from_str(ID_STOP),
        ]);
        unsafe {
            let _: () = msg_send![&**tb, setDefaultItemIdentifiers: &*ids];
        }
    });
}

/// Show processing layout: a single disabled status label.
fn set_processing_items(stage: &str) {
    RECORDING.store(false, Ordering::Relaxed);
    REC_START.with(|c| *c.borrow_mut() = None);

    // Update the label text first (item may already exist from a prior call)
    STATUS_BTN.with(|cell| {
        if let Some(btn) = cell.borrow().as_ref() {
            let title = NSString::from_str(&format!("⏳ {stage}"));
            unsafe {
                let _: () = msg_send![&**btn, setTitle: &*title];
            }
        }
    });

    TB.with(|cell| {
        let tb = cell.borrow();
        let Some(tb) = tb.as_ref() else { return };
        let ids = NSArray::from_retained_slice(&[NSString::from_str(ID_STATUS)]);
        unsafe {
            let _: () = msg_send![&**tb, setDefaultItemIdentifiers: &*ids];
        }
    });
}

/// Update the timer label to show elapsed recording time.
fn tick_timer() {
    let elapsed = REC_START.with(|c| c.borrow().map(|s| s.elapsed()));
    let Some(elapsed) = elapsed else { return };
    let secs = elapsed.as_secs();
    let text = format!("🔴 {}:{:02}", secs / 60, secs % 60);
    TIMER_BTN.with(|cell| {
        if let Some(btn) = cell.borrow().as_ref() {
            let title = NSString::from_str(&text);
            unsafe {
                let _: () = msg_send![&**btn, setTitle: &*title];
            }
        }
    });
}

// ---------------------------------------------------------------------------
// Public API — called from lib.rs setup (main thread)
// ---------------------------------------------------------------------------

/// Install the Touch Bar.
///
/// Must be called from the Tauri `setup` closure (which runs on the main
/// thread).  Harmless no-op on Macs without Touch Bar hardware.
pub fn install(app: &AppHandle, flow: &Flow) {
    let Some(mtm) = MainThreadMarker::new() else {
        tracing::warn!("touchbar::install called off main thread — skipping");
        return;
    };

    // ── Store bridge for button actions ──────────────────────────
    let _ = BRIDGE.set(Bridge {
        app: app.clone(),
        flow: flow.clone(),
    });

    // ── Create delegate ──────────────────────────────────────────
    let delegate = WisprTouchBarDelegate::create(mtm);

    // ── Create NSTouchBar ────────────────────────────────────────
    let touch_bar = NSTouchBar::new(mtm);
    let cust_id = NSString::from_str("com.wispr-fox.touchbar");
    unsafe {
        let _: () = msg_send![&*touch_bar, setCustomizationIdentifier: &*cust_id];
    }

    // Wire delegate (weak ref — we keep the delegate alive in DELEGATE)
    let proto = ProtocolObject::from_ref(&*delegate);
    touch_bar.setDelegate(Some(proto));

    // Set initial idle items
    let ids = NSArray::from_retained_slice(&[
        NSString::from_str(ID_FOX),
        NSString::from_str(ID_CAT),
        NSString::from_str(ID_CLIP),
        NSString::from_str(ID_LIGHT),
        NSString::from_str(ID_DRAFT),
    ]);
    unsafe {
        let _: () = msg_send![&*touch_bar, setDefaultItemIdentifiers: &*ids];
    }

    // ── Attach to NSApplication (app-level fallback touch bar) ──
    unsafe {
        let ns_app: *mut AnyObject =
            msg_send![NSApplication::class(), sharedApplication];
        let _: () = msg_send![ns_app, setTouchBar: &*touch_bar];
    }

    // ── Keep delegate + touch bar alive ──────────────────────────
    DELEGATE.with(|c| *c.borrow_mut() = Some(delegate));
    TB.with(|c| *c.borrow_mut() = Some(touch_bar));

    // ── Listen for flow-state events ─────────────────────────────
    let app_listen = app.clone();
    app.listen("wispr:state", move |event| {
        let payload = event.payload();
        // Payload is a JSON string like "\"recording\"", strip quotes.
        let state = payload.trim_matches('"');
        let state_owned = state.to_string();
        let app2 = app_listen.clone();

        // Dispatch UI update to the main thread.
        let _ = app2.run_on_main_thread(move || {
            match state_owned.as_str() {
                "idle" => set_idle_items(),
                "recording" => {
                    set_recording_items();
                    // Spawn a timer-update loop that ticks every second.
                    if let Some(bridge) = BRIDGE.get() {
                        let app3 = bridge.app.clone();
                        tauri::async_runtime::spawn(async move {
                            let mut tick =
                                tokio::time::interval(std::time::Duration::from_secs(1));
                            loop {
                                tick.tick().await;
                                if !RECORDING.load(Ordering::Relaxed) {
                                    break;
                                }
                                let app4 = app3.clone();
                                let _ = app4.run_on_main_thread(|| tick_timer());
                            }
                        });
                    }
                }
                "denoising" => set_processing_items("Reducing noise…"),
                "transcribing" => set_processing_items("Transcribing · Groq"),
                "cleaning" => set_processing_items("Polishing · Groq"),
                "injecting" | "pasting" => set_processing_items("Pasting…"),
                other => {
                    tracing::debug!("touchbar: unhandled state '{other}'");
                }
            }
        });
    });

    tracing::info!("Touch Bar installed (idle layout with 4 skins + 2 modes)");
}
