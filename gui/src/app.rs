//! libcosmic Application: the main GUI window.
//!
//! M2 layout polish: hero card with large swatch + huge hex; format rows in
//! a settings::section with icon copy buttons; history strip in its own
//! card. Uses libcosmic theme tokens (Card container, settings::item) so
//! the visual treatment matches Cosmic Settings and other native apps.

use crate::autostart;
use crate::color::PickedColor;
use crate::config::Config;
use crate::fl;
use crate::ipc;
use cosmic::app::{Core, Task};
use cosmic::cosmic_config::{self, CosmicConfigEntry};
use cosmic::iced::futures::channel::mpsc;
use cosmic::iced::futures::SinkExt;
use cosmic::iced::window;
use cosmic::iced::{Length, Subscription};
use cosmic::prelude::*;
use cosmic::widget;

#[derive(Default)]
pub struct Flags {
    /// Reserved for autostart wiring (M5).
    pub background: bool,
}

pub struct AppModel {
    core: Core,
    config: Config,
    flags: Flags,
    /// Most recently picked color, displayed in the result view.
    picked: Option<PickedColor>,
    /// True while the overlay is running, used to debounce repeated clicks.
    picking: bool,
    /// Recent picks, newest first. Capped at HISTORY_LIMIT. Mirrored to
    /// `config.history` (which is persisted by cosmic-config).
    history: Vec<PickedColor>,
    /// Which page is showing in the main view.
    page: Page,
    /// Cached result of `autostart::is_enabled()`, refreshed on entering the
    /// settings page so the toggle reflects the on-disk truth.
    autostart_enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Page {
    Home,
    Settings,
}

const HISTORY_LIMIT: usize = 16;

#[derive(Debug, Clone)]
pub enum Message {
    PickPressed,
    PickResult(Option<String>),
    Copy(String),
    SelectHistory(usize),
    ClearHistory,
    UpdateConfig(Config),
    OpenSettings,
    CloseSettings,
    ToggleAutostart(bool),
    /// IPC asked us to show the window (i.e. user re-launched while daemon
    /// was running, or hit a "Show" hotkey).
    ShowWindow,
    /// Window close clicked: hide instead of exit so the daemon survives.
    HideWindow,
}

impl cosmic::Application for AppModel {
    type Executor = cosmic::executor::Default;
    type Flags = Flags;
    type Message = Message;
    const APP_ID: &'static str = "com.pyxyll.CosmicColorPicker";

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(core: Core, flags: Self::Flags) -> (Self, Task<Message>) {
        let config = cosmic_config::Config::new(Self::APP_ID, Config::VERSION)
            .map(|ctx| match Config::get_entry(&ctx) {
                Ok(c) => c,
                Err((_e, c)) => c,
            })
            .unwrap_or_default();

        let history = parse_history(&config.history);
        // Picked starts as the most recent entry (if any) so the result view
        // is populated immediately on relaunch — feels nicer than a blank
        // welcome state when there's history to show.
        let picked = history.first().copied();

        let want_hidden = flags.background;
        let app = AppModel {
            core,
            config,
            flags,
            picked,
            picking: false,
            history,
            page: Page::Home,
            autostart_enabled: autostart::is_enabled(),
        };

        // Background mode: dispatch a HideWindow as the very first message so
        // the window starts visible for an instant, then minimises away. iced
        // doesn't expose "create hidden" cleanly, so this is the pragmatic
        // path. Visible blip is single-frame; users won't notice on fast HW.
        let task = if want_hidden {
            Task::done(cosmic::Action::App(Message::HideWindow))
        } else {
            Task::none()
        };
        (app, task)
    }

    // NOTE on closing: libcosmic forcibly calls iced::exit() when the main
    // window is closed (via core.exit_on_main_window_closed, no public
    // setter as of this writing). We can't intercept and convert "close" to
    // "hide" — the user clicking X always kills the daemon. Instead, we
    // expose an explicit "Hide" button in the header that calls set_mode
    // without triggering the close path. --background mode never shows the
    // window at all, so the daemon lives until manually killed.

