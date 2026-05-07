//! libcosmic Application: the main GUI window.
//!
//! M0 was an empty window with a placeholder Pick button.
//! M1 wires the button to the overlay and renders a result view with the
//! picked color in hex, rgb, hsl, oklch — each row with a copy button.

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
}

#[derive(Debug, Clone)]
pub enum Message {
    PickPressed,
    PickResult(Option<String>),
    Copy(String),
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
        };
        (app, Task::none())
    }

    fn header_start(&self) -> Vec<Element<'_, Message>> {
        vec![widget::text::heading(fl!("app-title")).into()]
    }

    fn view(&self) -> Element<'_, Message> {
        let pick_button = widget::button::suggested(if self.picked.is_some() {
            fl!("pick-another")
        } else {
            fl!("pick-button")
        })
        .on_press_maybe((!self.picking).then_some(Message::PickPressed));

        let body = match &self.picked {
            None => self.welcome_view(),
            Some(p) => self.result_view(p),
        };

        let column = widget::Column::new()
            .padding(20)
            .spacing(20)
            .push(pick_button)
            .push(body);

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
                // Run the overlay on a blocking thread so the GUI loop stays
                // responsive. Returns the picked hex (or None if cancelled).
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
                }
            }
            Message::Copy(text) => {
                return cosmic::iced::clipboard::write(text);
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
        widget::Column::new()
            .padding(8)
            .spacing(12)
            .push(widget::text::title3(fl!("welcome-title")))
            .push(widget::text::body(fl!("welcome-body")))
            .into()
    }

    fn result_view(&self, p: &PickedColor) -> Element<'_, Message> {
        let swatch = self.swatch(p.rgb);

        let headline = widget::Column::new()
            .spacing(4)
            .push(widget::text::title2(p.hex()))
            .push(
                widget::button::link(fl!("copy"))
                    .on_press(Message::Copy(p.hex())),
            );

        let header_row = widget::Row::new()
            .spacing(16)
            .align_y(cosmic::iced::Alignment::Center)
            .push(swatch)
            .push(headline);

        let rows = widget::Column::new()
            .spacing(8)
            .push(format_row(&fl!("format-rgb"), p.rgb_str()))
            .push(format_row(&fl!("format-hsl"), p.hsl_str()))
            .push(format_row(&fl!("format-oklch"), p.oklch_str()));

        widget::Column::new()
            .spacing(16)
            .push(header_row)
            .push(widget::divider::horizontal::default())
            .push(rows)
            .into()
    }

    fn swatch(&self, rgb: (u8, u8, u8)) -> Element<'_, Message> {
        let color = cosmic::iced::Color::from_rgb8(rgb.0, rgb.1, rgb.2);
        widget::container(widget::Space::new())
            .width(Length::Fixed(96.0))
            .height(Length::Fixed(96.0))
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

/// One row of "label  •  value text  •  Copy" used by the format readout.
fn format_row<'a>(label: &str, value: String) -> Element<'a, Message> {
    let value_for_copy = value.clone();
    widget::Row::new()
        .spacing(12)
        .align_y(cosmic::iced::Alignment::Center)
        .push(
            widget::text::body(label.to_string())
                .width(Length::Fixed(70.0)),
        )
        .push(widget::text::monotext(value).width(Length::Fill))
        .push(
            widget::button::standard("Copy")
                .on_press(Message::Copy(value_for_copy)),
        )
        .into()
}
