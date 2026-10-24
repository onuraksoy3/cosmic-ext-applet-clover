// SPDX-License-Identifier: MIT

const CORNER_DELAY_LABELS: [&str; 4] = ["100 ms", "200 ms", "350 ms", "500 ms"];

use crate::config::{Config, CORNER_ACTIONS, CORNER_LABELS, HOTKEYS, HOTKEY_LABELS, IDLE_LABELS, IDLE_SECS};
use crate::fl;
use crate::integrations::{self, Checks, Hardware, Touchpad};
use crate::keyd;
use cosmic::cosmic_config::{self, CosmicConfigEntry};
use cosmic::iced::platform_specific::shell::wayland::commands::popup::{destroy_popup, get_popup};
use cosmic::iced::{Length, Limits, Subscription, window::Id};
use cosmic::prelude::*;
use cosmic::widget::{self, segmented_button};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tab {
    Keys,
    Modifiers,
    Trackpad,
    Keyboard,
    Corners,
    Look,
    Apps,
    Settings,
}

impl Tab {
    const ALL: [Tab; 8] = [Tab::Keys, Tab::Modifiers, Tab::Trackpad, Tab::Keyboard, Tab::Corners, Tab::Look, Tab::Apps, Tab::Settings];

    fn icon(self) -> &'static str {
        match self {
            Tab::Keys => "input-keyboard-symbolic",
            Tab::Modifiers => "preferences-desktop-keyboard-shortcuts-symbolic",
            Tab::Trackpad => "input-touchpad-symbolic",
            Tab::Keyboard => "keyboard-brightness-symbolic",
            Tab::Corners => "view-fullscreen-symbolic",
            Tab::Look => "video-display-symbolic",
            Tab::Apps => "applications-games-symbolic",
            Tab::Settings => "emblem-system-symbolic",
        }
    }

    /// macOS-style tile colour behind the sidebar icon (r, g, b).
    fn tint(self) -> (f32, f32, f32) {
        match self {
            Tab::Keys => (0.20, 0.47, 0.96),
            Tab::Modifiers => (0.55, 0.36, 0.86),
            Tab::Trackpad => (0.49, 0.49, 0.53),
            Tab::Keyboard => (0.95, 0.60, 0.17),
            Tab::Corners => (0.19, 0.70, 0.72),
            Tab::Look => (0.91, 0.33, 0.56),
            Tab::Apps => (0.26, 0.72, 0.35),
            Tab::Settings => (0.49, 0.49, 0.53),
        }
    }

    fn title(self) -> String {
        match self {
            Tab::Keys => fl!("tab-keys"),
            Tab::Modifiers => fl!("tab-modifiers"),
            Tab::Trackpad => fl!("tab-trackpad"),
            Tab::Keyboard => fl!("tab-keyboard"),
            Tab::Corners => fl!("tab-corners"),
            Tab::Look => fl!("tab-look"),
            Tab::Apps => fl!("tab-apps"),
            Tab::Settings => fl!("tab-settings"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Toggle {
    MacKeys,
    TapSuper,
    CmdSpaceLauncher,
    RightCmd,
    CtrlSuper,
    CtrlSpaces,
    TextEditing,
    Navigation,
    Screenshots,
    Fkeys,
    Terminal,
    DockHotkey,
    Emoji,
    KbdBacklightKeys,
    AutoBrightness,
    AutoKbd,
    KbdIdleDim,
    GamesDgpu,
    InterFont,
    DockHover,
    Corners,
    FnStandard,
    Notify,
    NotifyHotkey,
    NaturalScroll,
    TapToClick,
    Clickfinger,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlatpakState {
    Unknown,
    Missing,
    Installing,
    Installed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preset {
    MacPurist,
    Hybrid,
    Minimal,
}

#[derive(Default)]
pub struct AppModel {
    core: cosmic::Core,
    popup: Option<Id>,
    nav: segmented_button::SingleSelectModel,
    config: Config,
    config_handler: Option<cosmic_config::Config>,
    touchpad: Touchpad,
    scroll_factor: f32,
    checks: Checks,
    fixing: bool,
    fn_standard: Option<bool>,
    min_brightness: f32,
    apps_input: String,
    light_sensor: bool,
    hw: Hardware,
    swayidle: bool,
    inter_installed: bool,
    dock_installed: bool,
    dock_hover: bool,
    corner_cmds: [String; 4],
    corner_size: f32,
    search: String,
    pointer_speed: f32,
    kbd_pct: f32,
    pause_secs: f32,
    hover_delay: f32,
    corner_delay: f32,
    flatpaks: HashMap<&'static str, FlatpakState>,
    /// Most recent problem, if any.
    error: Option<String>,
    /// Most recent one-line info ("Exported to …"), shown instead of the OK line.
    info: Option<String>,
}

#[derive(Debug, Clone)]
pub enum Message {
    TogglePopup,
    PopupClosed(Id),
    Tab(segmented_button::Entity),
    UpdateConfig(Config),
    Set(Toggle, bool),
    Hotkey(usize),
    ScrollFactor(f32),
    ScrollFactorCommit,
    MacTrackpad,
    ApplyPreset(Preset),
    Export,
    Import,
    FixIt,
    FixDone(Result<(), String>),
    InstallHelpers,
    HelpersDone(Result<(), String>),
    Install(&'static str),
    Apt(&'static str),
    AptDone(Result<(), String>),
    FnDone(Result<(), String>),
    MinBrightness(f32),
    MinBrightnessCommit,
    IdleSecs(usize),
    AppsInput(String),
    AppsApply,
    InstallDock,
    ToggleAdvanced,
    Search(String),
    PointerSpeed(f32),
    PointerSpeedCommit,
    KbdPct(f32),
    KbdPctCommit,
    PauseSecs(f32),
    PauseSecsCommit,
    HoverDelay(f32),
    HoverDelayCommit,
    CornerDelaySlide(f32),
    CornerDelayCommit,
    CornerAction(usize, usize),
    CornerCmd(usize, String),
    CornerCmdApply(usize),
    CornerSize(f32),
    CornerSizeCommit,
    CornerDelay(usize),
    InstallDone(&'static str, Result<(), String>),
    Reapply,
}

impl AppModel {
    /// Push the current config everywhere it needs to go. Idempotent.
    fn apply_all(&mut self) {
        let mut errors: Vec<String> = Vec::new();
        if let Err(e) = keyd::apply(&self.config) {
            errors.push(e);
        }
        if let Err(e) = integrations::sync_cosmic_shortcuts(&self.config) {
            errors.push(e);
        }
        if let Err(e) = integrations::set_terminal(self.config.terminal) {
            errors.push(e);
        }
        if let Err(e) = integrations::sync_sensord(&self.config) {
            errors.push(e);
        }
        if let Err(e) = integrations::sync_gpu_rules(&self.config) {
            errors.push(e);
        }
        if let Err(e) = integrations::sync_corners(&self.config) {
            errors.push(e);
        }
        self.error = if errors.is_empty() { None } else { Some(errors.join(" · ")) };
    }

    fn save_all(&mut self) {
        if let Some(handler) = self.config_handler.as_ref() {
            let _ = self.config.write_entry(handler);
        }
    }

    fn refresh(&mut self) {
        self.touchpad = integrations::read_touchpad();
        self.scroll_factor = self.touchpad.scroll_factor as f32;
        self.checks = integrations::run_checks();
        self.fn_standard = integrations::read_fn_standard();
        self.min_brightness = self.config.min_brightness as f32;
        self.hw = integrations::detect_hardware();
        self.light_sensor = self.hw.light_sensor;
        self.swayidle = integrations::swayidle_present();
        self.inter_installed = integrations::inter_font_installed();
        self.dock_installed = integrations::clover_dock_installed();
        self.dock_hover = integrations::hover_preview_get();
        self.corner_size = self.config.corner_size as f32;
        self.pointer_speed = self.touchpad.speed as f32;
        self.kbd_pct = self.config.kbd_max_pct as f32;
        self.pause_secs = self.config.brightness_pause_secs as f32;
        self.hover_delay = integrations::hover_delay_get() as f32;
        self.corner_delay = self.config.corner_delay as f32;
        for (i, v) in self.corner_values().into_iter().enumerate() {
            if let Some(cmd) = v.strip_prefix("cmd:") {
                if self.corner_cmds[i].is_empty() {
                    self.corner_cmds[i] = cmd.to_string();
                }
            }
        }
        if self.apps_input.is_empty() {
            self.apps_input = self.config.dgpu_apps.clone();
        }
        for id in [integrations::SMILE, integrations::CLIPBOARD] {
            let state = if integrations::flatpak_installed(id) { FlatpakState::Installed } else { FlatpakState::Missing };
            let entry = self.flatpaks.entry(id).or_insert(FlatpakState::Unknown);
            if *entry != FlatpakState::Installing {
                *entry = state;
            }
        }
    }

    fn set_field(&mut self, toggle: Toggle, value: bool) {
        let c = &mut self.config;
        match toggle {
            Toggle::MacKeys => c.mac_keys = value,
            Toggle::TapSuper => c.tap_super = value,
            Toggle::CmdSpaceLauncher => c.cmd_space_launcher = value,
            Toggle::RightCmd => c.right_cmd = value,
            Toggle::CtrlSuper => c.ctrl_super = value,
            Toggle::CtrlSpaces => c.ctrl_spaces = value,
            Toggle::TextEditing => c.text_editing = value,
            Toggle::Navigation => c.navigation = value,
            Toggle::Screenshots => c.screenshots = value,
            Toggle::Fkeys => c.fkeys = value,
            Toggle::Terminal => c.terminal = value,
            Toggle::DockHotkey => c.dock_hotkey = value,
            Toggle::Emoji => c.emoji = value,
            Toggle::KbdBacklightKeys => c.kbd_backlight_keys = value,
            Toggle::AutoBrightness => c.auto_brightness = value,
            Toggle::AutoKbd => c.auto_kbd = value,
            Toggle::KbdIdleDim => c.kbd_idle_dim = value,
            Toggle::GamesDgpu => c.games_dgpu = value,
            Toggle::InterFont => c.inter_font = value,
            Toggle::DockHover => {}
            Toggle::Corners => c.corners = value,
            Toggle::FnStandard => {}
            Toggle::Notify => c.notify = value,
            Toggle::NotifyHotkey => c.notify_hotkey = value,
            Toggle::NaturalScroll | Toggle::TapToClick | Toggle::Clickfinger => {}
        }
    }

    fn gpu_name(&self) -> String {
        self.hw.dgpu.clone().unwrap_or_else(|| fl!("dgpu-generic"))
    }

    fn label(&self, toggle: Toggle) -> String {
        match toggle {
            Toggle::MacKeys => fl!("mac-keys"),
            Toggle::TapSuper => fl!("tap-super"),
            Toggle::CmdSpaceLauncher => fl!("cmd-space"),
            Toggle::RightCmd => fl!("right-cmd"),
            Toggle::CtrlSuper => fl!("ctrl-super"),
            Toggle::CtrlSpaces => fl!("ctrl-spaces"),
            Toggle::TextEditing => fl!("text-editing"),
            Toggle::Navigation => fl!("navigation"),
            Toggle::Screenshots => fl!("screenshots"),
            Toggle::Fkeys => fl!("fkeys"),
            Toggle::Terminal => fl!("terminal"),
            Toggle::DockHotkey => fl!("dock-hotkey"),
            Toggle::Emoji => fl!("emoji"),
            Toggle::KbdBacklightKeys => fl!("kbd-backlight"),
            Toggle::AutoBrightness => fl!("auto-brightness"),
            Toggle::AutoKbd => fl!("auto-kbd"),
            Toggle::KbdIdleDim => fl!("kbd-idle-dim"),
            Toggle::GamesDgpu => fl!("games-dgpu", gpu = self.gpu_name()),
            Toggle::InterFont => fl!("inter-font"),
            Toggle::DockHover => fl!("dock-hover"),
            Toggle::Corners => fl!("corners"),
            Toggle::FnStandard => fl!("fn-standard"),
            Toggle::Notify => fl!("notify"),
            Toggle::NotifyHotkey => fl!("notify-hotkey"),
            Toggle::NaturalScroll => fl!("natural-scroll"),
            Toggle::TapToClick => fl!("tap-to-click"),
            Toggle::Clickfinger => fl!("clickfinger"),
        }
    }

    fn hint(&self, toggle: Toggle) -> String {
        match toggle {
            Toggle::MacKeys => fl!("mac-keys-hint"),
            Toggle::TapSuper => fl!("tap-super-hint"),
            Toggle::CmdSpaceLauncher => fl!("cmd-space-hint"),
            Toggle::RightCmd => fl!("right-cmd-hint"),
            Toggle::CtrlSuper => fl!("ctrl-super-hint"),
            Toggle::CtrlSpaces => fl!("ctrl-spaces-hint"),
            Toggle::TextEditing => fl!("text-editing-hint"),
            Toggle::Navigation => fl!("navigation-hint"),
            Toggle::Screenshots => fl!("screenshots-hint"),
            Toggle::Fkeys => fl!("fkeys-hint"),
            Toggle::Terminal => fl!("terminal-hint"),
            Toggle::DockHotkey => fl!("dock-hotkey-hint"),
            Toggle::Emoji => fl!("emoji-hint"),
            Toggle::KbdBacklightKeys => fl!("kbd-backlight-hint"),
            Toggle::AutoBrightness => fl!("auto-brightness-hint"),
            Toggle::AutoKbd => fl!("auto-kbd-hint"),
            Toggle::KbdIdleDim => fl!("kbd-idle-dim-hint"),
            Toggle::GamesDgpu => fl!("games-dgpu-hint", gpu = self.gpu_name()),
            Toggle::InterFont => fl!("inter-font-hint"),
            Toggle::DockHover => fl!("dock-hover-hint"),
            Toggle::Corners => fl!("corners-hint"),
            Toggle::FnStandard => fl!("fn-standard-hint"),
            Toggle::Clickfinger => fl!("clickfinger-hint"),
            _ => String::new(),
        }
    }

    /// macOS-sized switch (the default COSMIC toggler is a little large for dense settings rows).
    fn switch<'a>(value: bool, on: impl Fn(bool) -> Message + 'a) -> widget::Toggler<'a, Message> {
        widget::toggler(value).size(20.0).on_toggle(on)
    }

    /// Rows whose hint carries real information (which keys do what) keep it inline; the rest explain
    /// themselves in a tooltip so the list stays one line per setting.
    fn inline_hint(toggle: Toggle) -> bool {
        matches!(toggle, Toggle::TextEditing | Toggle::Navigation | Toggle::Screenshots | Toggle::Terminal | Toggle::Fkeys)
    }

    fn with_tip<'a>(content: impl Into<Element<'a, Message>>, tip: String) -> Element<'a, Message> {
        if tip.is_empty() {
            return content.into();
        }
        widget::tooltip(
            content,
            widget::container(widget::text::caption(tip)).max_width(320.0),
            widget::tooltip::Position::Bottom,
        )
        .into()
    }

    fn row<'a>(&self, toggle: Toggle, value: bool) -> Element<'a, Message> {
        let hint = self.hint(toggle);
        let mut item = widget::settings::item::builder(self.label(toggle));
        let mut tip = String::new();
        if !hint.is_empty() {
            if Self::inline_hint(toggle) {
                item = item.description(hint);
            } else {
                tip = hint;
            }
        }
        Self::with_tip(item.control(Self::switch(value, move |v| Message::Set(toggle, v))), tip)
    }

    /// A slider row: value shown in the description, released value committed.
    fn slider_row<'a>(
        title: String,
        shown: String,
        range: std::ops::RangeInclusive<f32>,
        step: f32,
        value: f32,
        on_change: fn(f32) -> Message,
        on_release: Message,
    ) -> Element<'a, Message> {
        let slider = widget::slider(range, value, on_change).step(step).on_release(on_release).width(Length::Fixed(160.0));
        Self::item(title, shown, slider)
    }

    /// A settings row with an arbitrary control and an optional description.
    fn item<'a>(title: String, hint: String, control: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
        let mut item = widget::settings::item::builder(title);
        if !hint.is_empty() {
            item = item.description(hint);
        }
        item.control(control).into()
    }

    fn note<'a>(text: String) -> Element<'a, Message> {
        widget::container(widget::text::caption(text)).padding([6, 12]).into()
    }

    /// A row whose control is "Install" until the Flatpak is present.
    fn flatpak_row<'a>(&self, id: &'static str, toggle: Toggle, value: bool) -> Element<'a, Message> {
        match self.flatpaks.get(id).copied().unwrap_or(FlatpakState::Unknown) {
            FlatpakState::Installed => self.row(toggle, value),
            FlatpakState::Installing => Self::item(self.label(toggle), self.hint(toggle), widget::text::body(fl!("installing"))),
            _ => Self::item(
                self.label(toggle),
                self.hint(toggle),
                widget::button::suggested(fl!("install")).on_press(Message::Install(id)),
            ),
        }
    }

    /// The clipboard applet opens only from its panel icon, so this row is
    /// Install-only; once present it just tells you where to add it.
    fn clipboard_row<'a>(&self) -> Element<'a, Message> {
        let state = self.flatpaks.get(integrations::CLIPBOARD).copied().unwrap_or(FlatpakState::Unknown);
        let control: Element<'a, Message> = match state {
            FlatpakState::Installed => widget::text::body(fl!("clipboard-installed")).into(),
            FlatpakState::Installing => widget::text::body(fl!("installing")).into(),
            _ => widget::button::suggested(fl!("install")).on_press(Message::Install(integrations::CLIPBOARD)).into(),
        };
        Self::item(fl!("clipboard"), fl!("clipboard-hint"), control)
    }

    /// zenity/wl-clipboard: Install when missing, a quiet "Installed" otherwise.
    fn helpers_row<'a>(&self) -> Element<'a, Message> {
        let control: Element<'a, Message> = if self.checks.helpers {
            widget::text::body(fl!("installed")).into()
        } else if self.fixing {
            widget::text::body(fl!("installing")).into()
        } else {
            widget::button::suggested(fl!("install")).on_press(Message::InstallHelpers).into()
        };
        Self::item(fl!("helpers"), fl!("helpers-hint"), control)
    }

    fn section<'a>(&self, title: String) -> widget::settings::Section<'a, Message> {
        widget::settings::section().header(widget::container(widget::text::caption_heading(title)).padding([6, 4, 0, 4]))
    }

    /// First card of a page: no header (the page title above it says what it is).
    fn lead<'a>(&self) -> widget::settings::Section<'a, Message> {
        widget::settings::section()
    }

    fn titled<'a>(&self, title: String, page: Element<'a, Message>) -> Element<'a, Message> {
        widget::column::with_capacity(2)
            .spacing(10)
            .push(widget::container(widget::text::title4(title)).padding([0, 4]))
            .push(page)
            .into()
    }

    fn page_keys<'a>(&self) -> Element<'a, Message> {
        let c = &self.config;
        let current = c.matches_preset();
        let pbtn = |label: String, key: &'static str, preset: Preset| -> Element<'a, Message> {
            if current == Some(key) {
                widget::button::suggested(label).on_press(Message::ApplyPreset(preset)).into()
            } else {
                widget::button::standard(label).on_press(Message::ApplyPreset(preset)).into()
            }
        };
        let presets = widget::row::with_capacity(3)
            .spacing(6)
            .push(pbtn(fl!("preset-minimal"), "minimal", Preset::Minimal))
            .push(pbtn(fl!("preset-hybrid"), "hybrid", Preset::Hybrid))
            .push(pbtn(fl!("preset-purist"), "purist", Preset::MacPurist));
        let main = self
            .lead()
            .add(self.row(Toggle::MacKeys, c.mac_keys))
            .add(Self::item(fl!("presets"), if current.is_some() { fl!("presets-hint") } else { fl!("presets-custom") }, presets));
        let shortcuts = self
            .section(fl!("sec-shortcuts"))
            .add(self.row(Toggle::TextEditing, c.text_editing))
            .add(self.row(Toggle::Navigation, c.navigation))
            .add(self.row(Toggle::Screenshots, c.screenshots))
            .add(self.row(Toggle::Fkeys, c.fkeys))
            .add(self.row(Toggle::Terminal, c.terminal))
            .add(self.flatpak_row(integrations::SMILE, Toggle::Emoji, c.emoji))
            .add(self.row(Toggle::DockHotkey, c.dock_hotkey))
            .add(self.helpers_row());
        widget::settings::view_column(vec![main.into(), shortcuts.into()]).into()
    }

    fn page_modifiers<'a>(&self) -> Element<'a, Message> {
        let c = &self.config;
        let sec = self
            .lead()
            .add(self.row(Toggle::TapSuper, c.tap_super))
            .add(self.row(Toggle::CmdSpaceLauncher, c.cmd_space_launcher))
            .add(self.row(Toggle::RightCmd, c.right_cmd))
            .add(self.row(Toggle::CtrlSuper, c.ctrl_super))
            .add(self.row(Toggle::CtrlSpaces, c.ctrl_spaces));
        widget::settings::view_column(vec![sec.into()]).into()
    }

    fn page_trackpad<'a>(&self) -> Element<'a, Message> {
        let t = self.touchpad;
        let slider = widget::slider(0.2..=3.0, self.scroll_factor, Message::ScrollFactor)
            .step(0.1_f32)
            .on_release(Message::ScrollFactorCommit)
            .width(Length::Fixed(150.0));
        let sec = self
            .lead()
            .add(Self::item(
                fl!("mac-trackpad"),
                fl!("mac-trackpad-hint"),
                widget::button::suggested(fl!("apply")).on_press(Message::MacTrackpad),
            ))
            .add(self.row(Toggle::NaturalScroll, t.natural_scroll))
            .add(self.row(Toggle::TapToClick, t.tap_to_click))
            .add(self.row(Toggle::Clickfinger, t.clickfinger))
            .add(Self::slider_row(fl!("pointer-speed"), String::new(), -1.0..=1.0, 0.1, self.pointer_speed, Message::PointerSpeed, Message::PointerSpeedCommit))
            .add(Self::item(fl!("scroll-speed"), String::new(), slider));
        widget::settings::view_column(vec![sec.into(), Self::note(fl!("trackpad-note"))]).into()
    }

    fn page_keyboard<'a>(&self) -> Element<'a, Message> {
        let c = &self.config;
        let mut sections: Vec<Element<'a, Message>> = Vec::new();
        if self.light_sensor {
            let mut auto = self.lead().add(self.row(Toggle::AutoBrightness, c.auto_brightness));
            if c.auto_brightness {
                let slider = widget::slider(5.0..=60.0, self.min_brightness, Message::MinBrightness)
                    .step(5.0_f32)
                    .on_release(Message::MinBrightnessCommit)
                    .width(Length::Fixed(150.0));
                auto = auto.add(Self::item(fl!("min-brightness"), format!("{}%", self.min_brightness as u32), slider));
            }
            if c.auto_brightness {
                auto = auto.add(Self::slider_row(fl!("pause-secs"), format!("{} s", self.pause_secs as u32), 30.0..=600.0, 30.0, self.pause_secs, Message::PauseSecs, Message::PauseSecsCommit));
            }
            if self.hw.kbd_backlight {
                auto = auto.add(self.row(Toggle::AutoKbd, c.auto_kbd));
                if c.auto_kbd {
                    auto = auto.add(Self::slider_row(fl!("kbd-level"), format!("{}%", self.kbd_pct as u32), 10.0..=100.0, 10.0, self.kbd_pct, Message::KbdPct, Message::KbdPctCommit));
                }
                if self.swayidle {
                    auto = auto.add(self.row(Toggle::KbdIdleDim, c.kbd_idle_dim));
                    if c.kbd_idle_dim {
                        let selected = IDLE_SECS.iter().position(|s| *s == c.kbd_idle_secs);
                        auto = auto.add(Self::item(fl!("idle-after"), String::new(), widget::dropdown(&IDLE_LABELS, selected, Message::IdleSecs)));
                    }
                } else {
                    auto = auto.add(Self::item(
                        self.label(Toggle::KbdIdleDim),
                        fl!("needs-swayidle"),
                        widget::button::suggested(fl!("install")).on_press(Message::Apt("swayidle")),
                    ));
                }
            }
            sections.push(auto.into());
        }
        let mut keys = self.section(fl!("sec-keyboard"));
        if self.hw.apple_keyboard {
            let fn_control: Element<'a, Message> = match self.fn_standard {
                Some(v) if !self.fixing => Self::switch(v, |x| Message::Set(Toggle::FnStandard, x)).into(),
                Some(_) => widget::text::body(fl!("applying")).into(),
                None => widget::text::body(fl!("unavailable")).into(),
            };
            keys = keys.add(Self::item(self.label(Toggle::FnStandard), self.hint(Toggle::FnStandard), fn_control));
        }
        if self.hw.kbd_backlight {
            keys = keys.add(self.row(Toggle::KbdBacklightKeys, c.kbd_backlight_keys));
        }
        keys = keys.add(self.clipboard_row());
        sections.push(keys.into());
        if !self.light_sensor {
            sections.push(Self::note(fl!("no-light-sensor")));
        }
        sections.push(Self::note(fl!("keyboard-note")));
        widget::settings::view_column(sections).into()
    }

    fn page_corners<'a>(&'a self) -> Element<'a, Message> {
        let c = &self.config;
        let mut corners = self.lead().add(self.row(Toggle::Corners, c.corners));
        if c.corners {
            for r in self.corner_rows() {
                corners = corners.add(r);
            }
        }
        let mut sections: Vec<Element<'a, Message>> = vec![corners.into()];
        if c.corners {
            let size = widget::slider(4.0..=40.0, self.corner_size, Message::CornerSize)
                .step(2.0_f32)
                .on_release(Message::CornerSizeCommit)
                .width(Length::Fixed(150.0));
            let tuning = self
                .section(fl!("sec-tuning"))
                .add(Self::item(fl!("corner-size"), format!("{} px", self.corner_size as u32), size))
                .add(Self::slider_row(fl!("corner-delay"), format!("{} ms", self.corner_delay as u32), 50.0..=800.0, 50.0, self.corner_delay, Message::CornerDelaySlide, Message::CornerDelayCommit));
            sections.push(tuning.into());
        }
        widget::settings::view_column(sections).into()
    }

    fn page_look<'a>(&self) -> Element<'a, Message> {
        let c = &self.config;
        let dock_control: Element<'a, Message> = if self.dock_installed {
            Self::switch(self.dock_hover, |v| Message::Set(Toggle::DockHover, v)).into()
        } else {
            widget::button::suggested(fl!("install")).on_press(Message::InstallDock).into()
        };
        let font_control: Element<'a, Message> = if self.inter_installed {
            Self::switch(c.inter_font, |v| Message::Set(Toggle::InterFont, v)).into()
        } else if self.fixing {
            widget::text::body(fl!("installing")).into()
        } else {
            widget::button::suggested(fl!("install")).on_press(Message::Apt("fonts-inter")).into()
        };
        let mut sec = self
            .lead()
            .add(Self::with_tip(Self::item(self.label(Toggle::DockHover), String::new(), dock_control), self.hint(Toggle::DockHover)));
        if self.dock_installed && self.dock_hover {
            sec = sec.add(Self::slider_row(fl!("hover-delay"), format!("{} ms", self.hover_delay as u32), 100.0..=800.0, 50.0, self.hover_delay, Message::HoverDelay, Message::HoverDelayCommit));
        }
        sec = sec.add(Self::with_tip(Self::item(self.label(Toggle::InterFont), String::new(), font_control), self.hint(Toggle::InterFont)));
        widget::settings::view_column(vec![sec.into()]).into()
    }

    fn page_apps<'a>(&'a self) -> Element<'a, Message> {
        let c = &self.config;
        if self.hw.dgpu.is_none() {
            return widget::settings::view_column(vec![Self::note(fl!("no-dgpu"))]).into();
        }
        let input = widget::text_input(fl!("apps-placeholder"), &self.apps_input)
            .on_input(Message::AppsInput)
            .width(Length::Fill);
        let apply = widget::button::standard(fl!("apply")).on_press(Message::AppsApply);
        let sec = self
            .lead()
            .add(self.row(Toggle::GamesDgpu, c.games_dgpu))
            .add(
                widget::column::with_capacity(3)
                    .spacing(6)
                    .push(widget::text::body(fl!("apps-named", gpu = self.gpu_name())))
                    .push(widget::text::caption(fl!("apps-named-hint")))
                    .push(widget::row::with_capacity(2).spacing(8).push(input).push(apply)),
            );
        widget::settings::view_column(vec![sec.into(), Self::note(fl!("apps-note"))]).into()
    }

    fn toggle_value(&self, t: Toggle) -> Option<bool> {
        let c = &self.config;
        Some(match t {
            Toggle::MacKeys => c.mac_keys,
            Toggle::TapSuper => c.tap_super,
            Toggle::CmdSpaceLauncher => c.cmd_space_launcher,
            Toggle::RightCmd => c.right_cmd,
            Toggle::CtrlSuper => c.ctrl_super,
            Toggle::CtrlSpaces => c.ctrl_spaces,
            Toggle::TextEditing => c.text_editing,
            Toggle::Navigation => c.navigation,
            Toggle::Screenshots => c.screenshots,
            Toggle::Fkeys => c.fkeys,
            Toggle::Terminal => c.terminal,
            Toggle::DockHotkey => c.dock_hotkey,
            Toggle::Emoji => c.emoji,
            Toggle::NaturalScroll => self.touchpad.natural_scroll,
            Toggle::TapToClick => self.touchpad.tap_to_click,
            Toggle::Clickfinger => self.touchpad.clickfinger,
            Toggle::KbdBacklightKeys => c.kbd_backlight_keys,
            Toggle::AutoBrightness => c.auto_brightness,
            Toggle::AutoKbd => c.auto_kbd,
            Toggle::KbdIdleDim => c.kbd_idle_dim,
            Toggle::GamesDgpu => c.games_dgpu,
            Toggle::InterFont => c.inter_font,
            Toggle::DockHover => self.dock_hover,
            Toggle::Corners => c.corners,
            Toggle::Notify => c.notify,
            Toggle::NotifyHotkey => c.notify_hotkey,
            Toggle::FnStandard => return None,
        })
    }

    const SEARCHABLE: [Toggle; 26] = [
        Toggle::MacKeys, Toggle::TapSuper, Toggle::CmdSpaceLauncher, Toggle::RightCmd, Toggle::CtrlSuper,
        Toggle::CtrlSpaces, Toggle::TextEditing, Toggle::Navigation, Toggle::Screenshots, Toggle::Fkeys,
        Toggle::Terminal, Toggle::DockHotkey, Toggle::Emoji, Toggle::NaturalScroll, Toggle::TapToClick,
        Toggle::Clickfinger, Toggle::KbdBacklightKeys, Toggle::AutoBrightness, Toggle::AutoKbd,
        Toggle::KbdIdleDim, Toggle::GamesDgpu, Toggle::InterFont, Toggle::DockHover, Toggle::Corners,
        Toggle::Notify, Toggle::NotifyHotkey,
    ];

    fn page_search<'a>(&self) -> Element<'a, Message> {
        let q = self.search.to_lowercase();
        let mut sec = self.section(fl!("search-results"));
        let mut n = 0;
        for t in Self::SEARCHABLE {
            let hit = self.label(t).to_lowercase().contains(&q) || self.hint(t).to_lowercase().contains(&q);
            if !hit {
                continue;
            }
            if let Some(v) = self.toggle_value(t) {
                sec = sec.add(self.row(t, v));
                n += 1;
            }
        }
        if n == 0 {
            return widget::settings::view_column(vec![Self::note(fl!("search-none"))]).into();
        }
        widget::settings::view_column(vec![sec.into()]).into()
    }

    fn corner_values(&self) -> [String; 4] {
        let c = &self.config;
        [c.corner_tl.clone(), c.corner_tr.clone(), c.corner_bl.clone(), c.corner_br.clone()]
    }

    fn set_corner_value(&mut self, i: usize, v: String) {
        match i {
            0 => self.config.corner_tl = v,
            1 => self.config.corner_tr = v,
            2 => self.config.corner_bl = v,
            _ => self.config.corner_br = v,
        }
    }

    fn corner_rows<'a>(&'a self) -> Vec<Element<'a, Message>> {
        let names = [fl!("corner-tl"), fl!("corner-tr"), fl!("corner-bl"), fl!("corner-br")];
        let values = self.corner_values();
        let mut rows: Vec<Element<'a, Message>> = Vec::new();
        for (i, name) in names.into_iter().enumerate() {
            let v = &values[i];
            let selected = if v.starts_with("cmd:") {
                Some(5)
            } else {
                CORNER_ACTIONS.iter().position(|a| a == v)
            };
            rows.push(Self::item(name, String::new(), widget::dropdown(&CORNER_LABELS, selected, move |ix| Message::CornerAction(i, ix))));
            if v.starts_with("cmd:") {
                let input = widget::text_input(fl!("corner-cmd-placeholder"), &self.corner_cmds[i])
                    .on_input(move |s| Message::CornerCmd(i, s))
                    .width(Length::Fill);
                rows.push(
                    widget::row::with_capacity(2)
                        .spacing(8)
                        .push(input)
                        .push(widget::button::standard(fl!("apply")).on_press(Message::CornerCmdApply(i)))
                        .into(),
                );
            }
        }
        rows
    }

    fn page_settings<'a>(&'a self) -> Element<'a, Message> {
        let c = &self.config;
        let selected = HOTKEYS.iter().position(|(id, _)| *id == c.hotkey);
        let io = widget::row::with_capacity(2)
            .spacing(8)
            .push(widget::button::standard(fl!("export")).on_press(Message::Export))
            .push(widget::button::standard(fl!("import")).on_press(Message::Import));
        let general = self
            .lead()
            .add(Self::item(fl!("hotkey"), fl!("hotkey-hint"), widget::dropdown(&HOTKEY_LABELS, selected, Message::Hotkey)))
            .add(self.row(Toggle::NotifyHotkey, c.notify_hotkey))
            .add(self.row(Toggle::Notify, c.notify));
        let maintenance = self
            .section(fl!("sec-maintenance"))
            .add(Self::item(fl!("settings-file"), fl!("settings-file-hint", path = integrations::export_path_display()), io))
            .add(Self::item(fl!("reapply"), String::new(), widget::button::standard("⟳").on_press(Message::Reapply)))
            .add(Self::item(fl!("fix-it"), fl!("fix-it-hint"), widget::button::standard(fl!("fix-it")).on_press(Message::FixIt)));
        widget::settings::view_column(vec![general.into(), maintenance.into()]).into()
    }

    fn page_first_run<'a>(&self) -> Element<'a, Message> {
        let ch = self.checks;
        let line = |ok: bool, text: String| {
            widget::text::body(format!("{}  {}", if ok { "✓" } else { "✗" }, text))
        };
        let mut col = widget::column::with_capacity(8)
            .spacing(6)
            .push(widget::text::heading(fl!("first-run-title")))
            .push(widget::text::caption(fl!("first-run-hint")))
            .push(line(ch.keyd_installed, fl!("check-keyd-installed")))
            .push(line(ch.keyd_running, fl!("check-keyd-running")))
            .push(line(ch.keyd_group, fl!("check-keyd-group")))
            .push(line(ch.keyd_layers, fl!("check-keyd-layers")));
        if !ch.keyd_installed {
            col = col.push(widget::text::caption(fl!("keyd-install-hint")));
        }
        let fix: Element<'a, Message> = if self.fixing {
            widget::button::suggested(fl!("fixing")).into()
        } else {
            widget::button::suggested(fl!("fix-it")).on_press(Message::FixIt).into()
        };
        col.push(fix).into()
    }
}

