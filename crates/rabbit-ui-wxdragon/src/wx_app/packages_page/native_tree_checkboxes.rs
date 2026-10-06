//! Windows-only helpers that turn the wx-created `wxTreeCtrl` into a
//! `SysTreeView32` with `TVS_CHECKBOXES` set. wxdragon doesn't expose any of
//! the native APIs we need (no `EnableCheckBoxes`, no `SetItemState`, no
//! `SetStateImageList`), so we reach down to the underlying `HWND` via
//! `Window::get_handle()` and drive the control directly through user32 +
//! raw `SendMessageW` traffic. `wxTreeItemId` on wxMSW is a single-member
//! struct (`void* m_pItem`, no vtable / no padding), so reading the first
//! pointer-sized word of the wxd_TreeItemId_t* gives us the native
//! `HTREEITEM`. This is implementation-dependent but stable on wxMSW today
//! — we live with that fragility because the alternative is forking
//! wxdragon-sys, and the prize is screen-reader-correct native checkboxes
//! (UIA Toggle pattern on each tree row).

use std::ffi::c_void;
use wxdragon::widgets::treectrl::TreeItemId;

pub const GWL_STYLE: i32 = -16;
pub const TVS_CHECKBOXES: u32 = 0x0100;
const TV_FIRST: u32 = 0x1100;
const TVM_SETITEMW: u32 = TV_FIRST + 63;
const TVM_GETITEMW: u32 = TV_FIRST + 62;
const TVM_SETIMAGELIST: u32 = TV_FIRST + 9;
const TVM_HITTEST: u32 = TV_FIRST + 17;
const TVSIL_STATE: usize = 2;
const TVIF_HANDLE: u32 = 0x0010;
const TVIF_STATE: u32 = 0x0008;
const TVIS_STATEIMAGEMASK: u32 = 0xF000;
pub const TVHT_ONITEMSTATEICON: u32 = 0x0040;

/// Themed checkbox state ids. See `BP_CHECKBOX` (= 3) of `BUTTON`
/// theme class in `<vsstyle.h>`. We use the "normal" variants because
/// the tree control overlays its own selection/hover effects on top.
const BP_CHECKBOX: i32 = 3;
const CBS_UNCHECKEDNORMAL: i32 = 1;
const CBS_CHECKEDNORMAL: i32 = 5;
const CBS_MIXEDNORMAL: i32 = 9;
const TS_TRUE: i32 = 1;

/// `DrawFrameControl` flags used as the unthemed fallback when
/// `OpenThemeData("BUTTON")` returns null (classic theme / no themes).
const DFC_BUTTON: u32 = 4;
const DFCS_BUTTONCHECK: u32 = 0x0000;
const DFCS_CHECKED: u32 = 0x0400;
const DFCS_BUTTON3STATE: u32 = 0x0008;

/// `ImageList_Create` flags: 32-bit color + mask channel.
const ILC_COLOR32: u32 = 0x0020;
const ILC_MASK: u32 = 0x0001;

/// State-image indices we use. `TVS_CHECKBOXES` indices are 1-based
/// (index 0 means "no state image"). `Mixed` is what the parent
/// "Packages" group shows when only some children are checked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TriState {
    Unchecked,
    Checked,
    Mixed,
}

impl TriState {
    fn state_image_index(self) -> u32 {
        match self {
            TriState::Unchecked => 1,
            TriState::Checked => 2,
            TriState::Mixed => 3,
        }
    }
}

/// Layout-compatible mirror of `TVITEMW` from `<commctrl.h>`.
#[repr(C)]
struct Tvitemw {
    mask: u32,
    h_item: *mut c_void,
    state: u32,
    state_mask: u32,
    text: *mut u16,
    text_max: i32,
    image: i32,
    selected_image: i32,
    children: i32,
    l_param: isize,
}

