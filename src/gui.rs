use eframe::{
    App, CreationContext,
    egui::{
        self, Align, Button, Color32, ComboBox, DragValue,
        FontFamily::Proportional,
        FontId, Frame, Id, Image, ImageSource, Layout, Modal, RichText, ScrollArea, TextEdit,
        TextStyle::*,
        Vec2,
        containers::{CentralPanel, TopBottomPanel},
    },
};
use lolclientapi_rs::blocking::LeagueClient;
use serde::{Deserialize, Serialize};
use std::sync::mpsc::{Receiver, Sender, channel};
use uuid::Uuid;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::client;

const APP_COLOR_STATUS_UNAVAILABLE: Color32 = Color32::from_rgb(70, 70, 70); // TODO implement theming for this app
const ASSET_ICON_GEAR: ImageSource = egui::include_image!("icons/bootstrap_gear.svg");
const ASSET_ICON_PLUS: ImageSource = egui::include_image!("icons/bootstrap_plus.svg");
const ASSET_ICON_DASH: ImageSource = egui::include_image!("icons/bootstrap_dash.svg");
const ASSET_ICON_CHECK: ImageSource = egui::include_image!("icons/bootstrap_check.svg");
const ASSET_ICON_REPEAT: ImageSource = egui::include_image!("icons/bootstrap_repeat.svg");
const ASSET_ICON_CIRCLE_FILLED_GREY: ImageSource = egui::include_image!("icons/vscode-codicon_circle-filled-grey.svg");
const ASSET_ICON_CIRCLE_FILLED_RED: ImageSource = egui::include_image!("icons/vscode-codicon_circle-filled-red.svg");
const ASSET_ICON_CIRCLE_FILLED_GREEN: ImageSource = egui::include_image!("icons/vscode-codicon_circle-filled-green.svg");
const ALLOWED_MIN_FRIENDS: usize = 1;
const ALLOWED_MAX_FRIENDS: usize = 10;
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

type FriendAvailability = String;
type FriendRiotId = String;
type SoundLabel = String;
type SoundPath = String;
type Sound = (SoundLabel, SoundPath);

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Friend {
    pub uuid: Uuid,
    pub timer_id: Uuid,
    #[serde(skip)]
    pub enabled: bool,
    pub name: String,
    pub sound: Sound,
    pub is_repeat: bool,
    pub notify_timer: u16,
    #[serde(skip)]
    pub status: FriendStatus,
}

#[derive(Debug, Clone, Default)]
pub enum FriendStatus {
    Online,
    Mobile,
    #[default]
    Offline,
}

#[derive(Debug, Clone, Default)]
pub enum ClientMessage {
    SpawnTimer(Friend),
    Notify(Friend),
    AddFriend(Friend),
    RemoveFriend(Friend),
    UpdateName(Friend),
    #[default]
    NoMessage,
}

#[derive(Debug, Clone, Default)]
pub enum GuiMessage {
    ClientStatus(bool),
    FriendStatus(Vec<(FriendRiotId, FriendAvailability)>),
    TimerTriggered(Friend),
    #[default]
    NoMessage,
}

#[derive(Debug, Clone, Default)]
pub struct PlaySound(pub SoundPath);

// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(Debug, Deserialize, Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct FriendNotifierApp {
    friends: Vec<Friend>,
    #[serde(skip)]
    c_sx: Sender<ClientMessage>,
    #[serde(skip)]
    g_rx: Receiver<GuiMessage>,
    #[serde(skip)]
    s_sx: Sender<PlaySound>,
    #[serde(skip)]
    client_status: bool,
    #[serde(skip)]
    pub settings_open: bool,
}

