use eframe::{
    App, CreationContext,
    egui::{
        self, Align, Button, Color32, ComboBox, CursorIcon, DragValue,
        FontFamily::Proportional,
        FontId, Frame, Id, Image, Layout, Margin, Modal, RichText, ScrollArea, Slider, TextEdit,
        TextStyle::*,
        Theme, Vec2,
        containers::{CentralPanel, Tooltip, TopBottomPanel},
        text::{LayoutJob, TextFormat},
    },
};
use lolclientapi_rs::blocking::LeagueClient;
use notify_rust::Notification;
use serde::{Deserialize, Serialize};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;
use uuid::Uuid;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::consts;
use crate::threads;

const ALLOWED_MIN_FRIENDS: usize = 1;
const ALLOWED_MAX_FRIENDS: usize = 10;

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

#[derive(Debug, Clone, Default, PartialEq)]
pub enum FriendStatus {
    Online,
    InGame,
    Mobile,
    Away,
    #[default]
    Offline,
}

#[derive(Debug, Clone, Default)]
pub enum GuiMessage {
    ClientStatus(bool),
    FriendStatus(Vec<(FriendRiotId, FriendAvailability)>),
    SpawnTimer(Friend),
    Notify(Friend),
    #[default]
    NoMessage,
}

#[derive(Debug, Clone, Default)]
pub enum SoundMessage {
    PlaySound(String),
    SetVolume(u8),
    #[default]
    NoMessage,
}

// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(Debug, Deserialize, Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct FriendsNotifierApp {
    friends: Vec<Friend>,
    #[serde(skip)]
    g_sx: Sender<GuiMessage>,
    #[serde(skip)]
    g_rx: Receiver<GuiMessage>,
    #[serde(skip)]
    s_sx: Sender<SoundMessage>,
    #[serde(skip)]
    client_status: bool,
    #[serde(skip)]
    settings_open: bool,
    native_notification: bool,
    volume: u8,
    notify_away_status: bool,
    notify_in_game_status: bool,
    dark_mode: bool,
}

impl FriendsNotifierApp {
    // Called once before the first frame to initialize gui configuration.
    pub fn new(cc: &CreationContext<'_>) -> Self {
        egui_extras::install_image_loaders(&cc.egui_ctx);

        // Redefine Global Fonts
        let text_styles: BTreeMap<_, _> = [
            (Heading, FontId::new(18.0, Proportional)),
            (Body, FontId::new(10.0, Proportional)),
            (Monospace, FontId::new(10.0, Proportional)),
            (Button, FontId::new(10.0, Proportional)),
            (Small, FontId::new(8.0, Proportional)),
        ]
        .into();
        cc.egui_ctx.all_styles_mut(move |style| {
            style.text_styles = text_styles.clone();
            style.interaction.selectable_labels = false;
        });

        let mut app: FriendsNotifierApp;
        // Load previous app state (if any).
        if let Some(storage) = cc.storage {
            app = eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default();
        } else {
            app = FriendsNotifierApp::default();
        }

        app.dark_mode = match cc.egui_ctx.theme() {
            Theme::Dark => true,
            Theme::Light => false,
        };

        let client = Arc::new(Mutex::new(LeagueClient::new()));
        let (g_sx, g_rx) = channel::<GuiMessage>();
        let (s_sx, s_rx) = channel::<SoundMessage>();
        // Initialize client threads
        threads::league_client(g_sx.clone(), Some(client.clone()));
        threads::sound_handler(s_rx);

        app.g_sx = g_sx;
        app.g_rx = g_rx;
        app.s_sx = s_sx;
        app
    }
}

impl App for FriendsNotifierApp {
    fn save(&mut self, _storage: &mut dyn eframe::Storage) {
        eframe::set_value(_storage, eframe::APP_KEY, &self);
    }

