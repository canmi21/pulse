// src/login_item.rs

use auto_launch::AutoLaunch;
use std::env;

// This will be the name displayed in System Settings > General > Login Items
const APP_NAME: &str = "Pulse";

/// Creates an AutoLaunch instance.
/// This will only succeed if the app is running from a proper .app bundle.
fn get_auto_launch() -> Option<AutoLaunch> {
    let app_path_exe = env::current_exe().ok()?;
    let app_bundle_path = app_path_exe
        .ancestors()
        .find(|p| p.extension().map_or(false, |ext| ext == "app"))?;

    let app_bundle_path_str = app_bundle_path.to_str()?;
    let app_name_with_ext = format!("{}.app", APP_NAME);

    if !app_bundle_path.ends_with(&app_name_with_ext) {
        return None;
    }

    Some(AutoLaunch::new(APP_NAME, app_bundle_path_str, true, &[] as &[&str; 0]))
}

pub fn is_enabled() -> bool {
    if let Some(auto) = get_auto_launch() {
        return auto.is_enabled().unwrap_or(false);
    }
    false
}

pub fn set_enabled(enabled: bool) {
    if let Some(auto) = get_auto_launch() {
        let result = if enabled {
            auto.enable()
        } else {
            auto.disable()
        };
        
        if result.is_err() {
            eprintln!("Failed to update auto-launch setting. Make sure the app is in the /Applications folder.");
        }
    } else {
        println!("Cannot set auto-launch: Not running from a valid .app bundle.");
    }
}
