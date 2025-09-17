// Hide the terminal on release builds on windows
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::{
    NativeOptions,
    egui::{ImageSource, include_image, viewport},
};
use lolclientapi_rs::blocking::LeagueClient;
use rodio::{Decoder, Sink};
use std::io::Cursor;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use gui::{ApiFriend, FriendsNotifierApp, Message};

mod gui;

const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
const ALLOWED_MIN_FRIENDS: usize = 1;
const ALLOWED_MAX_FRIENDS: usize = 10;

// Compiled assets
const ASSET_ICON: &[u8] = include_bytes!("icons/icon.png");
const ASSET_ICON_GEAR: ImageSource = include_image!("icons/bootstrap_gear_fill.svg");
const ASSET_ICON_PLUS: ImageSource = include_image!("icons/bootstrap_plus.svg");
const ASSET_ICON_DASH: ImageSource = include_image!("icons/bootstrap_dash.svg");
const ASSET_ICON_CHECK: ImageSource = include_image!("icons/bootstrap_check.svg");
const ASSET_ICON_REPEAT: ImageSource = include_image!("icons/bootstrap_repeat.svg");
const ASSET_ICON_CIRCLE_FILLED_GREY: ImageSource = include_image!("icons/vscode-codicon_circle-filled-grey.svg");
const ASSET_ICON_CIRCLE_FILLED_RED: ImageSource = include_image!("icons/vscode-codicon_circle-filled-red.svg");
const ASSET_ICON_CIRCLE_FILLED_GREEN: ImageSource = include_image!("icons/vscode-codicon_circle-filled-green.svg");
const ASSET_ICON_CIRCLE_FILLED_CYAN: ImageSource = include_image!("icons/vscode-codicon_circle-filled-cyan.svg");
const ASSET_ICON_CIRCLE_FILLED_YELLOW: ImageSource = include_image!("icons/vscode-codicon_circle-filled-yellow.svg");

// Sound files are loaded at runtime once to avoid increasing binary size
// they must be in the defined paths at runtime otherwise the sound thread will error
const ASSET_SOUNDS: [(&str, &str); 13] = [
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

/// Thread responsible to periodically run operations on the lcu api
/// The main goals are to retrieve the client status and available friends every num seconds
fn start_polling_league_client(g_sx: Sender<Message>, client: Option<Arc<Mutex<LeagueClient>>>) {
    thread::spawn(move || {
        let client = client.unwrap_or_else(|| Arc::new(Mutex::new(LeagueClient::new())));

        loop {
            if let Ok(mut c) = client.try_lock() {
                let is_connected = c.status() || c.retry();
                let _ = g_sx.send(Message::ClientStatus(is_connected));

                // Retrieves friends from the API and maps them into a Vec<(String, String)>
                // then sends them on the client channel
                if is_connected {
                    if let Ok(f) = c.get_lol_chat_v1_friends() {
                        let f: Vec<ApiFriend> = f
                            .into_iter()
                            .map(|_f| ApiFriend {
                                riot_id: (_f.game_name + "#" + &_f.game_tag).to_lowercase(),
                                availability: _f.availability.to_lowercase(),
                            })
                            .collect();
                        let _ = g_sx.send(Message::FriendStatus(f));
                    }
                }
            }
            thread::sleep(Duration::from_secs(3));
        }
    });
}

// Thread responsible to initialize the audio stream, load sound files and play them on demand
fn start_audio_message_receiver(s_rx: Receiver<Message>) {
    thread::spawn(move || {
        // Initilize audio device
        let stream_handle = rodio::OutputStreamBuilder::open_default_stream().unwrap();
        let sink = Sink::connect_new(stream_handle.mixer());
        // Load sound files in memory
        let mut sound_files = vec![];
        for (_, path) in ASSET_SOUNDS {
            let bytes = std::fs::read(path).unwrap();
            sound_files.push((path.to_string(), bytes));
        }
        // When the message fires we play the sound at the specific path
        loop {
            if let Ok(msg) = s_rx.recv() {
                match msg {
                    Message::PlaySound(path) => {
                        if let Some((_, sound_bytes)) = sound_files.iter().find(|f| f.0 == path) {
                            let cursor = Cursor::new(sound_bytes.clone());
                            let source = Decoder::new(cursor).unwrap();
                            stream_handle.mixer().add(source);
                        }
                    }
                    Message::SetVolume(v) => {
                        sink.set_volume(v as f32 / 100.0); // TODO fix set volume not affecting the playback volume on the sink
                    }
                    _ => {}
                }
            }
        }
    });
}

fn main() -> eframe::Result {
    // We define a single native window
    let native_options = NativeOptions {
        viewport: viewport::ViewportBuilder::default()
            .with_min_inner_size([400.0, 200.0])
            .with_max_inner_size([550.0, 390.0])
            .with_maximize_button(false)
            .with_app_id("friends_notifier")
            .with_icon(eframe::icon_data::from_png_bytes(ASSET_ICON).expect("Failed loading icon")),
        ..Default::default()
    };
    // Run the main egui loop
    eframe::run_native("Friends Notifier", native_options, Box::new(|cc| Ok(Box::new(FriendsNotifierApp::new(cc)))))
}