    fn header_start(&self) -> Vec<Element<'_, Message>> {
        vec![widget::text::heading(fl!("app-title")).into()]
    }

    fn header_end(&self) -> Vec<Element<'_, Message>> {
        let nav_icon = match self.page {
            Page::Home => "emblem-system-symbolic",
            Page::Settings => "go-previous-symbolic",
        };
        let nav_msg = match self.page {
            Page::Home => Message::OpenSettings,
            Page::Settings => Message::CloseSettings,
        };

        let hide = widget::button::icon(
            widget::icon::from_name("window-minimize-symbolic"),
        )
        .on_press(Message::HideWindow);

        let nav = widget::button::icon(widget::icon::from_name(nav_icon))
            .on_press(nav_msg);

        // Hide first so the typical action (send window away) sits closer to
        // the user's pointer-of-attention than the settings gear.
        vec![hide.into(), nav.into()]
    }

    fn view(&self) -> Element<'_, Message> {
        let pick_label = if self.picked.is_some() {
            fl!("pick-another")
        } else {
            fl!("pick-button")
        };
        let pick_button = widget::button::suggested(pick_label)
            .on_press_maybe(
                (self.page == Page::Home && !self.picking).then_some(Message::PickPressed),
            );

        let body = match self.page {
            Page::Settings => self.settings_view(),
            Page::Home => match &self.picked {
                None => self.welcome_view(),
                Some(p) => self.result_view(p),
            },
        };

        // Pick button stays pinned at the top; the rest scrolls so the
        // history strip is always reachable even at the smallest window
        // size. Spacing on the inner column gives breathing room between
        // the cards without the scrollbar overlapping them.
        let header = widget::Row::new()
            .align_y(cosmic::iced::Alignment::Center)
            .push(widget::Space::new().width(Length::Fill))
            .push(pick_button);

        let scrollable_body = widget::scrollable(
            widget::Column::new().padding([0, 4, 0, 0]).push(body),
        )
        .height(Length::Fill);

        let column = widget::Column::new()
            .padding([16, 24, 16, 24])
            .spacing(16)
            .push(header)
            .push(scrollable_body);

        widget::container(column).width(Length::Fill).into()
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            self.core()
                .watch_config::<Config>(Self::APP_ID)
                .map(|update| Message::UpdateConfig(update.config)),
            // Listen for pick requests from `cosmic-color-picker --pick`
            // (i.e. the user's hotkey). Each accepted connection translates
            // into a single PickPressed message handled by the normal
            // overlay path, so the result lands in this app's history.
            Subscription::run(|| {
                cosmic::iced::stream::channel::<Message>(
                    8,
                    |mut tx: mpsc::Sender<Message>| async move {
                        ipc::clean_stale_socket();
                        let Ok(listener) =
                            tokio::net::UnixListener::bind(ipc::socket_path())
                        else {
                            return;
                        };
                        loop {
                            let Ok((mut stream, _)) = listener.accept().await else {
                                continue;
                            };
                            use tokio::io::AsyncReadExt;
                            let mut buf = [0u8; 1];
                            let _ = stream.read(&mut buf).await;
                            let msg = match buf[0] {
                                b's' => Message::ShowWindow,
                                _ => Message::PickPressed,
                            };
                            let _ = tx.send(msg).await;
                        }
                    },
                )
            }),
        ])
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::PickPressed => {
                if self.picking {
                    return Task::none();
                }
                self.picking = true;
                // D0 transitional: spawn the daemon binary as a one-shot
                // subprocess and read the picked hex off stdout. D2 swaps
                // this for an IPC round-trip into a long-running daemon.
                return Task::perform(
                    async {
                        tokio::task::spawn_blocking(|| {
                            let out = std::process::Command::new("cosmic-color-pickerd")
                                .arg("--quiet")
                                .output()
                                .ok()?;
                            if !out.status.success() {
                                return None;
                            }
                            let s = String::from_utf8(out.stdout).ok()?;
                            let trimmed = s.trim();
                            if trimmed.is_empty() {
                                None
                            } else {
                                Some(trimmed.to_string())
                            }
                        })
                        .await
                        .ok()
                        .flatten()
                    },
                    |hex| cosmic::Action::App(Message::PickResult(hex)),
                );
            }
            Message::PickResult(hex) => {
                self.picking = false;
                if let Some(picked) = hex.as_deref().and_then(PickedColor::from_hex) {
                    self.picked = Some(picked);
                    self.history.insert(0, picked);
                    self.history.truncate(HISTORY_LIMIT);
                    self.save_history();
                }
            }
            Message::Copy(text) => {
                return cosmic::iced::clipboard::write(text);
            }
            Message::SelectHistory(i) => {
                if let Some(p) = self.history.get(i).copied() {
                    self.picked = Some(p);
                }
            }
            Message::ClearHistory => {
                self.history.clear();
                self.save_history();
            }
            Message::UpdateConfig(c) => {
                self.config = c;
                // Re-parse so the in-memory list matches whatever just landed
                // on disk (e.g. someone editing the config file directly).
                self.history = parse_history(&self.config.history);
            }
            Message::OpenSettings => {
                self.autostart_enabled = autostart::is_enabled();
                self.page = Page::Settings;
            }
            Message::CloseSettings => {
                self.page = Page::Home;
            }
            Message::ToggleAutostart(on) => {
                let result = if on {
                    autostart::enable()
                } else {
                    autostart::disable()
                };
                if let Err(e) = result {
                    eprintln!("color picker: autostart toggle failed: {e}");
                }
                self.autostart_enabled = autostart::is_enabled();
            }
            Message::ShowWindow => {
                if let Some(id) = self.core.main_window_id() {
                    return window::set_mode(id, window::Mode::Windowed);
                }
            }
            Message::HideWindow => {
                if let Some(id) = self.core.main_window_id() {
                    return window::set_mode(id, window::Mode::Hidden);
                }
            }
        }
        Task::none()
    }
}

