use anyhow::Result;
use tauri::{
    image::Image,
    menu::{MenuBuilder, MenuItem, MenuItemBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};

pub fn install_tray(app: &AppHandle) -> Result<MenuItem<tauri::Wry>> {
    let open_item = MenuItemBuilder::with_id("open", "Show Settings").build(app)?;
    let status_item = MenuItemBuilder::with_id("status", "Status: Idle")
        .enabled(false)
        .build(app)?;
    let quit_item = MenuItemBuilder::with_id("quit", "Quit").build(app)?;

    let menu = MenuBuilder::new(app)
        .item(&open_item)
        .separator()
        .item(&status_item)
        .separator()
        .item(&quit_item)
        .build()?;

    let handle = app.clone();

    // Load tray icon from embedded resources
    let icon_bytes = include_bytes!("../../icons/32x32.png");

    let mut tray_builder = TrayIconBuilder::with_id("main")
        .menu(&menu)
        .tooltip("Easy Dictate");

    // Create icon from PNG bytes
    if let Some(icon_image) = Image::from_bytes(icon_bytes).ok() {
        tray_builder = tray_builder.icon(icon_image);
    }

    tray_builder.on_tray_icon_event(move |_tray, event| match event {
            TrayIconEvent::Click {
                button,
                button_state,
                ..
            } => {
                if button == MouseButton::Left && button_state == MouseButtonState::Up {
                    show_settings_window(&handle);
                }
            }
            TrayIconEvent::DoubleClick { .. } => {
                show_settings_window(&handle);
            }
            _ => {}
        })
        .build(app)?;

    Ok(status_item)
}

pub fn show_settings_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}
