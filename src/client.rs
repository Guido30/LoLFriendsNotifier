use lolclientapi_rs::blocking::LeagueClient;
use rodio::{Decoder, OutputStreamBuilder, Sink, source::Source};
use std::fs::File;
use std::io::BufReader;
use std::io::Cursor;
use std::io::Seek;
use std::io::SeekFrom;

use std::ops::Deref;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::gui::{ASSET_SOUNDS, ClientMessage, Friend, GuiMessage, PlaySound};

/// New thread responsible for periodically run operations on the lcu api
/// The main goals are to retrieve the client status and available friends every num seconds
pub fn threaded_client_on_timer(c_sx: Sender<ClientMessage>, g_sx: Sender<GuiMessage>, client: Option<Arc<Mutex<LeagueClient>>>) {
    thread::spawn(move || {
        let client = client.unwrap_or_else(|| Arc::new(Mutex::new(LeagueClient::new())));

        loop {
            if let Ok(mut c) = client.try_lock() {
                let is_connected = c.status() || c.retry();

                let _ = g_sx.send(GuiMessage::ClientStatus(is_connected));

                // Retrieves friends from the API and maps them into a Vec<(String, String)>
                // then sends them on the client channel
                if let Ok(f) = c.get_lol_chat_v1_friends() {
                    let f: Vec<(String, String)> = f
                        .into_iter()
                        .map(|_f| ((_f.game_name + "#" + &_f.game_tag).to_lowercase(), _f.availability.to_lowercase()))
                        .collect();
                    let _ = g_sx.send(GuiMessage::FriendStatus(f));
                }
            }
            thread::sleep(Duration::from_secs(3));
        }
    });
}

/// Runs a message handler in another thread to alleviate load on the main gui loop.
/// Since moving Friends assigned to the state of the gui to another thread would be complicated
/// we have to store clones of Friends in this thread, and 'synchronize' them using messages
pub fn threaded_message_handler(
    startup_friends: Vec<Friend>,
    c_rx: Receiver<ClientMessage>,
    c_sx: Sender<ClientMessage>,
    g_sx: Sender<GuiMessage>,
    client: Option<Arc<Mutex<LeagueClient>>>,
) {
    thread::spawn(move || {
        let mut friends = startup_friends;
        let client = client.unwrap_or_default();
        loop {
            if let Ok(msg) = c_rx.recv() {
                match msg {
                    ClientMessage::SpawnTimer(f) => {
                        let g_sx = g_sx.clone();
                        thread::spawn(move || {
                            thread::sleep(Duration::from_secs(f.notify_timer as u64));
                            let _ = g_sx.send(GuiMessage::TimerTriggered(f));
                        });
                    }
                    ClientMessage::AddFriend(f) => {
                        if !friends.contains(&f) {
                            friends.push(f);
                        }
                    }
                    ClientMessage::RemoveFriend(f) => {
                        if let Some((i, __f)) = friends.iter().enumerate().find(|_f| _f.1 == &f) {
                            friends.remove(i);
                        }
                    }
                    ClientMessage::UpdateName(f) => {
                        if let Some(_f) = friends.iter_mut().find(|_f| _f == &&f) {
                            _f.name = f.name.clone();
                        }
                    }
                    _ => {}
                }
            }
        }
    });
}

pub fn threaded_sound_player(s_rx: Receiver<PlaySound>) {
    thread::spawn(move || {
        // Initilize audio device
        let stream_handle = rodio::OutputStreamBuilder::open_default_stream().unwrap();
        let _sink = Sink::connect_new(&stream_handle.mixer());
        // Load sound files in memory
        let mut sound_files = vec![];
        for (_, path) in ASSET_SOUNDS {
            let bytes = std::fs::read(path).unwrap();
            sound_files.push((path.to_string(), bytes));
        }
        // When the message fires we play the sound at the specific path
        loop {
            if let Ok(path) = s_rx.recv() {
                if let Some((_, sound_bytes)) = sound_files.iter().find(|f| f.0 == path.0) {
                    let cursor = Cursor::new(sound_bytes.clone());
                    let source = Decoder::new(cursor).unwrap();
                    stream_handle.mixer().add(source);
                }
            }
        }
    });
}
