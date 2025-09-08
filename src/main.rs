#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::{NativeOptions, egui::viewport};
use gui::FriendNotifierApp;

mod client;
mod gui;

const ASSET_ICON: &[u8] = include_bytes!("icons/icon.png");

fn main() -> eframe::Result {
    let native_options = NativeOptions {
        viewport: viewport::ViewportBuilder::default()
            .with_min_inner_size([400.0, 200.0])
            .with_max_inner_size([550.0, 440.0])
            .with_maximize_button(false)
            .with_app_id("friends_notifier")
            .with_icon(eframe::icon_data::from_png_bytes(ASSET_ICON).expect("Failed loading icon")),
        ..Default::default()
    };
    eframe::run_native("Friends Notifier", native_options, Box::new(|cc| Ok(Box::new(FriendNotifierApp::new(cc)))))
}