#[repr(C)]
struct RectStruct {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[repr(C)]
struct SizeStruct {
    cx: i32,
    cy: i32,
}

unsafe extern "system" {
    fn GetWindowLongPtrW(h_wnd: *mut c_void, n_index: i32) -> isize;
    fn SetWindowLongPtrW(h_wnd: *mut c_void, n_index: i32, dw_new_long: isize) -> isize;
    fn SendMessageW(h_wnd: *mut c_void, msg: u32, w_param: usize, l_param: isize) -> isize;
    fn GetDC(h_wnd: *mut c_void) -> *mut c_void;
    fn ReleaseDC(h_wnd: *mut c_void, hdc: *mut c_void) -> i32;
    fn DrawFrameControl(hdc: *mut c_void, lprc: *const RectStruct, type_: u32, state: u32) -> i32;
    fn FillRect(hdc: *mut c_void, lprc: *const RectStruct, hbr: *mut c_void) -> i32;
}

#[link(name = "gdi32")]
unsafe extern "system" {
    fn CreateCompatibleDC(hdc: *mut c_void) -> *mut c_void;
    fn DeleteDC(hdc: *mut c_void) -> i32;
    fn CreateCompatibleBitmap(hdc: *mut c_void, cx: i32, cy: i32) -> *mut c_void;
    fn DeleteObject(obj: *mut c_void) -> i32;
    fn SelectObject(hdc: *mut c_void, obj: *mut c_void) -> *mut c_void;
    fn CreateSolidBrush(color: u32) -> *mut c_void;
}

#[link(name = "uxtheme")]
unsafe extern "system" {
    fn OpenThemeData(h_wnd: *mut c_void, classlist: *const u16) -> *mut c_void;
    fn CloseThemeData(htheme: *mut c_void) -> i32;
    fn DrawThemeBackground(
        htheme: *mut c_void,
        hdc: *mut c_void,
        partid: i32,
        stateid: i32,
        prect: *const RectStruct,
        pcliprect: *const RectStruct,
    ) -> i32;
    fn GetThemePartSize(
        htheme: *mut c_void,
        hdc: *mut c_void,
        partid: i32,
        stateid: i32,
        prc: *const RectStruct,
        esize: i32,
        psz: *mut SizeStruct,
    ) -> i32;
}

#[link(name = "comctl32")]
unsafe extern "system" {
    fn ImageList_Create(cx: i32, cy: i32, flags: u32, initial: i32, grow: i32) -> *mut c_void;
    fn ImageList_AddMasked(himl: *mut c_void, hbm_image: *mut c_void, cr_mask: u32) -> i32;
    fn ImageList_Destroy(himl: *mut c_void) -> i32;
}

/// OR-in the `TVS_CHECKBOXES` style on an existing tree's `HWND` AND
/// install our own 3-image state list so the synthetic Packages group
/// can show an indeterminate ("half-checked") state when only some of
/// its children are checked. The native control by default creates a
/// 2-image list (unchecked, checked); we replace it with a 3-image
/// list (unchecked, checked, mixed). The first two come from the
/// `BUTTON` theme via `DrawThemeBackground` so they match the rest of
/// the OS's checkboxes; the third uses `CBS_MIXEDNORMAL` to draw the
/// system's standard mixed-state checkbox glyph.
pub fn enable_checkboxes(hwnd: *mut c_void) {
    if hwnd.is_null() {
        return;
    }
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
        if (style as u32 & TVS_CHECKBOXES) == 0 {
            SetWindowLongPtrW(hwnd, GWL_STYLE, style | TVS_CHECKBOXES as isize);
        }
    }
    install_tristate_state_image_list(hwnd);
}

