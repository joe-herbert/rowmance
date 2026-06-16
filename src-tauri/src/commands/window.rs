/// Repositions the macOS traffic light buttons to align with the floating titlebar card.
/// This must be called whenever --panel-spacing changes (i.e. on theme switch).
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
    let title_bar_frame_height = close_rect.size.height + y;
    let mut title_bar_rect = title_bar_container.frame();
    title_bar_rect.size.height = title_bar_frame_height;
    title_bar_rect.origin.y = window.frame().size.height - title_bar_frame_height;
    title_bar_container.setFrame(title_bar_rect);

    let space_between = miniaturize.frame().origin.x - close_rect.origin.x;

    let mut buttons: Vec<Retained<NSButton>> = vec![close, miniaturize];
    if let Some(z) = zoom {
        buttons.push(z);
    }

    for (i, button) in buttons.into_iter().enumerate() {
        button.setFrameOrigin(NSPoint::new(x + i as f64 * space_between, button.frame().origin.y));
    }
}