    fn auto_save_interval(&self) -> Duration {
        Duration::from_secs(30)
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
            // Update gui friend status, and send notification if is found in an active state
            GuiMessage::FriendStatus(fr) => {
                for f in self.friends.iter_mut() {
                    if let Some((_, new_status_raw)) =
                        fr.iter().find(|_f| _f.0 == f.name.to_lowercase())
                    {
                        let new_status: FriendStatus = new_status_raw.clone().into();
                        let old_status = f.status.clone(); // Clone old status for comparison before mutation.

                        // Determine if a notification should be sent based on several conditions.
                        let should_notify = {
                            // Helper closure to determine if a status is considered "active" based on settings.
                            // This now includes checking for the new `notify_in_game_status` flag.
                            let is_status_active = |status: &FriendStatus| -> bool {
                                matches!(status, FriendStatus::Online)
                                    || (self.notify_away_status
                                        && matches!(status, FriendStatus::Away))
                                    || (self.notify_in_game_status
                                        && matches!(status, FriendStatus::InGame))
                            };

                            let was_active = is_status_active(&old_status);
                            let is_now_active = is_status_active(&new_status);

                            // A meaningful change happens when the friend transitions between active and non-active states.
                            let is_meaningful_change = was_active != is_now_active;

                            // Explicitly ignore notifications for becoming "Away" or "InGame" if their respective settings are off,
                            // even if it's considered a meaningful change (e.g., Online -> Away).
                            let is_ignored_transition = (!self.notify_away_status
                                && matches!(new_status, FriendStatus::Away))
                                || (!self.notify_in_game_status
                                    && matches!(new_status, FriendStatus::InGame));

                            f.enabled && is_meaningful_change && !is_ignored_transition
                        };

                        // Always update the friend's status to reflect the latest data.
                        f.status = new_status;
                        if should_notify {
                            let _ = self.g_sx.send(GuiMessage::Notify(f.clone()));
                        }
                    } else {
                        // For friends not found in the API response set them to Offline.
                        f.status = FriendStatus::Offline;
                    }
                }
            }
            GuiMessage::ClientStatus(status) => {
                // Set all friends statuses to Offline, this is needed in case the client is closed after retrieving some statuses
                if !status {
                    for f in self.friends.iter_mut() {
                        f.status = FriendStatus::Offline
                    }
                }

                self.client_status = status;
            }
            // Spawn a timer thread when a friend is enabled, at timeout try to send a notification
            GuiMessage::SpawnTimer(f) => {
                let g_sx = self.g_sx.clone();
                thread::spawn(move || {
                    thread::sleep(Duration::from_secs(f.notify_timer as u64));
                    let _ = g_sx.send(GuiMessage::Notify(f));
                });
            }
            // When timer is triggered we check if conditions changed while waiting for the timer
            GuiMessage::Notify(fr) => {
                if let Some(friend) = self.friends.iter().find(|f| f == &&fr) {
                    // Friend must be online and timer_id matches
                    // Timer_id mismatch happens because the background thread that triggers after
                    // the defined timer times out has no idea whether this was the the original call to
                    // spawn it or a different one for the same friend (each time a friend is enabled it will spawn a timer)
                    // and since timer_id is regenerated everytime a friend is enabled we make sure it was the
                    // last call to spawn it by comparing the timer id
                    match (
                        &friend.status,
                        self.notify_away_status,
                        self.notify_in_game_status,
                    ) {
                        (FriendStatus::Online, _, _)
                        | (FriendStatus::Away, true, _)
                        | (FriendStatus::InGame, _, true) => {
                            if friend.timer_id == fr.timer_id {
                                // Handle repeating the notification
                                if friend.is_repeat {
                                    let _ = self.g_sx.send(GuiMessage::SpawnTimer(friend.clone()));
                                }
                                // Now that conditions are met, play the sound associated with this Friend
                                let _ = self
                                    .s_sx
                                    .send(SoundMessage::PlaySound(friend.sound.1.clone()));
                                // Send the windows notification if enabled
                                if self.native_notification {
                                    let _ = Notification::new()
                                        .appname("Friends Notifier")
                                        .timeout(Duration::from_millis(5000))
                                        .body(&format!("{} is Online!", fr.name))
                                        .auto_icon()
                                        .finalize()
                                        .show();
                                };
                            }
                        }
                        _ => {}
                    }
                };
            }
            _ => {}
        }