/// Build a 3-image `HIMAGELIST` containing themed unchecked + checked +
/// mixed checkbox glyphs and install it as the tree's state image
/// list. Replaces (and frees) whatever list `TVS_CHECKBOXES` may have
/// auto-created.
fn install_tristate_state_image_list(hwnd: *mut c_void) {
    // BGR magenta serves as the transparency key for the image list:
    // anywhere we leave magenta is treated as transparent on draw.
    const MAGENTA_BGR: u32 = 0x00FF00FF;

    let class: Vec<u16> = "BUTTON".encode_utf16().chain(std::iter::once(0)).collect();
    let htheme = unsafe { OpenThemeData(hwnd, class.as_ptr()) };

    // Determine checkbox glyph size from the theme (DPI-aware) when
    // available; fall back to a sensible default otherwise.
    let mut size = SizeStruct { cx: 13, cy: 13 };
    if !htheme.is_null() {
        unsafe {
            let _ = GetThemePartSize(
                htheme,
                std::ptr::null_mut(),
                BP_CHECKBOX,
                CBS_UNCHECKEDNORMAL,
                std::ptr::null(),
                TS_TRUE,
                &mut size,
            );
        }
    }
    let cx = size.cx.max(13);
    let cy = size.cy.max(13);

    let himl = unsafe { ImageList_Create(cx, cy, ILC_COLOR32 | ILC_MASK, 3, 0) };
    if himl.is_null() {
        if !htheme.is_null() {
            unsafe {
                CloseThemeData(htheme);
            }
        }
        return;
    }

    let states = [CBS_UNCHECKEDNORMAL, CBS_CHECKEDNORMAL, CBS_MIXEDNORMAL];

    unsafe {
        let hdc_screen = GetDC(std::ptr::null_mut());
        let hdc_mem = CreateCompatibleDC(hdc_screen);
        let key_brush = CreateSolidBrush(MAGENTA_BGR);

        for state in &states {
            let hbm = CreateCompatibleBitmap(hdc_screen, cx, cy);
            let prev_bm = SelectObject(hdc_mem, hbm);

            // Fill with magenta — anything DrawThemeBackground (or the
            // unthemed fallback) leaves untouched stays magenta and
            // becomes transparent in the image list.
            let rc = RectStruct {
                left: 0,
                top: 0,
                right: cx,
                bottom: cy,
            };
            let _ = FillRect(hdc_mem, &rc, key_brush);

            if !htheme.is_null() {
                let _ = DrawThemeBackground(
                    htheme,
                    hdc_mem,
                    BP_CHECKBOX,
                    *state,
                    &rc,
                    std::ptr::null(),
                );
            } else {
                let dfcs = match *state {
                    CBS_UNCHECKEDNORMAL => DFCS_BUTTONCHECK,
                    CBS_CHECKEDNORMAL => DFCS_BUTTONCHECK | DFCS_CHECKED,
                    CBS_MIXEDNORMAL => DFCS_BUTTON3STATE | DFCS_CHECKED,
                    _ => DFCS_BUTTONCHECK,
                };
                let _ = DrawFrameControl(hdc_mem, &rc, DFC_BUTTON, dfcs);
            }

            SelectObject(hdc_mem, prev_bm);
            let _ = ImageList_AddMasked(himl, hbm, MAGENTA_BGR);
            DeleteObject(hbm);
        }

        DeleteObject(key_brush);
        DeleteDC(hdc_mem);
        ReleaseDC(std::ptr::null_mut(), hdc_screen);

        if !htheme.is_null() {
            CloseThemeData(htheme);
        }

        // Hand the new image list to the tree control. The control
        // takes ownership of the new list and returns the old one for
        // us to destroy.
        let old_himl = SendMessageW(hwnd, TVM_SETIMAGELIST, TVSIL_STATE, himl as isize);
        if old_himl != 0 {
            ImageList_Destroy(old_himl as *mut c_void);
        }
    }
}

#[repr(C)]
struct TvHitTestPoint {
    x: i32,
    y: i32,
}

/// Layout-compatible mirror of `TVHITTESTINFO` from `<commctrl.h>`.
#[repr(C)]
struct TvHitTestInfo {
    pt: TvHitTestPoint,
    flags: u32,
    h_item: *mut c_void,
}

