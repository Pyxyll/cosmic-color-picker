//! libcosmic Application: the GUI window.
//!
//! Layout: a Cosmic-style sidebar nav (Picker / Settings / About) on the
//! left, page content on the right. The Picker page is the main view —
//! hero swatch + format readouts + history. Settings has the shortcut
//! binding and the autostart toggle. About is the standard libcosmic
//! about widget.

use crate::autostart;
use crate::color::PickedColor;
use crate::config::Config;
use crate::fl;
use crate::ipc;
use crate::shortcut;
use cosmic::Application;
use cosmic::app::{Core, Task};
use cosmic::cosmic_config::{self, CosmicConfigEntry};
use cosmic::iced::event;
use cosmic::iced::keyboard::{self, Key, key::Named};
use cosmic::iced::{Length, Subscription};
use std::time::{Duration, Instant};
use cosmic::prelude::*;
use cosmic::widget;
use cosmic::widget::nav_bar;

pub struct AppModel {
    core: Core,
    config: Config,
    /// Most recently picked color, displayed in the result view.
    picked: Option<PickedColor>,
    /// True while the overlay is running, used to debounce repeated clicks.
    picking: bool,
    /// Recent picks, newest first. Mirrored to `config.history` (persisted).
    history: Vec<PickedColor>,
    /// Sidebar navigation state.
    nav: nav_bar::Model,
    /// Cached "is autostart enabled?" so the toggle reflects on-disk truth.
    autostart_enabled: bool,
    /// Currently-bound shortcut, displayed on the Settings page button.
    shortcut_current: Option<String>,
    /// True while the user is in "press a combo" mode and we should listen
    /// to keyboard events.
    capturing_shortcut: bool,
    /// Feedback from the last shortcut save: `Ok(human)` on success,
    /// `Err(reason)` on parse / write failure, `None` while idle.
    shortcut_status: Option<Result<String, String>>,
    /// Cached About data so the widget reference stays stable across views.
    about: widget::about::About,
    /// Most recently copied value + when. Used to flash the copy icon to a
    /// check mark for a brief window after a click. `None` once the
    /// feedback has been cleared.
    last_copied: Option<(String, Instant)>,
}

const COPY_FEEDBACK_MS: u64 = 1500;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Page {
    Picker,
    Settings,
    About,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Hex,
    Rgb,
    Hsl,
    Hsv,
    Oklch,
}

const HISTORY_LIMIT: usize = 16;

#[derive(Debug, Clone)]
pub enum Message {
    PickPressed,
    PickResult(Option<String>),
    Copy(String),
    /// Fires ~1.5s after a Copy to revert the checkmark feedback.
    ClearCopyFeedback,
    SelectHistory(usize),
    ClearHistory,
    UpdateConfig(Config),
    ToggleAutostart(bool),
    ToggleFormat(Format, bool),
    /// Click on the shortcut button — start listening for the next combo.
    BeginCaptureShortcut,
    /// Either a real keypress while capturing, or Esc to cancel.
    CaptureShortcut(Key, keyboard::Modifiers),
    OpenUrl(String),
}

impl cosmic::Application for AppModel {
    type Executor = cosmic::executor::Default;
    type Flags = ();
    type Message = Message;
    const APP_ID: &'static str = "com.pyxyll.CosmicColorPicker";

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(core: Core, _flags: Self::Flags) -> (Self, Task<Message>) {
        let config = cosmic_config::Config::new(Self::APP_ID, Config::VERSION)
            .map(|ctx| match Config::get_entry(&ctx) {
                Ok(c) => c,
                Err((_e, c)) => c,
            })
            .unwrap_or_default();

        let history = parse_history(&config.history);
        let picked = history.first().copied();

        let mut nav = nav_bar::Model::default();
        nav.insert()
            .text(fl!("nav-picker"))
            .icon(widget::icon::from_name("color-select-symbolic"))
            .data::<Page>(Page::Picker)
            .activate();
        nav.insert()
            .text(fl!("nav-settings"))
            .icon(widget::icon::from_name("preferences-system-symbolic"))
            .data::<Page>(Page::Settings);
        nav.insert()
            .text(fl!("nav-about"))
            .icon(widget::icon::from_name("help-about-symbolic"))
            .data::<Page>(Page::About);

        let about = widget::about::About::default()
            .name("Color Picker")
            .icon(widget::icon::from_name("color-select-symbolic"))
            .version(env!("CARGO_PKG_VERSION"))
            .author("pyxyll")
            .license("MIT")
            .comments("Native Wayland color picker for COSMIC")
            .links([(
                "Source",
                "https://github.com/Pyxyll/cosmic-color-picker",
            )]);

        let app = AppModel {
            core,
            config,
            picked,
            picking: false,
            history,
            nav,
            autostart_enabled: autostart::is_enabled(),
            shortcut_current: shortcut::current_binding(),
            capturing_shortcut: false,
            shortcut_status: None,
            about,
            last_copied: None,
        };

        (app, Task::none())
    }

