//! libcosmic Application: the GUI window.
//!
//! M0: opens an empty window with a single "Pick a color" button. The button
//! handler will be wired in M1 to fire the overlay and display the result.

use crate::config::Config;
use crate::fl;
use cosmic::app::{Core, Task};
use cosmic::cosmic_config::{self, CosmicConfigEntry};
use cosmic::iced::Subscription;
use cosmic::prelude::*;
use cosmic::widget;

#[derive(Default)]
pub struct Flags {
    /// True if the app was launched with `--background` (autostart).
    /// In that case we don't actually create the visible window. Implemented
    /// in M0 just by tracking the flag; window-suppression wiring is M5.
    pub background: bool,
}

pub struct AppModel {
    core: Core,
    #[allow(dead_code)] // wired in M3 / M5 / M6
    config: Config,
    #[allow(dead_code)]
    flags: Flags,
}

#[derive(Debug, Clone)]
pub enum Message {
    PickPressed,
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
        };
        (app, Task::none())
    }

    fn header_start(&self) -> Vec<Element<'_, Message>> {
        vec![widget::text::heading(fl!("app-title")).into()]
    }

    fn view(&self) -> Element<'_, Message> {
        let body = widget::Column::new()
            .padding(24)
            .spacing(16)
            .push(widget::text::title3(fl!("welcome-title")))
            .push(widget::text::body(fl!("welcome-body")))
            .push(
                widget::button::suggested(fl!("pick-button"))
                    .on_press(Message::PickPressed),
            );

        widget::container(body)
            .center_x(cosmic::iced::Length::Fill)
            .into()
    }

    fn subscription(&self) -> Subscription<Message> {
        self.core()
            .watch_config::<Config>(Self::APP_ID)
            .map(|update| Message::UpdateConfig(update.config))
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::PickPressed => {
                // M1: actually fire the overlay and stash the result.
                // M0: just log so the wiring is observable.
                eprintln!("pick pressed (overlay wiring lands in M1)");
            }
            Message::UpdateConfig(c) => {
                self.config = c;
            }
        }
        Task::none()
    }
}
