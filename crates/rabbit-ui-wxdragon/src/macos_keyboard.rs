//! Whether the physical Control key is held, on macOS.
//!
//! wxWidgets on macOS reports Command as "Control", and the pinned wxDragon
//! doesn't wrap the raw modifier state, so a key handler can't tell Ctrl+Tab
//! from plain Tab. CoreGraphics can: `CGEventSourceFlagsState` returns the
//! modifier keys held right now. It is a plain C call, like the ones
//! `voiceover.rs` makes, so no Objective-C messaging is needed.

/// `kCGEventSourceStateCombinedSessionState`: every event source in the
/// login session together, so Control held on any keyboard counts.
const COMBINED_SESSION_STATE: i32 = 0;
/// `kCGEventFlagMaskControl`, which is `NX_CONTROLMASK`.
const FLAG_MASK_CONTROL: u64 = 0x0004_0000;

// SAFETY: CoreGraphics is a system framework, part of what wxWidgets already
// links on macOS; this declaration matches its public C header,
// `CGEventFlags CGEventSourceFlagsState(CGEventSourceStateID stateID)`, where
// `CGEventFlags` is a `uint64_t` and `CGEventSourceStateID` an `int32_t`.
#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGEventSourceFlagsState(state_id: i32) -> u64;
}

/// Whether the physical Control key is down right now.
pub(crate) fn control_key_down() -> bool {
    // SAFETY: a read-only query that takes no pointers; the state id is one
    // of the documented constants.
    let flags = unsafe { CGEventSourceFlagsState(COMBINED_SESSION_STATE) };
    flags & FLAG_MASK_CONTROL != 0
}