/// Send `TVM_HITTEST` to the native tree control and return the hit
/// item handle plus the result flags. `(x, y)` is in the tree's
/// client-area coordinates (which is what `wxEVT_LEFT_*` mouse-event
/// positions report on the bound window).
pub fn hit_test(hwnd: *mut c_void, x: i32, y: i32) -> (u32, *mut c_void) {
    if hwnd.is_null() {
        return (0, std::ptr::null_mut());
    }
    let mut info = TvHitTestInfo {
        pt: TvHitTestPoint { x, y },
        flags: 0,
        h_item: std::ptr::null_mut(),
    };
    unsafe {
        SendMessageW(hwnd, TVM_HITTEST, 0, &mut info as *mut _ as isize);
    }
    (info.flags, info.h_item)
}

/// Read the native `HTREEITEM` out of a wxdragon `TreeItemId`. Relies
/// on:
/// 1. `TreeItemId { ptr: *mut wxd_TreeItemId_t }` being a single-field
///    `repr(Rust)` struct — its layout matches the inner pointer.
/// 2. `wxd_TreeItemId_t*` being a `reinterpret_cast` of `wxTreeItemId*`
///    (confirmed in wxdragon-sys/cpp/src/treectrl.cpp).
/// 3. `wxTreeItemId` having `void* m_pItem` as its only non-static
///    member with no vtable.
fn htreeitem_from(item: &TreeItemId) -> *mut c_void {
    // SAFETY: see the contract above. Reading the first pointer-sized
    // word of the `TreeItemId` wrapper yields its private `ptr` field;
    // reading the first word of that yields `wxTreeItemId::m_pItem`.
    let inner: *mut c_void = unsafe { std::mem::transmute_copy(item) };
    if inner.is_null() {
        return std::ptr::null_mut();
    }
    unsafe { *(inner as *const *mut c_void) }
}

pub fn set_check_state(hwnd: *mut c_void, item: &TreeItemId, checked: bool) {
    let state = if checked {
        TriState::Checked
    } else {
        TriState::Unchecked
    };
    set_check_state_tri(hwnd, item, state);
}

/// Set the state image index (1, 2, or 3 — see `TriState`) for the
/// given tree item. Used both for leaf rows (only `Checked` /
/// `Unchecked`) and the synthetic Packages group (any of the three).
pub fn set_check_state_tri(hwnd: *mut c_void, item: &TreeItemId, state: TriState) {
    if hwnd.is_null() {
        return;
    }
    let h_item = htreeitem_from(item);
    if h_item.is_null() {
        return;
    }
    let state_value = state.state_image_index() << 12;
    let mut tvi = Tvitemw {
        mask: TVIF_STATE | TVIF_HANDLE,
        h_item,
        state: state_value,
        state_mask: TVIS_STATEIMAGEMASK,
        text: std::ptr::null_mut(),
        text_max: 0,
        image: 0,
        selected_image: 0,
        children: 0,
        l_param: 0,
    };
    unsafe {
        SendMessageW(hwnd, TVM_SETITEMW, 0, &mut tvi as *mut _ as isize);
    }
}

pub fn get_check_state(hwnd: *mut c_void, item: &TreeItemId) -> bool {
    if hwnd.is_null() {
        return false;
    }
    let h_item = htreeitem_from(item);
    if h_item.is_null() {
        return false;
    }
    let mut tvi = Tvitemw {
        mask: TVIF_STATE | TVIF_HANDLE,
        h_item,
        state: 0,
        state_mask: TVIS_STATEIMAGEMASK,
        text: std::ptr::null_mut(),
        text_max: 0,
        image: 0,
        selected_image: 0,
        children: 0,
        l_param: 0,
    };
    unsafe {
        SendMessageW(hwnd, TVM_GETITEMW, 0, &mut tvi as *mut _ as isize);
    }
    // State image index 2 = checked.
    ((tvi.state & TVIS_STATEIMAGEMASK) >> 12) == 2
}
