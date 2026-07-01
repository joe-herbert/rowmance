/// Repositions the macOS traffic light buttons to align with the floating titlebar card.
/// On non-macOS platforms this is a no-op.
#[tauri::command]
pub async fn window_set_traffic_light_position(
    webview_window: tauri::WebviewWindow,
    x: f64,
    y: f64,
) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    webview_window
        .with_webview(move |webview| {
            let ns_window_ptr = webview.ns_window();
            unsafe { inset_traffic_lights(ns_window_ptr, x, y) };
        })
        .map_err(|e| e.to_string())?;

    let _ = (x, y); // suppress unused warnings on non-macOS
    Ok(())
}

/// Installs a `setFrame:` swizzle on the private `_NSTitlebarContainerView` class.
///
/// macOS calls `setFrame:` on this view during every window resize, resetting the
/// traffic-light position back to default. By intercepting that call we can apply
/// the correct height and y-offset synchronously — before any frame is drawn —
/// which eliminates all visible flicker.
///
/// Must be called once at app startup (before the first resize).
/// On non-macOS platforms this is a no-op.
#[cfg(target_os = "macos")]
pub fn install_titlebar_swizzle() {
    use objc2::runtime::{AnyObject, Imp, Sel};
    use objc2_foundation::NSRect;
    use std::ffi::CStr;
    use std::sync::OnceLock;

    type SetFrameFn = unsafe extern "C-unwind" fn(*mut AnyObject, Sel, NSRect);
    static ORIGINAL: OnceLock<SetFrameFn> = OnceLock::new();

    unsafe extern "C-unwind" fn swizzled_set_frame(
        this: *mut AnyObject,
        sel: Sel,
        mut rect: NSRect,
    ) {
        // Read params without holding the lock across ObjC calls.
        let container_height = TL_PARAMS.lock().unwrap().map(|p| p.container_height);

        if let Some(ch) = container_height {
            use objc2_app_kit::NSView;
            let view: &NSView = &*(this as *const NSView);
            if let Some(window) = view.window() {
                let wf = window.frame();
                rect.size.height = ch;
                rect.origin.y = wf.size.height - ch;
            }
        }

        // Call the original setFrame: — this lays out subviews at standard positions.
        if let Some(original) = ORIGINAL.get() {
            original(this, sel, rect);
        }

        // Re-apply custom button x-positions that the standard layout just reset.
        let params = *TL_PARAMS.lock().unwrap();
        if let Some(p) = params {
            use objc2_app_kit::NSView;
            let view: &NSView = &*(this as *const NSView);
            if let Some(window) = view.window() {
                reposition_buttons(&window, p.button_x, p.button_spacing);
            }
        }
    }

    unsafe {
        let cls_name = CStr::from_bytes_with_nul(b"_NSTitlebarContainerView\0").unwrap();
        let Some(cls) = objc2::runtime::AnyClass::get(cls_name) else {
            return;
        };

        let sel = objc2::sel!(setFrame:);
        let Some(method) = cls.instance_method(sel) else {
            return;
        };

        let new_imp: Imp = std::mem::transmute(swizzled_set_frame as SetFrameFn);
        let old_imp: Imp = method.set_implementation(new_imp);
        let original: SetFrameFn = std::mem::transmute(old_imp);
        let _ = ORIGINAL.set(original);
    }
}

// ── macOS-only internals ──────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
#[derive(Copy, Clone)]
struct TlParams {
    container_height: f64,
    button_x: f64,
    button_spacing: f64,
}

#[cfg(target_os = "macos")]
static TL_PARAMS: std::sync::Mutex<Option<TlParams>> = std::sync::Mutex::new(None);

#[cfg(target_os = "macos")]
unsafe fn reposition_buttons(window: &objc2_app_kit::NSWindow, button_x: f64, button_spacing: f64) {
    use objc2_app_kit::NSWindowButton;
    use objc2_foundation::NSPoint;

    let buttons = [
        window.standardWindowButton(NSWindowButton::CloseButton),
        window.standardWindowButton(NSWindowButton::MiniaturizeButton),
        window.standardWindowButton(NSWindowButton::ZoomButton),
    ];

    for (i, btn) in buttons.into_iter().flatten().enumerate() {
        btn.setFrameOrigin(NSPoint::new(
            button_x + i as f64 * button_spacing,
            btn.frame().origin.y,
        ));
    }
}

#[cfg(target_os = "macos")]
unsafe fn inset_traffic_lights(ns_window_ptr: *mut std::ffi::c_void, x: f64, y: f64) {
    use objc2::rc::Retained;
    use objc2_app_kit::{NSButton, NSWindow, NSWindowButton};
    use objc2_foundation::NSPoint;

    let window: &NSWindow = &*(ns_window_ptr as *const NSWindow);

    let Some(close) = window.standardWindowButton(NSWindowButton::CloseButton) else {
        return;
    };
    let Some(miniaturize) = window.standardWindowButton(NSWindowButton::MiniaturizeButton) else {
        return;
    };
    let zoom = window.standardWindowButton(NSWindowButton::ZoomButton);

    let title_bar_container = close.superview().and_then(|v| v.superview());
    let Some(title_bar_container) = title_bar_container else {
        return;
    };

    let close_rect = close.frame();
    let container_height = close_rect.size.height + y;
    let space_between = miniaturize.frame().origin.x - close_rect.origin.x;

    // Persist so the swizzle can re-apply on every resize.
    *TL_PARAMS.lock().unwrap() = Some(TlParams {
        container_height,
        button_x: x,
        button_spacing: space_between,
    });

    let mut title_bar_rect = title_bar_container.frame();
    title_bar_rect.size.height = container_height;
    title_bar_rect.origin.y = window.frame().size.height - container_height;
    title_bar_container.setFrame(title_bar_rect);

    let mut buttons: Vec<Retained<NSButton>> = vec![close, miniaturize];
    if let Some(z) = zoom {
        buttons.push(z);
    }

    for (i, button) in buttons.into_iter().enumerate() {
        button.setFrameOrigin(NSPoint::new(
            x + i as f64 * space_between,
            button.frame().origin.y,
        ));
    }
}
