use eframe::egui::{self, Color32, ImageSource};

const _APP_COLOR_STATUS_UNAVAILABLE: Color32 = Color32::from_rgb(70, 70, 70); // TODO implement theming for this app

// Compiled assets
pub const ASSET_ICON: &[u8] = include_bytes!("icons/icon.png");
pub const ASSET_ICON_GEAR: ImageSource = egui::include_image!("icons/bootstrap_gear_fill.svg");
pub const ASSET_ICON_PLUS: ImageSource = egui::include_image!("icons/bootstrap_plus.svg");
pub const ASSET_ICON_DASH: ImageSource = egui::include_image!("icons/bootstrap_dash.svg");
pub const ASSET_ICON_CHECK: ImageSource = egui::include_image!("icons/bootstrap_check.svg");
pub const ASSET_ICON_REPEAT: ImageSource = egui::include_image!("icons/bootstrap_repeat.svg");
pub const ASSET_ICON_CIRCLE_FILLED_GREY: ImageSource = egui::include_image!("icons/vscode-codicon_circle-filled-grey.svg");
pub const ASSET_ICON_CIRCLE_FILLED_RED: ImageSource = egui::include_image!("icons/vscode-codicon_circle-filled-red.svg");
pub const ASSET_ICON_CIRCLE_FILLED_GREEN: ImageSource = egui::include_image!("icons/vscode-codicon_circle-filled-green.svg");
pub const ASSET_ICON_CIRCLE_FILLED_CYAN: ImageSource = egui::include_image!("icons/vscode-codicon_circle-filled-cyan.svg");
pub const ASSET_ICON_CIRCLE_FILLED_YELLOW: ImageSource = egui::include_image!("icons/vscode-codicon_circle-filled-yellow.svg");

// Sound files are loaded at runtime once to avoid increasing binary size
// they must be in the defined paths at runtime otherwise the sound thread will error
pub const ASSET_SOUNDS: [(&str, &str); 13] = [
    ("Sound 1", "assets/notification-1.mp3"),
    ("Sound 2", "assets/notification-2.mp3"),
    ("Sound 3", "assets/notification-3.mp3"),
    ("Sound 4", "assets/notification-4.mp3"),
    ("Sound 5", "assets/notification-5.mp3"),
    ("Sound 6", "assets/notification-6.mp3"),
    ("Sound 7", "assets/notification-7.mp3"),
    ("Sound 8", "assets/notification-8.mp3"),
    ("Sound 9", "assets/notification-9.mp3"),
    ("Sound 10", "assets/notification-10.mp3"),
    ("Sound 11", "assets/notification-11.mp3"),
    ("Sound 12", "assets/notification-12.mp3"),
    ("Sound 13", "assets/notification-13.mp3"),
];
