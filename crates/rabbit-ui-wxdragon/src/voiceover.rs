//! Ask VoiceOver to speak a short message.
//!
//! A control changed from code rather than by the user tells VoiceOver
//! nothing, so it stays silent where a click would have been read back. The
//! macOS packages list is the case in point: Space ticks a row through the
//! model, and the checkbox VoiceOver would announce is rebuilt behind it.
//!
//! This posts `NSAccessibilityAnnouncementRequestedNotification`, the AppKit
//! call for exactly that. The dictionary is built with CoreFoundation, whose
//! strings, numbers and dictionaries are toll-free bridged to their
//! Foundation counterparts, so no Objective-C message sending is needed.

use std::ffi::{CString, c_char, c_void};

type CfRef = *const c_void;

/// `CFDictionaryKeyCallBacks` / `CFDictionaryValueCallBacks`: only ever
/// passed by address, so their layout doesn't matter here.
#[repr(C)]
struct CfCallBacks {
    _opaque: [u8; 0],
}

const CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;
const CF_NUMBER_CF_INDEX_TYPE: isize = 14;
/// `NSAccessibilityPriorityHigh`: interrupts whatever VoiceOver is saying,
/// which is what a direct answer to a key press should do.
const NS_ACCESSIBILITY_PRIORITY_HIGH: isize = 90;

// SAFETY: AppKit and CoreFoundation are system frameworks that wxWidgets
// already links; these declarations match their public C headers.
#[link(name = "AppKit", kind = "framework")]
unsafe extern "C" {
    static NSApp: CfRef;
    static NSAccessibilityAnnouncementRequestedNotification: CfRef;
    static NSAccessibilityAnnouncementKey: CfRef;
    static NSAccessibilityPriorityKey: CfRef;
    fn NSAccessibilityPostNotificationWithUserInfo(
        element: CfRef,
        notification: CfRef,
        user_info: CfRef,
    );
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    static kCFTypeDictionaryKeyCallBacks: CfCallBacks;
    static kCFTypeDictionaryValueCallBacks: CfCallBacks;
    fn CFStringCreateWithCString(alloc: CfRef, text: *const c_char, encoding: u32) -> CfRef;
    fn CFNumberCreate(alloc: CfRef, number_type: isize, value: *const c_void) -> CfRef;
    fn CFDictionaryCreate(
        alloc: CfRef,
        keys: *const CfRef,
        values: *const CfRef,
        count: isize,
        key_callbacks: *const CfCallBacks,
        value_callbacks: *const CfCallBacks,
    ) -> CfRef;
    fn CFRelease(object: CfRef);
}

/// Have VoiceOver speak `text` now. Does nothing when VoiceOver is off:
/// macOS drops the notification.
pub(crate) fn announce(text: &str) {
    let Ok(text) = CString::new(text) else {
        return;
    };
    // SAFETY: every object created here is released before returning, and
    // the dictionary retains what it holds. NSApp is set by wxWidgets before
    // any window exists, so it is valid wherever a key event can arrive.
    unsafe {
        if NSApp.is_null() {
            return;
        }
        let message =
            CFStringCreateWithCString(std::ptr::null(), text.as_ptr(), CF_STRING_ENCODING_UTF8);
        let priority = CFNumberCreate(
            std::ptr::null(),
            CF_NUMBER_CF_INDEX_TYPE,
            (&NS_ACCESSIBILITY_PRIORITY_HIGH as *const isize).cast(),
        );
        if message.is_null() || priority.is_null() {
            for object in [message, priority] {
                if !object.is_null() {
                    CFRelease(object);
                }
            }
            return;
        }
        let keys = [NSAccessibilityAnnouncementKey, NSAccessibilityPriorityKey];
        let values = [message, priority];
        let user_info = CFDictionaryCreate(
            std::ptr::null(),
            keys.as_ptr(),
            values.as_ptr(),
            keys.len() as isize,
            &kCFTypeDictionaryKeyCallBacks,
            &kCFTypeDictionaryValueCallBacks,
        );
        if !user_info.is_null() {
            NSAccessibilityPostNotificationWithUserInfo(
                NSApp,
                NSAccessibilityAnnouncementRequestedNotification,
                user_info,
            );
            CFRelease(user_info);
        }
        CFRelease(message);
        CFRelease(priority);
    }
}
