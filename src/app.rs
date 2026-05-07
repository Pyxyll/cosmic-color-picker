//! libcosmic Application: the main GUI window.
//!
//! M2 layout polish: hero card with large swatch + huge hex; format rows in
//! a settings::section with icon copy buttons; history strip in its own
//! card. Uses libcosmic theme tokens (Card container, settings::item) so
//! the visual treatment matches Cosmic Settings and other native apps.

use crate::color::PickedColor;
use crate::config::Config;
use crate::fl;
use crate::overlay;
use cosmic::app::{Core, Task};
use cosmic::cosmic_config::{self, CosmicConfigEntry};
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
    #[allow(dead_code)]
    config: Config,
    #[allow(dead_code)]
    flags: Flags,
    /// Most recently picked color, displayed in the result view.
    picked: Option<PickedColor>,
    /// True while the overlay is running, used to debounce repeated clicks.
    picking: bool,
    /// Recent picks, newest first. Capped at HISTORY_LIMIT. M3 makes this
    /// persistent via cosmic-config; right now it lives in-memory only.
    history: Vec<PickedColor>,
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

        let app = AppModel {
            core,
            config,
            flags,
            picked: None,
            picking: false,
            history: Vec::new(),
        };
        (app, Task::none())
    }

    fn header_start(&self) -> Vec<Element<'_, Message>> {
        vec![widget::text::heading(fl!("app-title")).into()]
    }

    fn view(&self) -> Element<'_, Message> {
        let pick_label = if self.picked.is_some() {
            fl!("pick-another")
        } else {
            fl!("pick-button")
        };
        let pick_button = widget::button::suggested(pick_label)
            .on_press_maybe((!self.picking).then_some(Message::PickPressed));

        let body = match &self.picked {
            None => self.welcome_view(),
            Some(p) => self.result_view(p),
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
        self.core()
            .watch_config::<Config>(Self::APP_ID)
            .map(|update| Message::UpdateConfig(update.config))
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::PickPressed => {
                if self.picking {
                    return Task::none();
                }
                self.picking = true;
                return Task::perform(
                    async {
                        tokio::task::spawn_blocking(|| {
                            overlay::pick_color().ok().flatten()
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
            }
            Message::UpdateConfig(c) => {
                self.config = c;
            }
        }
        Task::none()
    }
}

impl AppModel {
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