impl FriendNotifierApp {
    // Called once before the first frame to initialize gui configuration.
    pub fn new(cc: &CreationContext<'_>) -> Self {
        egui_extras::install_image_loaders(&cc.egui_ctx);

        // Redefine Global Fonts
        let text_styles: BTreeMap<_, _> = [
            (Heading, FontId::new(28.0, Proportional)),
            (Name("Heading2".into()), FontId::new(23.0, Proportional)),
            (Name("Context".into()), FontId::new(21.0, Proportional)),
            (Body, FontId::new(12.0, Proportional)),
            (Monospace, FontId::new(12.0, Proportional)),
            (Button, FontId::new(12.0, Proportional)),
            (Small, FontId::new(8.0, Proportional)),
        ]
        .into();
        cc.egui_ctx.all_styles_mut(move |style| {
            style.text_styles = text_styles.clone();
            #[cfg(debug_assertions)]
            {
                // style.debug = DebugOptions {
                //     debug_on_hover: true,
                //     debug_on_hover_with_all_modifiers: false,
                //     hover_shows_next: true,
                //     show_expand_height: true,
                //     show_expand_width: true,
                //     show_interactive_widgets: false,
                //     show_resize: true,
                //     show_unaligned: true,
                //     show_widget_hits: false,
                // };
            }
        });

        let mut app: FriendNotifierApp;
        // Load previous app state (if any).
        if let Some(storage) = cc.storage {
            app = eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default();
        } else {
            app = FriendNotifierApp::default();
        }

        let client = Arc::new(Mutex::new(LeagueClient::new()));
        let (c_sx, c_rx) = channel::<ClientMessage>();
        let (g_sx, g_rx) = channel::<GuiMessage>();
        let (s_sx, s_rx) = channel::<PlaySound>();
        // Initialize client threads
        client::threaded_client_on_timer(c_sx.clone(), g_sx.clone(), Some(client.clone()));
        client::threaded_message_handler(app.friends.clone(), c_rx, c_sx.clone(), g_sx.clone(), Some(client.clone()));
        client::threaded_sound_player(s_rx);

        app.c_sx = c_sx;
        app.g_rx = g_rx;
        app.s_sx = s_sx;
        app
    }
}

impl App for FriendNotifierApp {
    fn save(&mut self, _storage: &mut dyn eframe::Storage) {
        eframe::set_value(_storage, eframe::APP_KEY, &self);
    }

    fn auto_save_interval(&self) -> Duration {
        Duration::from_secs(10)
    }

    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        // Force egui to repaint every 16ms (60 FPS), avoids 'pausing' the application
        // background threads are also paused if a repaint is not esplicitly requested
        // meaning notifying is out of sync with actual specified Friends timers and only
        // trigger on user interaction with the gui, there might be performance implications
        // by running it every loop, TODO call ctx.request_repaint when needed from the other threads
        ctx.request_repaint();

        // Handle messages to mutate state before initializing widgets
        let msg = self.g_rx.try_recv().unwrap_or_default();
        match msg {
            GuiMessage::FriendStatus(fr) => {
                for f in self.friends.iter_mut() {
                    match fr.iter().find(|_f| _f.0 == f.name.to_lowercase()) {
                        Some(_f) => {
                            f.status = _f.1.clone().into();
                        }
                        None => f.status = FriendStatus::Offline,
                    }
                }

                for f in fr {
                    if let Some(friend) = self.friends.iter_mut().find(|_f| _f.name == f.0) {
                        friend.status = f.1.clone().into();
                    }
                }
            }
            GuiMessage::ClientStatus(status) => {
                self.client_status = status;
            }
            // When timer is triggered we check if conditions changed while waiting for the timer
            GuiMessage::TimerTriggered(fr) => {
                if let Some(friend) = self.friends.iter().find(|f| f == &&fr) {
                    // Friend must be online and timer_id must match
                    if matches!(friend.status, FriendStatus::Online) && friend.timer_id == fr.timer_id {
                        // Handle repeating the notification
                        if friend.is_repeat {
                            let _ = self.c_sx.send(ClientMessage::SpawnTimer(friend.clone()));
                        }

                        // Now that conditions are met, play the sound associated with this Friend
                        let _ = self.s_sx.send(PlaySound(friend.sound.1.clone()));
                    }
                };
            }
            _ => {}
        }

        // Initialize widgets based on current state
        let client_status_img = match self.client_status {
            true => Image::new(ASSET_ICON_CIRCLE_FILLED_GREEN),
            false => Image::new(ASSET_ICON_CIRCLE_FILLED_RED),
        };