    fn nav_model(&self) -> Option<&nav_bar::Model> {
        Some(&self.nav)
    }

    fn on_nav_select(&mut self, id: nav_bar::Id) -> Task<Message> {
        self.nav.activate(id);
        // Refresh page-specific cached state on entry — covers external edits
        // to the autostart file or shortcut config since the GUI was opened.
        self.autostart_enabled = autostart::is_enabled();
        self.shortcut_current = shortcut::current_binding();
        // Leaving the Settings page mid-capture should cancel cleanly.
        self.capturing_shortcut = false;
        Task::none()
    }

    fn header_start(&self) -> Vec<Element<'_, Message>> {
        vec![widget::text::heading(fl!("app-title")).into()]
    }

    fn header_end(&self) -> Vec<Element<'_, Message>> {
        // Window controls (close/min/max) are provided by the compositor —
        // we don't add anything else here. Hide-from-the-header was a
        // workaround from the single-binary era and is unneeded now that
        // the daemon owns its own lifecycle.
        Vec::new()
    }

    fn view(&self) -> Element<'_, Message> {
        let page = self.nav.active_data::<Page>().copied().unwrap_or(Page::Picker);
        let body = match page {
            Page::Picker => self.picker_page(),
            Page::Settings => self.settings_page(),
            Page::About => widget::about(&self.about, |url| {
                Message::OpenUrl(url.to_string())
            }),
        };

        widget::container(widget::scrollable(
            widget::container(body).padding([16, 24, 24, 24]).max_width(640),
        ))
        .center_x(Length::Fill)
        .into()
    }

    fn subscription(&self) -> Subscription<Message> {
        let config_sub = self
            .core()
            .watch_config::<Config>(Self::APP_ID)
            .map(|update| Message::UpdateConfig(update.config));

        if self.capturing_shortcut {
            let capture = event::listen_with(|e, _status, _window| match e {
                event::Event::Keyboard(keyboard::Event::KeyPressed {
                    key, modifiers, ..
                }) => Some(Message::CaptureShortcut(key, modifiers)),
                _ => None,
            });
            Subscription::batch([config_sub, capture])
        } else {
            config_sub
        }
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
                        if let Some(result) = ipc::request_pick().await {
                            return result;
                        }
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
                self.last_copied = Some((text.clone(), Instant::now()));
                let copy = cosmic::iced::clipboard::write::<cosmic::Action<Message>>(text);
                let clear = Task::perform(
                    async {
                        tokio::time::sleep(Duration::from_millis(COPY_FEEDBACK_MS)).await
                    },
                    |_| cosmic::Action::App(Message::ClearCopyFeedback),
                );
                return copy.chain(clear);
            }
            Message::ClearCopyFeedback => {
                // Only clear if the most-recent copy is now stale; ignore
                // strays from earlier rapid clicks (each Copy schedules its
                // own clear, but a fresh click moves the goalposts).
                if let Some((_, t)) = self.last_copied
                    && t.elapsed() >= Duration::from_millis(COPY_FEEDBACK_MS)
                {
                    self.last_copied = None;
                }
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
                self.history = parse_history(&self.config.history);
                if let Some(top) = self.history.first().copied()
                    && Some(top) != self.picked
                {
                    self.picked = Some(top);
                }
            }
            Message::ToggleFormat(format, on) => {
                let app_id = <Self as cosmic::Application>::APP_ID;
                if let Ok(ctx) = cosmic_config::Config::new(app_id, Config::VERSION) {
                    let mut new_config = self.config.clone();
                    match format {
                        Format::Hex => new_config.format_hex = on,
                        Format::Rgb => new_config.format_rgb = on,
                        Format::Hsl => new_config.format_hsl = on,
                        Format::Hsv => new_config.format_hsv = on,
                        Format::Oklch => new_config.format_oklch = on,
                    }
                    let _ = new_config.write_entry(&ctx);
                    self.config = new_config;
                }
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
            Message::BeginCaptureShortcut => {
                self.capturing_shortcut = true;
                self.shortcut_status = None;
                // Temp-unbind so the user's current combo doesn't fire the
                // picker while they're trying to re-set it. We restore on
                // cancel; on a real save the new binding overwrites this.
                if let Err(e) = shortcut::clear() {
                    eprintln!("color picker: temp-unbind failed: {e}");
                }
            }
            Message::CaptureShortcut(key, modifiers) => {
                if !self.capturing_shortcut {
                    return Task::none();
                }
                // Modifier keys on their own don't complete a binding —
                // wait for an actual key while the user holds them.
                if is_modifier_key(&key) {
                    return Task::none();
                }
                // Esc with no modifiers cancels the capture and restores
                // whatever we cleared on entry.
                if matches!(&key, Key::Named(Named::Escape)) && modifiers.is_empty() {
                    self.capturing_shortcut = false;
                    if let Some(prev) = self.shortcut_current.clone()
                        && let Err(e) = shortcut::set_binding(&prev)
                    {
                        eprintln!("color picker: restore previous binding failed: {e}");
                    }
                    return Task::none();
                }
                self.capturing_shortcut = false;
                let combo = format_combo(modifiers, &key);
                if combo.is_empty() {
                    self.shortcut_status = Some(Err("Unsupported key".to_string()));
                    // Restore the binding we cleared so we're not left in a
                    // half-applied state.
                    if let Some(prev) = self.shortcut_current.clone() {
                        let _ = shortcut::set_binding(&prev);
                    }
                    return Task::none();
                }
                self.shortcut_status = Some(match shortcut::set_binding(&combo) {
                    Ok(()) => {
                        self.shortcut_current = Some(combo.clone());
                        Ok(combo)
                    }
                    Err(e) => Err(e),
                });
            }
            Message::OpenUrl(url) => {
                let _ = std::process::Command::new("xdg-open").arg(url).spawn();
            }
        }
        Task::none()
    }
}

