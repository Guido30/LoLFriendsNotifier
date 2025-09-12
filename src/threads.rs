use lolclientapi_rs::blocking::LeagueClient;
use rodio::{Decoder, Sink};
use std::io::Cursor;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::consts;
use crate::gui::{GuiMessage, SoundMessage};

/// Thread responsible to periodically run operations on the lcu api
/// The main goals are to retrieve the client status and available friends every num seconds
pub fn league_client(g_sx: Sender<GuiMessage>, client: Option<Arc<Mutex<LeagueClient>>>) {
    thread::spawn(move || {
        let client = client.unwrap_or_else(|| Arc::new(Mutex::new(LeagueClient::new())));

        loop {
            if let Ok(mut c) = client.try_lock() {
                let is_connected = c.status() || c.retry();
                let _ = g_sx.send(GuiMessage::ClientStatus(is_connected));

                // Retrieves friends from the API and maps them into a Vec<(String, String)>
                // then sends them on the client channel
                if is_connected {
                    if let Ok(f) = c.get_lol_chat_v1_friends() {
                        let f: Vec<(String, String)> = f
                            .into_iter()
                            .map(|_f| ((_f.game_name + "#" + &_f.game_tag).to_lowercase(), _f.availability.to_lowercase()))
                            .collect();
                        let _ = g_sx.send(GuiMessage::FriendStatus(f));
                    }
                }
            }
            thread::sleep(Duration::from_secs(3));
        }
    });
}

// Thread responsible to initialize the audio stream, load sound files and play them on demand
pub fn sound_handler(s_rx: Receiver<SoundMessage>) {
    thread::spawn(move || {
        // Initilize audio device
        let stream_handle = rodio::OutputStreamBuilder::open_default_stream().unwrap();
        let sink = Sink::connect_new(stream_handle.mixer());
        // Load sound files in memory
        let mut sound_files = vec![];
        for (_, path) in consts::ASSET_SOUNDS {
            let bytes = std::fs::read(path).unwrap();
            sound_files.push((path.to_string(), bytes));
        }
        // When the message fires we play the sound at the specific path
        loop {
            if let Ok(msg) = s_rx.recv() {
                match msg {
                    SoundMessage::PlaySound(path) => {
                        if let Some((_, sound_bytes)) = sound_files.iter().find(|f| f.0 == path) {
                            let cursor = Cursor::new(sound_bytes.clone());
                            let source = Decoder::new(cursor).unwrap();
                            stream_handle.mixer().add(source);
                        }
                    }
                    SoundMessage::SetVolume(v) => {
                        sink.set_volume(v as f32 / 100.0);
                    }
                    _ => {}
                }
            }
        }
    });
}