        // Apply tint to loaded images to match theme
        let icon_gear: Image;
        let icon_dash: Image;
        let icon_plus: Image;
        let icon_check: Image;
        let icon_repeat: Image;
        if let Theme::Light = ctx.theme() {
            icon_gear = Image::new(consts::ASSET_ICON_GEAR).tint(Color32::DARK_GRAY);
            icon_dash = Image::new(consts::ASSET_ICON_DASH).tint(Color32::DARK_GRAY);
            icon_plus = Image::new(consts::ASSET_ICON_PLUS).tint(Color32::DARK_GRAY);
            icon_check = Image::new(consts::ASSET_ICON_CHECK).tint(Color32::DARK_GRAY);
            icon_repeat = Image::new(consts::ASSET_ICON_REPEAT)
                .max_height(14.0)
                .tint(Color32::DARK_GRAY);
        } else {
            icon_gear = Image::new(consts::ASSET_ICON_GEAR);
            icon_dash = Image::new(consts::ASSET_ICON_DASH);
            icon_plus = Image::new(consts::ASSET_ICON_PLUS);
            icon_check = Image::new(consts::ASSET_ICON_CHECK);
            icon_repeat = Image::new(consts::ASSET_ICON_REPEAT).max_height(14.0);
        }

        // Start defining the gui and its interactions with the state
        // Footer of this app, contains settings btn and client status widgets
        TopBottomPanel::bottom("bottom_panel").show(ctx, |ui| {
            Frame::new()
                .inner_margin(Vec2::new(0.0, 5.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let settings_btn = Button::image_and_text(icon_gear, "Settings");
                        if ui.add(settings_btn).clicked() {
                            self.settings_open = !self.settings_open;
                        };
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.style_mut().spacing.item_spacing = [5.0, 0.0].into();
                            ui.add(match self.client_status {
                                true => Image::new(consts::ASSET_ICON_CIRCLE_FILLED_GREEN),
                                false => Image::new(consts::ASSET_ICON_CIRCLE_FILLED_RED),
                            });
                            ui.label(RichText::from("Client").italics().size(11.0));
                        })
                    });
                });
        });

        // Main central layouts and widgets of the app
        CentralPanel::default()
            .frame(
                // New frame required to set margin
                Frame::new()
                    .inner_margin(Margin {
                        left: 5,
                        right: 5,
                        top: 0,
                        bottom: 5,
                    })
                    .fill(ctx.theme().default_visuals().panel_fill),
            )
            .show(ctx, |ui| {
                // Table Header
                ui.style_mut().spacing.item_spacing = [0.0, 3.0].into();
                ui.horizontal(|ui| {
                    ui.set_height(2.0);
                    ui.add_space(25.0);
                    ui.label("Name#Tag").on_hover_cursor(CursorIcon::Default);
                    ui.add_space(80.0);
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.add_space(70.0);
                        ui.label("Repeat").on_hover_cursor(CursorIcon::Default);
                        ui.add_space(82.0);
                        ui.label("Sound").on_hover_cursor(CursorIcon::Default);
                    })
                });
                ui.separator();
                // Table Body, we start defining it from the most outer widgets/layouts
                // so later the main ScrollArea can expand to fill the whole window space
                ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
                    ui.with_layout(Layout::right_to_left(Align::Max), |ui| {
                        ui.horizontal(|ui| {
                            // New frame 'container' to define some top margin
                            Frame::new()
                                .inner_margin(Margin {
                                    top: 2,
                                    left: 0,
                                    right: 0,
                                    bottom: 2,
                                })
                                .show(ui, |ui| {
                                    // Add/Remove rows buttons are added
                                    ui.style_mut().spacing.item_spacing = [5.0, 0.0].into();
                                    if ui.add(Button::image(icon_dash)).clicked()
                                        && self.friends.len() > ALLOWED_MIN_FRIENDS
                                    {
                                        let _f = self.friends.pop().unwrap();
                                    };
                                    if ui.add(Button::image(icon_plus)).clicked()
                                        && self.friends.len() < ALLOWED_MAX_FRIENDS
                                    {
                                        let _f = Friend::default();
                                        self.friends.push(_f.clone());
                                    };
                                });
                        });
                    });
                    ui.with_layout(Layout::top_down(Align::Center), |ui| {
                        // Main scroll area of this app where friend rows will be added
                        // Has to be the last nested child so it can take as much space left within the main window
                        ScrollArea::vertical().show(ui, |ui| {
                            for (i, friend) in self.friends.iter_mut().enumerate() {
                                ui.horizontal(|ui| {
                                    ui.style_mut().spacing.item_spacing = [2.0, 0.0].into();
                                    // Friend notification enabling button widget
                                    if ui
                                        .add(
                                            Button::selectable(friend.enabled, icon_check.clone())
                                                .frame_when_inactive(true),
                                        )
                                        .clicked()
                                    {
                                        friend.enabled = !friend.enabled;
                                        friend.timer_id = Uuid::new_v4();
                                        if friend.enabled {
                                            let _ =
                                                self.g_sx.send(GuiMessage::Notify(friend.clone()));
                                        };
                                    };
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        ui.style_mut().spacing.button_padding = [10.0, 0.0].into();
                                        // Friend status icon widget
                                        let friend_status_img = match friend.status {
                                            FriendStatus::Online => {
                                                Image::new(consts::ASSET_ICON_CIRCLE_FILLED_GREEN)
                                            }
                                            FriendStatus::InGame => {
                                                Image::new(consts::ASSET_ICON_CIRCLE_FILLED_CYAN)
                                            }
                                            FriendStatus::Mobile => {
                                                Image::new(consts::ASSET_ICON_CIRCLE_FILLED_GREY)
                                            }
                                            FriendStatus::Away => {
                                                Image::new(consts::ASSET_ICON_CIRCLE_FILLED_YELLOW)
                                            }
                                            FriendStatus::Offline => {
                                                Image::new(consts::ASSET_ICON_CIRCLE_FILLED_RED)
                                            }
                                        };
                                        let friend_status_img_res = ui.add(friend_status_img);
                                        Tooltip::for_enabled(&friend_status_img_res).show(|ui| {
                                            match friend.status {
                                                FriendStatus::Online => ui.label("Online"),
                                                FriendStatus::InGame => ui.label("In Game"),
                                                FriendStatus::Mobile => ui.label("Mobile"),
                                                FriendStatus::Away => ui.label("Away"),
                                                FriendStatus::Offline => ui.label("Offline"),
                                            }
                                        });

                                        ui.separator();
                                        // Repeat notification button and value widgets
                                        ui.add_enabled(
                                            !friend.enabled,
                                            DragValue::new(&mut friend.notify_timer)
                                                .range(5..=100)
                                                .suffix("s")
                                                .update_while_editing(false),
                                        );
                                        if ui
                                            .add(
                                                Button::selectable(
                                                    friend.is_repeat,
                                                    icon_repeat.clone(),
                                                )
                                                .frame_when_inactive(true),
                                            )
                                            .clicked()
                                        {
                                            friend.is_repeat = !friend.is_repeat;
                                            if friend.is_repeat {
                                                friend.timer_id = Uuid::new_v4();
                                                let _ = self
                                                    .g_sx
                                                    .send(GuiMessage::Notify(friend.clone()));
                                            }
                                        };
                                        ui.separator();
                                        // Friend specific sound, combobox selector widget
                                        let before = &friend.sound.0.clone();
                                        ComboBox::from_id_salt(format!("box{i}"))
                                            .selected_text(&friend.sound.0)
                                            .show_ui(ui, |ui| {
                                                ui.selectable_value(
                                                    &mut friend.sound.0,
                                                    consts::ASSET_SOUNDS[0].0.to_string(),
                                                    consts::ASSET_SOUNDS[0].0,
                                                );
                                                ui.selectable_value(
                                                    &mut friend.sound.0,
                                                    consts::ASSET_SOUNDS[1].0.to_string(),
                                                    consts::ASSET_SOUNDS[1].0,
                                                );
                                                ui.selectable_value(
                                                    &mut friend.sound.0,
                                                    consts::ASSET_SOUNDS[2].0.to_string(),
                                                    consts::ASSET_SOUNDS[2].0,
                                                );
                                                ui.selectable_value(
                                                    &mut friend.sound.0,
                                                    consts::ASSET_SOUNDS[3].0.to_string(),
                                                    consts::ASSET_SOUNDS[3].0,
                                                );
                                                ui.selectable_value(
                                                    &mut friend.sound.0,
                                                    consts::ASSET_SOUNDS[4].0.to_string(),
                                                    consts::ASSET_SOUNDS[4].0,
                                                );
                                                ui.selectable_value(
                                                    &mut friend.sound.0,
                                                    consts::ASSET_SOUNDS[5].0.to_string(),
                                                    consts::ASSET_SOUNDS[5].0,
                                                );
                                                ui.selectable_value(
                                                    &mut friend.sound.0,
                                                    consts::ASSET_SOUNDS[6].0.to_string(),
                                                    consts::ASSET_SOUNDS[6].0,
                                                );
                                                ui.selectable_value(
                                                    &mut friend.sound.0,
                                                    consts::ASSET_SOUNDS[7].0.to_string(),
                                                    consts::ASSET_SOUNDS[7].0,
                                                );
                                                ui.selectable_value(
                                                    &mut friend.sound.0,
                                                    consts::ASSET_SOUNDS[8].0.to_string(),
                                                    consts::ASSET_SOUNDS[8].0,
                                                );
                                                ui.selectable_value(
                                                    &mut friend.sound.0,
                                                    consts::ASSET_SOUNDS[9].0.to_string(),
                                                    consts::ASSET_SOUNDS[9].0,
                                                );
                                                ui.selectable_value(
                                                    &mut friend.sound.0,
                                                    consts::ASSET_SOUNDS[10].0.to_string(),
                                                    consts::ASSET_SOUNDS[10].0,
                                                );
                                                ui.selectable_value(
                                                    &mut friend.sound.0,
                                                    consts::ASSET_SOUNDS[11].0.to_string(),
                                                    consts::ASSET_SOUNDS[11].0,
                                                );
                                                ui.selectable_value(
                                                    &mut friend.sound.0,
                                                    consts::ASSET_SOUNDS[12].0.to_string(),
                                                    consts::ASSET_SOUNDS[12].0,
                                                );
                                            });
                                        // Handle changing the underlying sound played when notifying for this friend
                                        if &friend.sound.0 != before {
                                            if let Some(s) = consts::ASSET_SOUNDS
                                                .iter()
                                                .find(|_s| _s.0 == friend.sound.0)
                                            {
                                                friend.sound = (s.0.to_string(), s.1.to_string());
                                                let _ = self.s_sx.send(SoundMessage::PlaySound(
                                                    friend.sound.1.clone(),
                                                ));
                                            }
                                        }
                                        ui.separator();
                                        // Finally the friend name box widget
                                        ui.add_sized(
                                            ui.available_size(),
                                            TextEdit::singleline(&mut friend.name)
                                                .vertical_align(Align::Center)
                                                .interactive(!friend.enabled),
                                        )
                                    })
                                });
                                ui.separator();
                            }
                        });
                    });
                });
            });
        // Settings modal, only drawn when it is supposed to be open
        if self.settings_open
            && Modal::new(Id::new("settings_modal"))
                .show(ctx, |ui| {
                    ui.set_max_width(200.0);
                    ui.horizontal(|ui| {
                        ui.heading("Settings").on_hover_cursor(CursorIcon::Default);
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.add_space(10.0);
                            if ui.add(Button::new("X").frame(false)).clicked() {
                                self.settings_open = false;
                            };
                        })
                    });
                    ui.separator();

                    Frame::new()
                        .inner_margin(Margin {
                            left: 10,
                            right: 10,
                            top: 6,
                            bottom: 2,
                        })
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing = [0.0, 1.0].into();
                            ui.horizontal(|ui| {
                                ui.style_mut().spacing.button_padding = [6.0, 0.0].into();
                                ui.style_mut().spacing.item_spacing = [4.0, 0.0].into();
                                ui.label("Theme");
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if ui.selectable_label(self.dark_mode, "Dark").clicked() {
                                        self.dark_mode = true;
                                        ctx.set_theme(Theme::Dark);
                                    };
                                    if ui.selectable_label(!self.dark_mode, "Light").clicked() {
                                        self.dark_mode = false;
                                        ctx.set_theme(Theme::Light);
                                    };
                                });
                            });
                            ui.add_space(3.0);
                            ui.horizontal(|ui| {
                                ui.label("Volume").on_hover_cursor(CursorIcon::Default);

                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if ui
                                        .add_sized(
                                            ui.available_size(),
                                            Slider::new(&mut self.volume, 0..=100)
                                                .handle_shape(egui::style::HandleShape::Rect {
                                                    aspect_ratio: 1.2,
                                                })
                                                .show_value(false),
                                        )
                                        .drag_stopped()
                                    {
                                        let _ =
                                            self.s_sx.send(SoundMessage::SetVolume(self.volume));
                                    };
                                })
                            });
                            ui.separator();
                            ui.horizontal(|ui| {
                                ui.label("Windows Notification");

                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    ui.checkbox(&mut self.native_notification, "");
                                })
                            });
                            ui.horizontal(|ui| {
                                let mut text = LayoutJob::default();
                                text.append(
                                    "Notify ",
                                    0.0,
                                    TextFormat {
                                        font_id: FontId {
                                            size: 10.0,
                                            ..Default::default()
                                        },
                                        color: ctx.style().visuals.text_color(),
                                        ..Default::default()
                                    },
                                );
                                text.append(
                                    "In Game",
                                    0.0,
                                    TextFormat {
                                        font_id: FontId {
                                            size: 10.0,
                                            ..Default::default()
                                        },
                                        color: Color32::from_rgb(10, 203, 230),
                                        ..Default::default()
                                    },
                                );
                                text.append(
                                    " status",
                                    0.0,
                                    TextFormat {
                                        font_id: FontId {
                                            size: 10.0,
                                            ..Default::default()
                                        },
                                        color: ctx.style().visuals.text_color(),
                                        ..Default::default()
                                    },
                                );
                                ui.label(text);

                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    ui.checkbox(&mut self.notify_in_game_status, "");
                                })
                            });
                            ui.horizontal(|ui| {
                                let mut text = LayoutJob::default();
                                text.append(
                                    "Notify ",
                                    0.0,
                                    TextFormat {
                                        font_id: FontId {
                                            size: 10.0,
                                            ..Default::default()
                                        },
                                        color: ctx.style().visuals.text_color(),
                                        ..Default::default()
                                    },
                                );
                                text.append(
                                    "Away",
                                    0.0,
                                    TextFormat {
                                        font_id: FontId {
                                            size: 10.0,
                                            ..Default::default()
                                        },
                                        color: Color32::from_rgb(255, 130, 0),
                                        ..Default::default()
                                    },
                                );
                                text.append(
                                    " status",
                                    0.0,
                                    TextFormat {
                                        font_id: FontId {
                                            size: 10.0,
                                            ..Default::default()
                                        },
                                        color: ctx.style().visuals.text_color(),
                                        ..Default::default()
                                    },
                                );
                                ui.label(text);
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    ui.checkbox(&mut self.notify_away_status, "");
                                })
                            });
                        });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(RichText::from("v".to_string() + consts::APP_VERSION).size(8.0));
                    })
                })
                .should_close()
        {
            self.settings_open = false;
        };
    }
}