impl AppModel {
    fn save_history(&self) {
        if let Ok(ctx) = cosmic_config::Config::new(Self::APP_ID, Config::VERSION) {
            let mut new_config = self.config.clone();
            new_config.history = self.history.iter().map(|p| p.hex()).collect();
            let _ = new_config.write_entry(&ctx);
        }
    }

    fn picker_page(&self) -> Element<'_, Message> {
        // Icon-only Pick button — text label felt heavy, the color-select
        // glyph reads at-a-glance and matches the panel applet's icon.
        let pick_button = widget::button::icon(
            widget::icon::from_name("color-select-symbolic"),
        )
        .large()
        .on_press_maybe((!self.picking).then_some(Message::PickPressed));

        let header = widget::Row::new()
            .align_y(cosmic::iced::Alignment::Center)
            .push(widget::Space::new().width(Length::Fill))
            .push(pick_button);

        let body = match &self.picked {
            None => self.welcome_view(),
            Some(p) => self.result_view(p),
        };

        widget::Column::new()
            .spacing(16)
            .push(header)
            .push(body)
            .into()
    }

    fn settings_page(&self) -> Element<'_, Message> {
        // While idle: a button with the current binding (click to record).
        // While capturing: a labelled "listening" indicator instead of a
        // button so the longer prompt text isn't constrained to the button
        // width and overflowing its container.
        let trailing: Element<'_, Message> = if self.capturing_shortcut {
            widget::container(widget::text::body(fl!("shortcut-listening")))
                .padding([4, 12])
                .into()
        } else {
            let label = self
                .shortcut_current
                .clone()
                .unwrap_or_else(|| fl!("shortcut-unset"));
            widget::button::standard(label)
                .on_press(Message::BeginCaptureShortcut)
                .into()
        };

        let mut shortcut_col = widget::Column::new()
            .spacing(6)
            .push(widget::settings::item(fl!("shortcut-label"), trailing))
            .push(widget::text::caption(fl!("shortcut-hint")).width(Length::Fill));

        if let Some(status) = &self.shortcut_status {
            let line = match status {
                Ok(combo) => widget::text::caption(format!("✓  {combo}")),
                Err(e) => widget::text::caption(format!("✗  {e}")),
            };
            shortcut_col = shortcut_col.push(line);
        }

        let shortcut_section = widget::settings::section()
            .title(fl!("settings-shortcut"))
            .add(shortcut_col);

        let formats_section = widget::settings::section()
            .title(fl!("settings-formats"))
            .add(format_toggle_row("HEX", Format::Hex, self.config.format_hex))
            .add(format_toggle_row("RGB", Format::Rgb, self.config.format_rgb))
            .add(format_toggle_row("HSL", Format::Hsl, self.config.format_hsl))
            .add(format_toggle_row("HSV", Format::Hsv, self.config.format_hsv))
            .add(format_toggle_row("OKLCH", Format::Oklch, self.config.format_oklch));

        let autostart_section = widget::settings::section()
            .title(fl!("settings-startup"))
            .add(widget::settings::item(
                fl!("settings-autostart"),
                widget::toggler(self.autostart_enabled).on_toggle(Message::ToggleAutostart),
            ))
            .add(widget::text::caption(fl!("settings-autostart-hint")));

        widget::Column::new()
            .spacing(16)
            .push(shortcut_section)
            .push(formats_section)
            .push(autostart_section)
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

        let icon_name = if self.is_recently_copied(&p.hex()) {
            "object-select-symbolic"
        } else {
            "edit-copy-symbolic"
        };
        let copy_hex = widget::button::icon(widget::icon::from_name(icon_name))
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
        let mut section = widget::settings::section();
        if self.config.format_hex {
            let v = p.hex();
            let copied = self.is_recently_copied(&v);
            section = section.add(format_item(&fl!("format-hex"), v, copied));
        }
        if self.config.format_rgb {
            let v = p.rgb_str();
            let copied = self.is_recently_copied(&v);
            section = section.add(format_item(&fl!("format-rgb"), v, copied));
        }
        if self.config.format_hsl {
            let v = p.hsl_str();
            let copied = self.is_recently_copied(&v);
            section = section.add(format_item(&fl!("format-hsl"), v, copied));
        }
        if self.config.format_hsv {
            let v = p.hsv_str();
            let copied = self.is_recently_copied(&v);
            section = section.add(format_item(&fl!("format-hsv"), v, copied));
        }
        if self.config.format_oklch {
            let v = p.oklch_str();
            let copied = self.is_recently_copied(&v);
            section = section.add(format_item(&fl!("format-oklch"), v, copied));
        }
        section.into()
    }

    fn is_recently_copied(&self, value: &str) -> bool {
        match &self.last_copied {
            Some((s, t)) => {
                s == value && t.elapsed() < Duration::from_millis(COPY_FEEDBACK_MS)
            }
            None => false,
        }
    }

    fn history_card(&self) -> Element<'_, Message> {
        let mut strip = widget::Row::new().spacing(8);
        for (i, c) in self.history.iter().enumerate() {
            strip = strip.push(self.history_chip(i, c.rgb));
        }
        // Wrap in a horizontal scroller so a long history doesn't overflow
        // the popup width. The scrollbar appears on demand.
        // Bottom padding on the inner strip so the scrollbar sits below
        // the chips instead of overlapping them. Path through iced because
        // cosmic::widget only re-exports the scrollable constructor.
        let strip_padded = widget::container(strip).padding([0, 0, 12, 0]);
        let scrollable_strip = widget::scrollable(strip_padded).direction(
            cosmic::iced::widget::scrollable::Direction::Horizontal(
                cosmic::iced::widget::scrollable::Scrollbar::new(),
            ),
        );

        let header = widget::Row::new()
            .align_y(cosmic::iced::Alignment::Center)
            .push(widget::text::heading(fl!("history-title")).width(Length::Fill))
            .push(
                widget::button::link(fl!("history-clear"))
                    .on_press(Message::ClearHistory),
            );
        widget::container(
            widget::Column::new()
                .spacing(12)
                .push(header)
                .push(scrollable_strip),
        )
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