        TopBottomPanel::bottom("bottom_panel").show(ctx, |ui| {
            Frame::new().inner_margin(Vec2::new(0.0, 5.0)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    let settings_btn = Button::image_and_text(Image::new(ASSET_ICON_GEAR), "Settings");
                    if ui.add(settings_btn).clicked() {
                        self.settings_open = !self.settings_open;
                    };
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.style_mut().spacing.item_spacing = [5.0, 0.0].into();
                        ui.add(client_status_img);
                        ui.label(RichText::from("Client").italics());
                    })
                });
            });
        });
        CentralPanel::default().show(ctx, |ui| {
            let panel_style = ui.style_mut();
            panel_style.spacing.item_spacing = [0.0, 5.0].into();
            // Table Header
            ui.horizontal(|ui| {
                ui.add_space(25.0);
                ui.label("Name#Tag");
                ui.add_space(80.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.add_space(68.0);
                    ui.label("Repeat");
                    ui.add_space(80.0);
                    ui.label("Sound");
                })
            });
            ui.separator();
            // Table Body
            ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
                ui.with_layout(Layout::right_to_left(Align::Max), |ui| {
                    ui.style_mut().spacing.item_spacing = [5.0, 2.0].into();
                    if ui.add(Button::image(Image::new(ASSET_ICON_DASH))).clicked() && self.friends.len() > ALLOWED_MIN_FRIENDS {
                        let _f = self.friends.pop().unwrap();
                        let _ = self.c_sx.send(ClientMessage::RemoveFriend(_f));
                    };
                    if ui.add(Button::image(Image::new(ASSET_ICON_PLUS))).clicked() && self.friends.len() < ALLOWED_MAX_FRIENDS {
                        let _f = Friend::default();
                        self.friends.push(_f.clone());
                        let _ = self.c_sx.send(ClientMessage::AddFriend(_f));
                    };
                });
                ui.with_layout(Layout::top_down(Align::Center), |ui| {
                    ScrollArea::vertical().show(ui, |ui| {
                        for (i, friend) in self.friends.iter_mut().enumerate() {
                            ui.horizontal(|ui| {
                                ui.style_mut().spacing.item_spacing = [2.0, 0.0].into();
                                // Friend Notification Enabling
                                if ui.add(Button::image(ASSET_ICON_CHECK).selected(friend.enabled)).clicked() {
                                    friend.enabled = !friend.enabled;
                                    friend.timer_id = Uuid::new_v4();
                                    if friend.enabled {
                                        let _ = self.c_sx.send(ClientMessage::SpawnTimer(friend.clone()));
                                    };
                                };
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    ui.style_mut().spacing.button_padding = [10.0, 0.0].into();
                                    // Friend status icon
                                    let friend_status_img = match friend.status {
                                        FriendStatus::Online => Image::new(ASSET_ICON_CIRCLE_FILLED_GREEN),
                                        FriendStatus::Mobile => Image::new(ASSET_ICON_CIRCLE_FILLED_GREY),
                                        FriendStatus::Offline => Image::new(ASSET_ICON_CIRCLE_FILLED_RED),
                                    };
                                    ui.add(friend_status_img);
                                    ui.separator();
                                    // Repeat Notification Buttons
                                    ui.add_enabled(
                                        !friend.enabled,
                                        DragValue::new(&mut friend.notify_timer).range(5..=100).suffix("s").update_while_editing(false),
                                    );
                                    if ui.add(Button::image(ASSET_ICON_REPEAT).selected(friend.is_repeat)).clicked() {
                                        friend.is_repeat = !friend.is_repeat;
                                    };
                                    ui.separator();
                                    // Sound Combobox
                                    let before = &friend.sound.0.clone();
                                    ComboBox::from_id_salt(format!("box{i}")).selected_text(&friend.sound.0).show_ui(ui, |ui| {
                                        ui.selectable_value(&mut friend.sound.0, ASSET_SOUNDS[0].0.to_string(), ASSET_SOUNDS[0].0);
                                        ui.selectable_value(&mut friend.sound.0, ASSET_SOUNDS[1].0.to_string(), ASSET_SOUNDS[1].0);
                                        ui.selectable_value(&mut friend.sound.0, ASSET_SOUNDS[2].0.to_string(), ASSET_SOUNDS[2].0);
                                        ui.selectable_value(&mut friend.sound.0, ASSET_SOUNDS[3].0.to_string(), ASSET_SOUNDS[3].0);
                                        ui.selectable_value(&mut friend.sound.0, ASSET_SOUNDS[4].0.to_string(), ASSET_SOUNDS[4].0);
                                        ui.selectable_value(&mut friend.sound.0, ASSET_SOUNDS[5].0.to_string(), ASSET_SOUNDS[5].0);
                                        ui.selectable_value(&mut friend.sound.0, ASSET_SOUNDS[6].0.to_string(), ASSET_SOUNDS[6].0);
                                        ui.selectable_value(&mut friend.sound.0, ASSET_SOUNDS[7].0.to_string(), ASSET_SOUNDS[7].0);
                                        ui.selectable_value(&mut friend.sound.0, ASSET_SOUNDS[8].0.to_string(), ASSET_SOUNDS[8].0);
                                        ui.selectable_value(&mut friend.sound.0, ASSET_SOUNDS[9].0.to_string(), ASSET_SOUNDS[9].0);
                                        ui.selectable_value(&mut friend.sound.0, ASSET_SOUNDS[10].0.to_string(), ASSET_SOUNDS[10].0);
                                        ui.selectable_value(&mut friend.sound.0, ASSET_SOUNDS[11].0.to_string(), ASSET_SOUNDS[11].0);
                                        ui.selectable_value(&mut friend.sound.0, ASSET_SOUNDS[12].0.to_string(), ASSET_SOUNDS[12].0);
                                    });
                                    if &friend.sound.0 != before {
                                        // Update the sound if it was changes using the checkbox
                                        if let Some(s) = ASSET_SOUNDS.iter().find(|_s| _s.0 == friend.sound.0) {
                                            friend.sound = (s.0.to_string(), s.1.to_string());
                                        }
                                    }
                                    ui.separator();
                                    // Friend Name text box
                                    if ui
                                        .add_sized(ui.available_size(), TextEdit::singleline(&mut friend.name).interactive(!friend.enabled))
                                        .changed()
                                    {
                                        let _ = self.c_sx.send(ClientMessage::UpdateName(friend.clone()));
                                    };
                                })
                            });
                            ui.separator();
                        }
                    });
                });
            });
        });
        if self.settings_open {
            if Modal::new(Id::new("settings_modal"))
                .show(ctx, |ui| {
                    ui.label("Testing");
                })
                .should_close()
            {
                self.settings_open = false;
            };
        }
    }
}

impl Default for FriendNotifierApp {
    fn default() -> Self {
        let (c_sx, _) = channel::<ClientMessage>();
        let (_, g_rx) = channel::<GuiMessage>();
        let (s_sx, _) = channel::<PlaySound>();
        Self {
            friends: vec![Friend::default()],
            c_sx,
            g_rx,
            s_sx,
            client_status: false,
            settings_open: false,
        }
    }
}

impl Default for Friend {
    fn default() -> Self {
        Self {
            uuid: Uuid::new_v4(),
            timer_id: Uuid::new_v4(),
            enabled: false,
            name: "".to_string(),
            sound: (ASSET_SOUNDS[0].0.to_string(), ASSET_SOUNDS[0].1.to_string()),
            notify_timer: 5,
            is_repeat: false,
            status: FriendStatus::default(),
        }
    }
}

impl PartialEq for Friend {
    fn eq(&self, other: &Self) -> bool {
        self.uuid == other.uuid
    }
}

impl From<String> for FriendStatus {
    fn from(value: String) -> Self {
        let value = &*value;
        match value {
            "dnd" | "chat" | "away" => FriendStatus::Online,
            "mobile" => FriendStatus::Mobile,
            _ => FriendStatus::Offline,
        }
    }
}