impl cosmic::Application for AppModel {
    type Executor = cosmic::executor::Default;
    type Flags = ();
    type Message = Message;

    const APP_ID: &'static str = "io.github.onuraksoy3.Clover";

    fn core(&self) -> &cosmic::Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut cosmic::Core {
        &mut self.core
    }

    fn init(core: cosmic::Core, _flags: Self::Flags) -> (Self, Task<cosmic::Action<Self::Message>>) {
        let config_handler = cosmic_config::Config::new(Self::APP_ID, Config::VERSION).ok();
        let config = config_handler
            .as_ref()
            .map(|handler| match Config::get_entry(handler) {
                Ok(config) => config,
                Err((_errors, config)) => config,
            })
            .unwrap_or_default();

        let mut nav = segmented_button::SingleSelectModel::default();
        for tab in Tab::ALL {
            nav.insert().text(tab.title()).icon(widget::icon::from_name(tab.icon()).size(16)).data(tab);
        }
        nav.activate_position(0);

        let mut app = AppModel { core, nav, config, config_handler, ..Default::default() };
        app.refresh();
        // Runtime keyd bindings do not survive a daemon restart, so push the
        // saved state as soon as the applet comes up with the session.
        app.apply_all();
        (app, Task::none())
    }

    fn on_close_requested(&self, id: Id) -> Option<Message> {
        Some(Message::PopupClosed(id))
    }