fn format_toggle_row<'a>(label: &str, kind: Format, on: bool) -> Element<'a, Message> {
    widget::settings::item(
        label.to_string(),
        widget::toggler(on).on_toggle(move |v| Message::ToggleFormat(kind, v)),
    )
    .into()
}

fn parse_history(raw: &[String]) -> Vec<PickedColor> {
    raw.iter()
        .filter_map(|s| PickedColor::from_hex(s))
        .collect()
}

fn is_modifier_key(key: &Key) -> bool {
    matches!(
        key,
        Key::Named(
            Named::Shift
                | Named::Control
                | Named::Alt
                | Named::Super
                | Named::Meta
                | Named::AltGraph
                | Named::CapsLock
                | Named::NumLock
                | Named::ScrollLock
                | Named::Symbol
        )
    )
}

/// Format an iced (modifiers, key) pair into the human + Cosmic-config
/// form: `"Super+Shift+C"`. Returns empty string for keys we can't map
/// (e.g. dead keys, unidentified).
fn format_combo(mods: keyboard::Modifiers, key: &Key) -> String {
    let mut parts: Vec<String> = Vec::new();
    if mods.logo() {
        parts.push("Super".into());
    }
    if mods.control() {
        parts.push("Ctrl".into());
    }
    if mods.alt() {
        parts.push("Alt".into());
    }
    if mods.shift() {
        parts.push("Shift".into());
    }
    let key_str = match key {
        // iced delivers Space as Character(" "), not a Named variant.
        Key::Character(c) if c.as_str() == " " => "space".to_string(),
        Key::Character(c) => c.to_uppercase(),
        Key::Named(n) => match named_key_str(*n) {
            Some(s) => s.to_string(),
            None => return String::new(),
        },
        Key::Unidentified => return String::new(),
    };
    parts.push(key_str);
    parts.join("+")
}

