// SPDX-License-Identifier: MIT

use cosmic::cosmic_config::{self, CosmicConfigEntry, cosmic_config_derive::CosmicConfigEntry};

/// Persisted settings. Each field is a file under
/// ~/.config/cosmic/io.github.onuraksoy3.Clover/v1/, so the `clover` script and
/// other tools can read or flip them; the applet watches the files and re-applies.
#[derive(Debug, Clone, CosmicConfigEntry, Eq, PartialEq)]
#[version = 1]
pub struct Config {
    // Keys
    pub mac_keys: bool,
    pub tap_super: bool,
    pub cmd_space_launcher: bool,
    pub right_cmd: bool,
    pub ctrl_super: bool,
    pub ctrl_spaces: bool,
    // Shortcuts
    pub text_editing: bool,
    pub navigation: bool,
    pub screenshots: bool,
    pub fkeys: bool,
    pub terminal: bool,
    pub dock_hotkey: bool,
    pub emoji: bool,
    // Keyboard / sensors
    pub kbd_backlight_keys: bool,
    pub auto_brightness: bool,
    pub min_brightness: u32,
    pub auto_kbd: bool,
    pub kbd_max_pct: u32,
    pub kbd_idle_dim: bool,
    pub kbd_idle_secs: u32,
    // Apps / look
    pub games_dgpu: bool,
    pub dgpu_apps: String,
    pub inter_font: bool,
    pub font_prev: String,
    // Applet
    pub notify: bool,
    pub notify_hotkey: bool,
    /// One of "ctrl+alt+m", "super+alt+m", "ctrl+alt+k", "none".
    pub hotkey: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            mac_keys: true,
            tap_super: true,
            cmd_space_launcher: false,
            right_cmd: false,
            ctrl_super: false,
            ctrl_spaces: false,
            text_editing: true,
            navigation: true,
            screenshots: true,
            fkeys: true,
            terminal: true,
            dock_hotkey: false,
            emoji: true,
            kbd_backlight_keys: true,
            auto_brightness: false,
            min_brightness: 20,
            auto_kbd: true,
            kbd_max_pct: 60,
            kbd_idle_dim: true,
            kbd_idle_secs: 30,
            games_dgpu: true,
            dgpu_apps: String::new(),
            inter_font: false,
            font_prev: String::new(),
            notify: true,
            notify_hotkey: true,
            hotkey: "ctrl+alt+m".into(),
        }
    }
}

pub const IDLE_SECS: [u32; 4] = [15, 30, 60, 120];
pub const IDLE_LABELS: [&str; 4] = ["15 s", "30 s", "1 min", "2 min"];

pub const HOTKEY_LABELS: [&str; 4] = ["Ctrl + Alt + M", "Super + Alt + M", "Ctrl + Alt + K", "None"];

pub const HOTKEYS: [(&str, &str); 4] = [
    ("ctrl+alt+m", "Ctrl + Alt + M"),
    ("super+alt+m", "Super + Alt + M"),
    ("ctrl+alt+k", "Ctrl + Alt + K"),
    ("none", "None"),
];

impl Config {
    pub fn mac_purist(&mut self) {
        *self = Self {
            ctrl_super: true,
            ctrl_spaces: true,
            hotkey: self.hotkey.clone(),
            ..Self::default()
        };
    }

    pub fn hybrid(&mut self) {
        *self = Self {
            hotkey: self.hotkey.clone(),
            ..Self::default()
        };
    }

    /// Simple `key=value` text, for export/import without extra crates.
    pub fn export(&self) -> String {
        format!(
            "mac_keys={}\ntap_super={}\ncmd_space_launcher={}\nright_cmd={}\nctrl_super={}\nctrl_spaces={}\ntext_editing={}\nnavigation={}\nscreenshots={}\nfkeys={}\nterminal={}\ndock_hotkey={}\nemoji={}\nkbd_backlight_keys={}\nauto_brightness={}\nmin_brightness={}\nauto_kbd={}\nkbd_max_pct={}\nkbd_idle_dim={}\nkbd_idle_secs={}\ngames_dgpu={}\ndgpu_apps={}\ninter_font={}\nnotify={}\nnotify_hotkey={}\nhotkey={}\n",
            self.mac_keys, self.tap_super, self.cmd_space_launcher, self.right_cmd, self.ctrl_super, self.ctrl_spaces,
            self.text_editing, self.navigation, self.screenshots, self.fkeys,
            self.terminal, self.dock_hotkey, self.emoji, self.kbd_backlight_keys,
            self.auto_brightness, self.min_brightness, self.auto_kbd, self.kbd_max_pct,
            self.kbd_idle_dim, self.kbd_idle_secs, self.games_dgpu, self.dgpu_apps, self.inter_font,
            self.notify, self.notify_hotkey, self.hotkey
        )
    }

    pub fn import(&mut self, text: &str) {
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            let (k, v) = (k.trim(), v.trim());
            let b = v == "true";
            match k {
                "mac_keys" => self.mac_keys = b,
                "tap_super" => self.tap_super = b,
                "cmd_space_launcher" => self.cmd_space_launcher = b,
                "right_cmd" => self.right_cmd = b,
                "ctrl_super" => self.ctrl_super = b,
                "ctrl_spaces" => self.ctrl_spaces = b,
                "text_editing" => self.text_editing = b,
                "navigation" => self.navigation = b,
                "screenshots" => self.screenshots = b,
                "fkeys" => self.fkeys = b,
                "terminal" => self.terminal = b,
                "dock_hotkey" => self.dock_hotkey = b,
                "emoji" => self.emoji = b,
                "kbd_backlight_keys" => self.kbd_backlight_keys = b,
                "auto_brightness" => self.auto_brightness = b,
                "auto_kbd" => self.auto_kbd = b,
                "kbd_idle_dim" => self.kbd_idle_dim = b,
                "games_dgpu" => self.games_dgpu = b,
                "inter_font" => self.inter_font = b,
                "dgpu_apps" => self.dgpu_apps = v.to_string(),
                "min_brightness" => self.min_brightness = v.parse().unwrap_or(20),
                "kbd_max_pct" => self.kbd_max_pct = v.parse().unwrap_or(60),
                "kbd_idle_secs" => self.kbd_idle_secs = v.parse().unwrap_or(30),
                "notify" => self.notify = b,
                "notify_hotkey" => self.notify_hotkey = b,
                "hotkey" => {
                    if HOTKEYS.iter().any(|(id, _)| *id == v) {
                        self.hotkey = v.to_string();
                    }
                }
                _ => {}
            }
        }
    }
}