    /// Panel button: ⌘ while Mac keys are on, ⌃ while they are off.
    fn view(&self) -> Element<'_, Self::Message> {
        let label = if self.config.mac_keys { "⌘" } else { "⌃" };
        let button = widget::button::custom(self.core.applet.text(label))
            .class(cosmic::theme::Button::AppletIcon)
            .on_press(Message::TogglePopup);
        self.core.applet.autosize_window(button).into()
    }

    fn view_window(&self, _id: Id) -> Element<'_, Self::Message> {
        let active = self.nav.active_data::<Tab>().copied().unwrap_or(Tab::Keys);

        let page: Element<'_, Message> = if !self.search.trim().is_empty() {
            self.page_search()
        } else if !self.checks.all_ok() && !matches!(active, Tab::Settings) {
            self.page_first_run()
        } else {
            let inner = match active {
                Tab::Keys => self.page_keys(),
                Tab::Modifiers => self.page_modifiers(),
                Tab::Trackpad => self.page_trackpad(),
                Tab::Keyboard => self.page_keyboard(),
                Tab::Corners => self.page_corners(),
                Tab::Look => self.page_look(),
                Tab::Apps => self.page_apps(),
                Tab::Settings => self.page_settings(),
            };
            self.titled(active.title(), inner)
        };

        let sidebar = {
            let mut col = widget::column::with_capacity(Tab::ALL.len()).spacing(2).width(Length::Fixed(170.0));
            for tab in Tab::ALL {
                if matches!(tab, Tab::Apps) && self.hw.dgpu.is_none() {
                    continue;
                }
                let selected = tab == active;
                let (r, g, b) = tab.tint();
                let tile = widget::container(widget::icon::from_name(tab.icon()).size(14).icon())
                    .width(Length::Fixed(24.0))
                    .height(Length::Fixed(24.0))
                    .align_x(cosmic::iced::Alignment::Center)
                    .align_y(cosmic::iced::Alignment::Center)
                    .class(cosmic::theme::Container::custom(move |_| cosmic::iced::widget::container::Style {
                        background: Some(cosmic::iced::Color::from_rgb(r, g, b).into()),
                        border: cosmic::iced::Border { radius: 6.0.into(), ..Default::default() },
                        text_color: Some(cosmic::iced::Color::WHITE),
                        icon_color: Some(cosmic::iced::Color::WHITE),
                        ..Default::default()
                    }));
                let label = widget::row::with_capacity(2)
                    .spacing(10)
                    .align_y(cosmic::iced::Alignment::Center)
                    .push(tile)
                    .push(widget::text::body(tab.title()));
                let entity = self.nav.iter().find(|e| self.nav.data::<Tab>(*e) == Some(&tab));
                let mut button = widget::button::custom(label).width(Length::Fill).padding([6, 8]);
                button = if selected { button.class(cosmic::theme::Button::Suggested) } else { button.class(cosmic::theme::Button::Text) };
                if let Some(e) = entity {
                    button = button.on_press(Message::Tab(e));
                }
                col = col.push(button);
            }
            col
        };

        let body_page = widget::scrollable(page).width(Length::Fixed(470.0)).height(Length::Fixed(440.0));

        let mut search = widget::search_input(fl!("search"), &self.search)
            .on_input(Message::Search)
            .width(Length::Fixed(170.0));
        if !self.search.is_empty() {
            search = search.on_clear(Message::Search(String::new()));
        }

        let left = widget::column::with_capacity(2)
            .spacing(10)
            .push(search)
            .push(sidebar);

        let body = widget::row::with_capacity(2)
            .spacing(12)
            .push(left)
            .push(body_page);

        let status = match (&self.error, &self.info) {
            (Some(err), _) => widget::text::caption(fl!("status-error", err = err.clone())),
            (None, Some(info)) => widget::text::caption(info.clone()),
            (None, None) => widget::text::caption(fl!("status-ok")),
        };

        let footer = widget::row::with_capacity(3)
            .spacing(12)
            .align_y(cosmic::iced::Alignment::Center)
            .push(Self::switch(self.config.mac_keys, |v| Message::Set(Toggle::MacKeys, v)))
            .push(widget::text::body(fl!("mac-keys")))
            .push(widget::container(status).width(Length::Fill).align_x(cosmic::iced::Alignment::End));

        let content = widget::column::with_capacity(3)
            .spacing(8)
            .padding([8, 10])
            .width(Length::Fixed(672.0))
            .push(body)
            .push(widget::divider::horizontal::default())
            .push(footer);

        // libcosmic's popup_container pins the width to 360 px; build the same frame at our width.
        static POPUP_ID: std::sync::LazyLock<widget::Id> = std::sync::LazyLock::new(|| widget::Id::new("clover-popup"));
        let framed = widget::container(content).class(cosmic::theme::Container::custom(|theme| {
            let cosmic = theme.cosmic();
            let bg = cosmic.background(theme.transparent).base;
            cosmic::iced::widget::container::Style {
                text_color: Some(cosmic.background(theme.transparent).on.into()),
                background: Some(cosmic::iced::Color::from(bg).into()),
                border: cosmic::iced::Border {
                    radius: cosmic.corner_radii.radius_m.into(),
                    width: 1.0,
                    color: cosmic.background(theme.transparent).divider.into(),
                },
                shadow: Default::default(),
                icon_color: Some(cosmic.background(theme.transparent).on.into()),
                snap: true,
            }
        }));
        widget::autosize::autosize(framed, POPUP_ID.clone())
            .limits(cosmic::iced::core::layout::Limits::NONE.min_width(672.0).max_width(672.0).min_height(1.0).max_height(620.0))
            .into()
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        self.core()
            .watch_config::<Config>(Self::APP_ID)
            .map(|update| Message::UpdateConfig(update.config))
    }

    fn update(&mut self, message: Self::Message) -> Task<cosmic::Action<Self::Message>> {
        match message {
            Message::UpdateConfig(config) => {
                if config != self.config {
                    self.config = config;
                    self.apply_all();
                }
            }
            Message::Tab(entity) => {
                self.nav.activate(entity);
                self.info = None;
            }
            Message::Set(toggle, value) => {
                self.info = None;
                match toggle {
                    Toggle::NaturalScroll => {
                        self.touchpad.natural_scroll = value;
                        self.error = integrations::set_natural_scroll(value).err();
                    }
                    Toggle::TapToClick => {
                        self.touchpad.tap_to_click = value;
                        self.error = integrations::set_tap_to_click(value).err();
                    }
                    Toggle::Clickfinger => {
                        self.touchpad.clickfinger = value;
                        self.error = integrations::set_clickfinger(value).err();
                    }
                    Toggle::FnStandard => {
                        if !self.fixing {
                            self.fixing = true;
                            return Task::perform(
                                async move { tokio::task::spawn_blocking(move || integrations::set_fn_standard(value)).await.unwrap_or_else(|e| Err(e.to_string())) },
                                |r| cosmic::Action::App(Message::FnDone(r)),
                            );
                        }
                    }
                    Toggle::DockHover => {
                        match integrations::hover_preview_set(value) {
                            Ok(()) => self.dock_hover = value,
                            Err(e) => self.error = Some(e),
                        }
                    }
                    Toggle::InterFont => {
                        match integrations::set_inter_font(value, &self.config.font_prev) {
                            Ok(prev) => {
                                if value { self.config.font_prev = prev; }
                                self.config.inter_font = value;
                                self.save_all();
                            }
                            Err(e) => self.error = Some(e),
                        }
                    }
                    _ => {
                        self.set_field(toggle, value);
                        self.save_all();
                        self.apply_all();
                    }
                }
                if self.config.notify && !matches!(toggle, Toggle::Notify | Toggle::NotifyHotkey) {
                    let state = if value { fl!("on") } else { fl!("off") };
                    integrations::notify(&self.label(toggle), &state);
                }
            }
            Message::Hotkey(index) => {
                if let Some((id, _)) = HOTKEYS.get(index) {
                    self.config.hotkey = (*id).to_string();
                    self.save_all();
                    self.apply_all();
                }
            }
            Message::ScrollFactor(v) => self.scroll_factor = v,
            Message::ScrollFactorCommit => {
                let v = (self.scroll_factor as f64 * 10.0).round() / 10.0;
                self.touchpad.scroll_factor = v;
                self.error = integrations::set_scroll_factor(v).err();
            }
            Message::MacTrackpad => {
                self.error = integrations::apply_mac_trackpad().err();
                self.touchpad = integrations::read_touchpad();
                self.scroll_factor = self.touchpad.scroll_factor as f32;
                if self.config.notify {
                    integrations::notify(&fl!("mac-trackpad"), &fl!("applied"));
                }
            }
            Message::ApplyPreset(preset) => {
                match preset {
                    Preset::MacPurist => self.config.mac_purist(),
                    Preset::Hybrid => self.config.hybrid(),
                    Preset::Minimal => self.config.minimal(),
                }
                self.save_all();
                self.apply_all();
                if matches!(preset, Preset::MacPurist | Preset::Hybrid) {
                    self.error = integrations::apply_mac_trackpad().err().or(self.error.take());
                    self.touchpad = integrations::read_touchpad();
                    self.scroll_factor = self.touchpad.scroll_factor as f32;
                }
                self.info = Some(fl!("preset-applied"));
            }
            Message::Export => {
                let path = integrations::export_path();
                self.info = match std::fs::write(&path, self.config.export()) {
                    Ok(()) => Some(fl!("exported", path = integrations::export_path_display())),
                    Err(e) => Some(format!("export failed: {e}")),
                };
            }
            Message::Import => {
                let path = integrations::export_path();
                match std::fs::read_to_string(&path) {
                    Ok(text) => {
                        self.config.import(&text);
                        self.save_all();
                        self.apply_all();
                        self.info = Some(fl!("imported"));
                    }
                    Err(e) => self.info = Some(format!("import failed: {e}")),
                }
            }
            Message::FixIt => {
                if !self.fixing {
                    self.fixing = true;
                    return Task::perform(
                        async { tokio::task::spawn_blocking(integrations::fix_system).await.unwrap_or_else(|e| Err(e.to_string())) },
                        |r| cosmic::Action::App(Message::FixDone(r)),
                    );
                }
            }
            Message::InstallHelpers => {
                if !self.fixing {
                    self.fixing = true;
                    return Task::perform(
                        async { tokio::task::spawn_blocking(integrations::install_helpers).await.unwrap_or_else(|e| Err(e.to_string())) },
                        |r| cosmic::Action::App(Message::HelpersDone(r)),
                    );
                }
            }
            Message::HelpersDone(result) => {
                self.fixing = false;
                self.refresh();
                match result {
                    Ok(()) => self.info = Some(fl!("installed")),
                    Err(e) => self.error = Some(format!("install failed: {e}")),
                }
            }
            Message::FixDone(result) => {
                self.fixing = false;
                self.refresh();
                match result {
                    Ok(()) => {
                        self.apply_all();
                        self.info = Some(fl!("fixed"));
                        integrations::notify(&fl!("app-title"), &fl!("fixed"));
                    }
                    Err(e) => self.error = Some(e),
                }
            }
            Message::Install(id) => {
                self.flatpaks.insert(id, FlatpakState::Installing);
                return Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || integrations::flatpak_install(id))
                            .await
                            .unwrap_or_else(|e| Err(e.to_string()))
                    },
                    move |r| cosmic::Action::App(Message::InstallDone(id, r)),
                );
            }
            Message::InstallDone(id, result) => {
                match result {
                    Ok(()) => {
                        self.flatpaks.insert(id, FlatpakState::Installed);
                        self.info = Some(fl!("installed"));
                        integrations::notify(&fl!("app-title"), &fl!("installed"));
                        self.apply_all();
                    }
                    Err(e) => {
                        self.flatpaks.insert(id, FlatpakState::Missing);
                        self.error = Some(format!("install failed: {e}"));
                    }
                }
            }
            Message::FnDone(result) => {
                self.fixing = false;
                self.fn_standard = integrations::read_fn_standard();
                if let Err(e) = result { self.error = Some(e); }
            }
            Message::Apt(pkgs) => {
                if !self.fixing {
                    self.fixing = true;
                    return Task::perform(
                        async move {
                            tokio::task::spawn_blocking(move || {
                                let list: Vec<&str> = pkgs.split_whitespace().collect();
                                integrations::apt_install(&list)
                            })
                            .await
                            .unwrap_or_else(|e| Err(e.to_string()))
                        },
                        |r| cosmic::Action::App(Message::AptDone(r)),
                    );
                }
            }
            Message::AptDone(result) => {
                self.fixing = false;
                self.refresh();
                match result {
                    Ok(()) => { self.info = Some(fl!("installed")); self.apply_all(); }
                    Err(e) => self.error = Some(format!("install failed: {e}")),
                }
            }
            Message::MinBrightness(v) => self.min_brightness = v,
            Message::MinBrightnessCommit => {
                self.config.min_brightness = self.min_brightness.round() as u32;
                self.save_all();
            }
            Message::IdleSecs(index) => {
                if let Some(s) = IDLE_SECS.get(index) {
                    self.config.kbd_idle_secs = *s;
                    self.save_all();
                }
            }
            Message::Search(s) => self.search = s,
            Message::PointerSpeed(v) => self.pointer_speed = v,
            Message::PointerSpeedCommit => {
                self.touchpad.speed = self.pointer_speed as f64;
                self.error = integrations::set_pointer_speed(self.pointer_speed as f64).err();
            }
            Message::KbdPct(v) => self.kbd_pct = v,
            Message::KbdPctCommit => {
                self.config.kbd_max_pct = self.kbd_pct.round() as u32;
                self.save_all();
            }
            Message::PauseSecs(v) => self.pause_secs = v,
            Message::PauseSecsCommit => {
                self.config.brightness_pause_secs = self.pause_secs.round() as u32;
                self.save_all();
            }
            Message::HoverDelay(v) => self.hover_delay = v,
            Message::HoverDelayCommit => {
                self.error = integrations::hover_delay_set(self.hover_delay.round() as u64).err();
            }
            Message::CornerDelaySlide(v) => self.corner_delay = v,
            Message::CornerDelayCommit => {
                self.config.corner_delay = self.corner_delay.round() as u32;
                self.save_all();
                self.error = integrations::sync_corners(&self.config).err();
            }
            Message::ToggleAdvanced => {
                self.config.advanced = !self.config.advanced;
                self.save_all();
            }
            Message::InstallDock => {
                match integrations::install_clover_dock() {
                    Ok(()) => self.info = Some(fl!("dock-installing")),
                    Err(e) => self.error = Some(e),
                }
            }
            Message::CornerAction(i, ix) => {
                let v = match CORNER_ACTIONS.get(ix).copied().unwrap_or("off") {
                    "cmd" => format!("cmd:{}", self.corner_cmds[i]),
                    other => other.to_string(),
                };
                self.set_corner_value(i, v);
                self.save_all();
                self.error = integrations::sync_corners(&self.config).err();
            }
            Message::CornerCmd(i, s) => self.corner_cmds[i] = s,
            Message::CornerCmdApply(i) => {
                let v = format!("cmd:{}", self.corner_cmds[i].trim());
                self.set_corner_value(i, v);
                self.save_all();
                self.error = integrations::sync_corners(&self.config).err();
            }
            Message::CornerSize(v) => self.corner_size = v,
            Message::CornerSizeCommit => {
                self.config.corner_size = self.corner_size.round() as u32;
                self.save_all();
                self.error = integrations::sync_corners(&self.config).err();
            }
            Message::CornerDelay(ix) => {
                if let Some(d) = [100u32, 200, 350, 500].get(ix) {
                    self.config.corner_delay = *d;
                    self.save_all();
                    self.error = integrations::sync_corners(&self.config).err();
                }
            }
            Message::AppsInput(text) => self.apps_input = text,
            Message::AppsApply => {
                self.config.dgpu_apps = self.apps_input.trim().to_string();
                self.save_all();
                match integrations::sync_gpu_rules(&self.config) {
                    Ok(summary) => self.info = Some(summary),
                    Err(e) => self.error = Some(e),
                }
            }
            Message::Reapply => {
                self.refresh();
                self.apply_all();
                self.info = None;
            }
            Message::TogglePopup => {
                return if let Some(p) = self.popup.take() {
                    destroy_popup(p)
                } else {
                    self.refresh();
                    self.info = None;
                    let new_id = Id::unique();
                    self.popup.replace(new_id);
                    let mut popup_settings = self.core.applet.get_popup_settings(
                        self.core.main_window_id().unwrap(),
                        new_id,
                        None,
                        None,
                        None,
                    );
                    popup_settings.positioner.size_limits = Limits::NONE
                        .max_width(700.0)
                        .min_width(300.0)
                        .min_height(300.0)
                        .max_height(600.0);
                    get_popup(popup_settings)
                };
            }
            Message::PopupClosed(id) => {
                if self.popup.as_ref() == Some(&id) {
                    self.popup = None;
                }
            }
        }
        Task::none()
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(cosmic::applet::style())
    }
}