/// Map iced's `Named` enum to the names Cosmic accepts in its shortcut
/// config. Anything not handled returns None which the caller treats as
/// "unsupported key".
fn named_key_str(n: Named) -> Option<&'static str> {
    Some(match n {
        Named::ArrowDown => "Down",
        Named::ArrowUp => "Up",
        Named::ArrowLeft => "Left",
        Named::ArrowRight => "Right",
        Named::Enter => "Return",
        Named::Escape => "Escape",
        Named::Tab => "Tab",
        Named::Backspace => "Backspace",
        Named::Delete => "Delete",
        Named::Insert => "Insert",
        Named::Home => "Home",
        Named::End => "End",
        Named::PageUp => "PageUp",
        Named::PageDown => "PageDown",
        Named::F1 => "F1",
        Named::F2 => "F2",
        Named::F3 => "F3",
        Named::F4 => "F4",
        Named::F5 => "F5",
        Named::F6 => "F6",
        Named::F7 => "F7",
        Named::F8 => "F8",
        Named::F9 => "F9",
        Named::F10 => "F10",
        Named::F11 => "F11",
        Named::F12 => "F12",
        Named::PrintScreen => "Print",
        _ => return None,
    })
}

/// A settings-list row: label on the left, monospace value, copy icon button.
/// `copied=true` swaps the copy icon for a checkmark to confirm the click.
fn format_item<'a>(label: &str, value: String, copied: bool) -> Element<'a, Message> {
    let value_for_copy = value.clone();
    let icon_name = if copied {
        "object-select-symbolic"
    } else {
        "edit-copy-symbolic"
    };
    let trailing = widget::Row::new()
        .spacing(12)
        .align_y(cosmic::iced::Alignment::Center)
        .push(widget::text::monotext(value))
        .push(
            widget::button::icon(widget::icon::from_name(icon_name))
                .extra_small()
                .on_press(Message::Copy(value_for_copy)),
        );
    widget::settings::item(label.to_string(), trailing).into()
}
