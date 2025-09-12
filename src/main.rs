// Hide the terminal on release builds on windows
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::{NativeOptions, egui::viewport};

use gui::FriendsNotifierApp;

mod consts;
mod gui;
mod threads;

// TODO when repeat is turned on during an active timer, if the repeat was turned off it wont recreate one
// TODO When the client is closed, the friend statuses remain online if they were online
// TODO Implement Away and maybe mobile status handling, also away as offline option
// TODO implement logging usin the tracing crate
// TODO find and fix bug that after sometime that the app is running it stops sending notifications, threads die? or what? find out

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
