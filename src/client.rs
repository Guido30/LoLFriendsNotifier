use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use lolclientapi_rs::blocking::LeagueClient;

use crate::gui::{ClientMessage, Friend, GuiMessage};

/// Runs a LeagueClient in another thread, sending messages on the channel's senders
/// The main goals are to retrieve the client status and available friends every x seconds
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

/// Runs a client message handler in another thread, making use of the LeagueClient when necessary
pub fn threaded_client_message_handler(
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
                    // Play the sound assigned to Friend in another thread
                    ClientMessage::Notify(f) => {
                        let c = client.clone();
                        let g_sx = g_sx.clone();
                        // Handle client requests in a different thread than the current message handler
                        thread::spawn(move || println!("Notifying.. {:?}", f));
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
