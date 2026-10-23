// SPDX-License-Identifier: MIT

use crate::integrations::Hardware;
use crate::config::{Config, HOTKEYS, HOTKEY_LABELS, IDLE_LABELS, IDLE_SECS};
use crate::fl;
use crate::integrations::{self, Checks, Touchpad};
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
    Shortcuts,
    Trackpad,
    Keyboard,
    Apps,
    Applet,
}

impl Tab {
    const ALL: [Tab; 6] = [Tab::Keys, Tab::Shortcuts, Tab::Trackpad, Tab::Keyboard, Tab::Apps, Tab::Applet];

    fn icon(self) -> &'static str {
        match self {
            Tab::Keys => "input-keyboard-symbolic",
            Tab::Shortcuts => "preferences-desktop-keyboard-shortcuts-symbolic",
            Tab::Trackpad => "input-touchpad-symbolic",
            Tab::Keyboard => "computer-symbolic",
            Tab::Apps => "applications-games-symbolic",
            Tab::Applet => "emblem-system-symbolic",
        }
    }

    fn title(self) -> String {
        match self {
            Tab::Keys => fl!("tab-keys"),
            Tab::Shortcuts => fl!("tab-shortcuts"),
            Tab::Trackpad => fl!("tab-trackpad"),
            Tab::Keyboard => fl!("tab-keyboard"),
            Tab::Apps => fl!("tab-apps"),
            Tab::Applet => fl!("tab-applet"),
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
            Toggle::FnStandard => fl!("fn-standard-hint"),
            Toggle::Clickfinger => fl!("clickfinger-hint"),
            _ => String::new(),
        }
    }

    fn row<'a>(&self, toggle: Toggle, value: bool) -> Element<'a, Message> {
        let hint = self.hint(toggle);
        let mut item = widget::settings::item::builder(self.label(toggle));
        if !hint.is_empty() {
            item = item.description(hint);
        }
        item.control(widget::toggler(value).on_toggle(move |v| Message::Set(toggle, v)))
            .into()
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

    fn page_keys<'a>(&self) -> Element<'a, Message> {
        let c = &self.config;
        widget::list_column()
            .add(self.row(Toggle::MacKeys, c.mac_keys))
            .add(self.row(Toggle::TapSuper, c.tap_super))
            .add(self.row(Toggle::CmdSpaceLauncher, c.cmd_space_launcher))
            .add(self.row(Toggle::RightCmd, c.right_cmd))
            .add(self.row(Toggle::CtrlSuper, c.ctrl_super))
            .add(self.row(Toggle::CtrlSpaces, c.ctrl_spaces))
            .into()
    }

    fn page_shortcuts<'a>(&self) -> Element<'a, Message> {
        let c = &self.config;
        widget::list_column()
            .add(self.row(Toggle::TextEditing, c.text_editing))
            .add(self.row(Toggle::Navigation, c.navigation))
            .add(self.helpers_row())
            .add(self.row(Toggle::Screenshots, c.screenshots))
            .add(self.row(Toggle::Fkeys, c.fkeys))
            .add(self.row(Toggle::DockHotkey, c.dock_hotkey))
            .add(self.flatpak_row(integrations::SMILE, Toggle::Emoji, c.emoji))
            .add(self.row(Toggle::Terminal, c.terminal))
            .into()
    }

    fn page_trackpad<'a>(&self) -> Element<'a, Message> {
        let t = self.touchpad;
        let slider = widget::slider(0.2..=3.0, self.scroll_factor, Message::ScrollFactor)
            .step(0.1_f32)
            .on_release(Message::ScrollFactorCommit)
            .width(Length::Fixed(150.0));
        widget::list_column()
            .add(Self::item(
                fl!("mac-trackpad"),
                fl!("mac-trackpad-hint"),
                widget::button::suggested(fl!("apply")).on_press(Message::MacTrackpad),
            ))
            .add(self.row(Toggle::NaturalScroll, t.natural_scroll))
            .add(self.row(Toggle::TapToClick, t.tap_to_click))
            .add(self.row(Toggle::Clickfinger, t.clickfinger))
            .add(Self::item(fl!("scroll-speed"), String::new(), slider))
            .add(Self::note(fl!("trackpad-note")))
            .into()
    }

    fn page_keyboard<'a>(&self) -> Element<'a, Message> {
        let c = &self.config;
        let mut col = widget::list_column();
        // Fn mode
        let fn_control: Element<'a, Message> = match self.fn_standard {
            Some(v) if !self.fixing => widget::toggler(v).on_toggle(|x| Message::Set(Toggle::FnStandard, x)).into(),
            Some(_) => widget::text::body(fl!("applying")).into(),
            None => widget::text::body(fl!("unavailable")).into(),
        };
        col = col.add(Self::item(self.label(Toggle::FnStandard), self.hint(Toggle::FnStandard), fn_control));
        col = col.add(self.row(Toggle::KbdBacklightKeys, c.kbd_backlight_keys));
        if self.light_sensor {
            col = col.add(self.row(Toggle::AutoBrightness, c.auto_brightness));
            if c.auto_brightness {
                let slider = widget::slider(5.0..=60.0, self.min_brightness, Message::MinBrightness)
                    .step(5.0_f32)
                    .on_release(Message::MinBrightnessCommit)
                    .width(Length::Fixed(150.0));
                col = col.add(Self::item(fl!("min-brightness"), format!("{}%", self.min_brightness as u32), slider));
            }
            if self.hw.kbd_backlight {
                col = col.add(self.row(Toggle::AutoKbd, c.auto_kbd));
            }
        } else {
            col = col.add(Self::note(fl!("no-light-sensor")));
        }
        if !self.hw.kbd_backlight {
            // nothing to dim
        } else if self.swayidle {
            col = col.add(self.row(Toggle::KbdIdleDim, c.kbd_idle_dim));
            if c.kbd_idle_dim {
                let selected = IDLE_SECS.iter().position(|s| *s == c.kbd_idle_secs);
                col = col.add(Self::item(fl!("idle-after"), String::new(), widget::dropdown(&IDLE_LABELS, selected, Message::IdleSecs)));
            }
        } else {
            col = col.add(Self::item(
                self.label(Toggle::KbdIdleDim),
                fl!("needs-swayidle"),
                widget::button::suggested(fl!("install")).on_press(Message::Apt("swayidle")),
            ));
        }
        col = col.add(self.clipboard_row());
        col.add(Self::note(fl!("keyboard-note"))).into()
    }

    fn page_apps<'a>(&'a self) -> Element<'a, Message> {
        let c = &self.config;
        let input = widget::text_input(fl!("apps-placeholder"), &self.apps_input)
            .on_input(Message::AppsInput)
            .width(Length::Fill);
        let apply = widget::button::standard(fl!("apply")).on_press(Message::AppsApply);
        let dock_control: Element<'a, Message> = if self.dock_installed {
            widget::toggler(self.dock_hover).on_toggle(|v| Message::Set(Toggle::DockHover, v)).into()
        } else {
            widget::button::suggested(fl!("install")).on_press(Message::InstallDock).into()
        };
        let font_control: Element<'a, Message> = if self.inter_installed {
            widget::toggler(c.inter_font).on_toggle(|v| Message::Set(Toggle::InterFont, v)).into()
        } else if self.fixing {
            widget::text::body(fl!("installing")).into()
        } else {
            widget::button::suggested(fl!("install")).on_press(Message::Apt("fonts-inter")).into()
        };
        widget::list_column()
            .add(self.row(Toggle::GamesDgpu, c.games_dgpu))
            .add(widget::column::with_capacity(3)
                .spacing(6)
                .push(widget::text::body(fl!("apps-named")))
                .push(widget::text::caption(fl!("apps-named-hint")))
                .push(widget::row::with_capacity(2).spacing(8).push(input).push(apply)))
            .add(Self::note(fl!("apps-note")))
            .add(Self::item(self.label(Toggle::InterFont), self.hint(Toggle::InterFont), font_control))
            .add(Self::item(self.label(Toggle::DockHover), self.hint(Toggle::DockHover), dock_control))
            .into()
    }

    fn page_applet<'a>(&self) -> Element<'a, Message> {
        let c = &self.config;
        let selected = HOTKEYS.iter().position(|(id, _)| *id == c.hotkey);
        let presets = widget::row::with_capacity(2)
            .spacing(8)
            .push(widget::button::standard(fl!("preset-purist")).on_press(Message::ApplyPreset(Preset::MacPurist)))
            .push(widget::button::standard(fl!("preset-hybrid")).on_press(Message::ApplyPreset(Preset::Hybrid)));
        let io = widget::row::with_capacity(2)
            .spacing(8)
            .push(widget::button::standard(fl!("export")).on_press(Message::Export))
            .push(widget::button::standard(fl!("import")).on_press(Message::Import));
        widget::list_column()
            .add(Self::item(fl!("hotkey"), fl!("hotkey-hint"), widget::dropdown(&HOTKEY_LABELS, selected, Message::Hotkey)))
            .add(self.row(Toggle::NotifyHotkey, c.notify_hotkey))
            .add(self.row(Toggle::Notify, c.notify))
            .add(Self::item(fl!("presets"), fl!("presets-hint"), presets))
            .add(Self::item(
                fl!("settings-file"),
                fl!("settings-file-hint", path = integrations::export_path_display()),
                io,
            ))
            .add(Self::item(fl!("reapply"), String::new(), widget::button::standard("⟳").on_press(Message::Reapply)))
            .into()
    }

    /// Shown instead of the tabs while something essential is missing.
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
            nav.insert().icon(widget::icon::from_name(tab.icon()).size(18)).data(tab);
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

        let page: Element<'_, Message> = if !self.checks.all_ok() && !matches!(active, Tab::Applet) {
            self.page_first_run()
        } else {
            match active {
                Tab::Keys => self.page_keys(),
                Tab::Shortcuts => self.page_shortcuts(),
                Tab::Trackpad => self.page_trackpad(),
                Tab::Keyboard => self.page_keyboard(),
                Tab::Apps => self.page_apps(),
                Tab::Applet => self.page_applet(),
            }
        };

        let tabs = widget::tab_bar::horizontal(&self.nav)
            .on_activate(Message::Tab)
            .minimum_button_width(44)
            .maximum_button_width(96)
            .button_height(36)
            .button_spacing(4)
            .width(Length::Fill);

        let heading = widget::container(widget::text::title4(active.title())).padding([4, 10]);

        let body = widget::column::with_capacity(3)
            .spacing(6)
            .push(tabs)
            .push(heading)
            .push(widget::scrollable(page).width(Length::Fill).height(Length::Fixed(340.0)));

        let status = match (&self.error, &self.info) {
            (Some(err), _) => widget::text::caption(fl!("status-error", err = err.clone())),
            (None, Some(info)) => widget::text::caption(info.clone()),
            (None, None) => widget::text::caption(fl!("status-ok")),
        };

        let footer = widget::row::with_capacity(2)
            .spacing(12)
            .push(widget::toggler(self.config.mac_keys).on_toggle(|v| Message::Set(Toggle::MacKeys, v)))
            .push(status);

        let content = widget::column::with_capacity(3)
            .spacing(8)
            .padding([8, 10])
            .push(body)
            .push(widget::divider::horizontal::default())
            .push(footer);

        self.core.applet.popup_container(content).into()
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
                }
                self.save_all();
                self.apply_all();
                if matches!(preset, Preset::MacPurist) {
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
            Message::InstallDock => {
                match integrations::install_clover_dock() {
                    Ok(()) => self.info = Some(fl!("dock-installing")),
                    Err(e) => self.error = Some(e),
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
                        .max_width(480.0)
                        .min_width(440.0)
                        .min_height(300.0)
                        .max_height(520.0);
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
