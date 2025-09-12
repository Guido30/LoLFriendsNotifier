// Hide the terminal on release builds on windows
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::{NativeOptions, egui::viewport};

use gui::FriendsNotifierApp;

mod consts;
mod gui;
mod threads;

// TODO implement logging using the tracing crate
// TODO fix set volume not working on the sink

fn main() -> eframe::Result {
    // We define a single native window
    let native_options = NativeOptions {
        viewport: viewport::ViewportBuilder::default()
            .with_min_inner_size([400.0, 200.0])
            .with_max_inner_size([550.0, 390.0])
            .with_maximize_button(false)
            .with_app_id("friends_notifier")
            .with_icon(eframe::icon_data::from_png_bytes(consts::ASSET_ICON).expect("Failed loading icon")),
        ..Default::default()
    };
    // Run the main egui loop
    eframe::run_native("Friends Notifier", native_options, Box::new(|cc| Ok(Box::new(FriendsNotifierApp::new(cc)))))
}