impl Default for FriendsNotifierApp {
    fn default() -> Self {
        let (g_sx, g_rx) = channel::<GuiMessage>();
        let (s_sx, _) = channel::<SoundMessage>();
        Self {
            friends: vec![Friend::default()],
            g_sx,
            g_rx,
            s_sx,
            client_status: false,
            settings_open: false,
            native_notification: false,
            volume: 100,
            notify_away_status: false,
            notify_in_game_status: true,
            dark_mode: true,
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
            sound: (
                consts::ASSET_SOUNDS[0].0.to_string(),
                consts::ASSET_SOUNDS[0].1.to_string(),
            ),
            notify_timer: 5,
            is_repeat: false,
            status: FriendStatus::default(),
        }
    }
}

// Friend uniqueness depends on generated uuid, otherwise two different added friends
// could be the same if their name was equal, and during logical comparison
// only the first one would be used for such operations
impl PartialEq for Friend {
    fn eq(&self, other: &Self) -> bool {
        self.uuid == other.uuid
    }
}

impl From<String> for FriendStatus {
    fn from(value: String) -> Self {
        let value = &*value;
        match value {
            "chat" => FriendStatus::Online,
            "dnd" => FriendStatus::InGame,
            "mobile" => FriendStatus::Mobile,
            "away" => FriendStatus::Away,
            _ => FriendStatus::Offline,
        }
    }
}