impl AppModel {
    /// Persist the current history list to cosmic-config. Failure is silent —
    /// we'd rather lose a pick than crash the app.
    fn save_history(&mut self) {
        let app_id = <Self as cosmic::Application>::APP_ID;
        if let Ok(ctx) = cosmic_config::Config::new(app_id, Config::VERSION) {
            self.config.history =
                self.history.iter().map(PickedColor::hex).collect();
            let _ = self.config.write_entry(&ctx);
        }
    }

    fn settings_view(&self) -> Element<'_, Message> {
        let autostart_row = widget::settings::item(
            fl!("settings-autostart"),
            widget::toggler(self.autostart_enabled).on_toggle(Message::ToggleAutostart),
        );

        let section = widget::settings::section()
            .title(fl!("settings-startup"))
            .add(autostart_row);

        widget::Column::new()
            .spacing(16)
            .push(section)
            .push(widget::text::caption(fl!("settings-autostart-hint")))
            .into()
    }

    fn welcome_view(&self) -> Element<'_, Message> {
        widget::container(
            widget::Column::new()
                .spacing(12)
                .align_x(cosmic::iced::Alignment::Center)
                .push(widget::icon::from_name("color-select-symbolic").size(64))
                .push(widget::text::title3(fl!("welcome-title")))
                .push(widget::text::body(fl!("welcome-body"))),
        )
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .padding(48)
        .into()
    }

    fn result_view(&self, p: &PickedColor) -> Element<'_, Message> {
        let mut col = widget::Column::new()
            .spacing(16)
            .push(self.hero_card(p))
            .push(self.formats_card(p));

        if !self.history.is_empty() {
            col = col.push(self.history_card());
        }

        col.into()
    }

    fn hero_card(&self, p: &PickedColor) -> Element<'_, Message> {
        let swatch = self.color_block(p.rgb, 120.0);

        let copy_hex = widget::button::icon(
            widget::icon::from_name("edit-copy-symbolic"),
        )
        .extra_small()
        .on_press(Message::Copy(p.hex()));

        let title = widget::Row::new()
            .spacing(8)
            .align_y(cosmic::iced::Alignment::Center)
            .push(widget::text::title1(p.hex()))
            .push(copy_hex);

        let info = widget::Column::new()
            .spacing(4)
            .push(title)
            .push(widget::text::caption(p.rgb_str()));

        let row = widget::Row::new()
            .spacing(20)
            .align_y(cosmic::iced::Alignment::Center)
            .push(swatch)
            .push(info);

        widget::container(row)
            .padding(20)
            .width(Length::Fill)
            .class(cosmic::theme::style::Container::Card)
            .into()
    }

    fn formats_card(&self, p: &PickedColor) -> Element<'_, Message> {
        widget::settings::section()
            .add(format_item(&fl!("format-rgb"), p.rgb_str()))
            .add(format_item(&fl!("format-hsl"), p.hsl_str()))
            .add(format_item(&fl!("format-oklch"), p.oklch_str()))
            .into()
    }

    fn history_card(&self) -> Element<'_, Message> {
        let mut strip = widget::Row::new().spacing(8);
        for (i, c) in self.history.iter().enumerate() {
            strip = strip.push(self.history_chip(i, c.rgb));
        }

        let header = widget::Row::new()
            .align_y(cosmic::iced::Alignment::Center)
            .push(widget::text::heading(fl!("history-title")).width(Length::Fill))
            .push(
                widget::button::link(fl!("history-clear"))
                    .on_press(Message::ClearHistory),
            );

        let body = widget::Column::new()
            .spacing(12)
            .push(header)
            .push(strip);

        widget::container(body)
            .padding(20)
            .width(Length::Fill)
            .class(cosmic::theme::style::Container::Card)
            .into()
    }

    fn history_chip(&self, idx: usize, rgb: (u8, u8, u8)) -> Element<'_, Message> {
        widget::button::custom(self.color_block(rgb, 36.0))
            .padding(0)
            .class(cosmic::theme::style::Button::Standard)
            .on_press(Message::SelectHistory(idx))
            .into()
    }

    fn color_block(&self, rgb: (u8, u8, u8), size: f32) -> Element<'_, Message> {
        let color = cosmic::iced::Color::from_rgb8(rgb.0, rgb.1, rgb.2);
        widget::container(widget::Space::new())
            .width(Length::Fixed(size))
            .height(Length::Fixed(size))
            .class(cosmic::theme::style::Container::custom(
                move |theme: &cosmic::Theme| {
                    let cosmic = theme.cosmic();
                    cosmic::iced::widget::container::Style {
                        background: Some(color.into()),
                        border: cosmic::iced::Border {
                            radius: cosmic.corner_radii.radius_s.into(),
                            width: 1.0,
                            color: cosmic.background.divider.into(),
                        },
                        ..Default::default()
                    }
                },
            ))
            .into()
    }
}

/// Decode a list of `#RRGGBB` strings into PickedColors, dropping anything
/// malformed. Used both at startup (loading from disk) and on UpdateConfig
/// (when an external edit triggers a refresh).
fn parse_history(hex_list: &[String]) -> Vec<PickedColor> {
    hex_list.iter().filter_map(|s| PickedColor::from_hex(s)).collect()
}

/// A settings-list row: label on the left, monospace value, copy icon button.
fn format_item<'a>(label: &str, value: String) -> Element<'a, Message> {
    let value_for_copy = value.clone();
    let trailing = widget::Row::new()
        .spacing(12)
        .align_y(cosmic::iced::Alignment::Center)
        .push(widget::text::monotext(value))
        .push(
            widget::button::icon(widget::icon::from_name("edit-copy-symbolic"))
                .extra_small()
                .on_press(Message::Copy(value_for_copy)),
        );

    widget::settings::item(label.to_string(), trailing).into()
}
