// SPDX-License-Identifier: MIT

//! Everything that is not keyd: COSMIC's custom shortcuts, cosmic-term's
//! shortcuts, the touchpad config, Flatpak helpers, first-run checks and
//! notifications. All config goes through the same files COSMIC Settings uses.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use cosmic::cosmic_config::{self, ConfigGet, ConfigSet};
use cosmic_settings_config::shortcuts::{self, Action, Binding, Modifiers, Shortcuts};
use serde::{Deserialize, Serialize};
use xkbcommon::xkb;

use crate::config::Config;

/// Reads a config key, treating "file does not exist" as the default value
/// but refusing to proceed on any other error — so a newer, unparseable file
/// is never silently overwritten with only our entries.
fn get_or_default<T, E>(key: &str, read: impl FnOnce() -> Result<T, E>) -> Result<T, String>
where
    T: Default,
    E: std::fmt::Debug + std::fmt::Display,
{
    match read() {
        Ok(v) => Ok(v),
        Err(e) => {
            let dbg = format!("{e:?}");
            if dbg.contains("NotFound") || dbg.contains("NoConfigDirectory") || dbg.contains("No such file") {
                Ok(T::default())
            } else {
                Err(format!("cannot read {key} (unknown format?): {e}"))
            }
        }
    }
}

// ------------------------------------------------------- COSMIC shortcuts

fn binding(ctrl: bool, alt: bool, shift: bool, logo: bool, key: &str) -> Binding {
    let sym = xkb::keysym_from_name(key, xkb::KEYSYM_NO_FLAGS);
    Binding::new(Modifiers { ctrl, alt, shift, logo }, Some(sym))
}

/// Super+Alt+<key>: the "internal" combos keyd translates ⌘ chords into.
fn sa(key: &str) -> Binding {
    binding(false, true, false, true, key)
}

fn hotkey_binding(id: &str) -> Option<Binding> {
    match id {
        "ctrl+alt+m" => Some(binding(true, true, false, false, "m")),
        "super+alt+m" => Some(binding(false, true, false, true, "m")),
        "ctrl+alt+k" => Some(binding(true, true, false, false, "k")),
        _ => None,
    }
}

/// Writes every COSMIC custom shortcut Clover owns, in one pass, from the
/// current config. Entries for disabled features are removed.
pub fn sync_cosmic_shortcuts(cfg: &Config) -> Result<(), String> {
    let ctx = shortcuts::context().map_err(|e| format!("shortcuts config: {e}"))?;
    let mut custom: Shortcuts = get_or_default("COSMIC shortcuts", || ctx.get::<Shortcuts>("custom"))?;

    let mut want: Vec<(Binding, Option<Action>)> = Vec::new();

    let spawn = |s: &str| Some(Action::Spawn(s.to_string()));
    let on = |enabled: bool, a: Option<Action>| if enabled { a } else { None };

    want.push((sa("3"), on(cfg.screenshots, spawn("clover-screenshot screen"))));
    want.push((sa("4"), on(cfg.screenshots, spawn("clover-screenshot rect"))));
    want.push((sa("5"), on(cfg.screenshots, spawn("clover-screenshot window"))));
    want.push((binding(false, true, true, true, "3"), on(cfg.screenshots, spawn("clover-screenshot screen clip"))));
    want.push((binding(false, true, true, true, "4"), on(cfg.screenshots, spawn("clover-screenshot rect clip"))));
    want.push((sa("BackSpace"), on(cfg.navigation, spawn("clover-empty-trash"))));
    want.push((sa("h"), on(cfg.navigation, Some(Action::Minimize))));
    want.push((sa("d"), on(cfg.dock_hotkey, spawn("clover dock"))));
    want.push((sa("space"), on(cfg.emoji, spawn("flatpak run it.mijorus.smile"))));
    want.push((
        binding(false, false, false, false, "XF86KbdBrightnessUp"),
        on(cfg.kbd_backlight_keys, spawn("clover-kbd-backlight up")),
    ));
    want.push((
        binding(false, false, false, false, "XF86KbdBrightnessDown"),
        on(cfg.kbd_backlight_keys, spawn("clover-kbd-backlight down")),
    ));
    want.push((sa("F1"), spawn("clover autobrightness")));
    want.push((sa("F5"), spawn("clover autokbd")));
    // Hotkey: remove every candidate, then add the chosen one.
    for (id, _) in crate::config::HOTKEYS {
        if let Some(b) = hotkey_binding(id) {
            want.push((b, None));
        }
    }
    if let Some(b) = hotkey_binding(&cfg.hotkey) {
        want.push((b, spawn("clover toggle --hotkey")));
    }

    for (b, action) in want {
        match action {
            Some(a) => {
                custom.0.insert(b, a);
            }
            None => {
                custom.0.remove(&b);
            }
        }
    }

    ctx.set("custom", &custom)
        .map_err(|e| format!("cannot write COSMIC shortcuts: {e}"))
}

// ------------------------------------------------------------------ terminal

/// Mirrors cosmic-term's `shortcuts::Binding` / `KeyBindAction` so the file
/// round-trips exactly. Keep the action list in sync with cosmic-term.
#[derive(Clone, Debug, Deserialize, Serialize, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum TermModifier {
    Ctrl,
    Shift,
    Alt,
    Super,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct TermBinding {
    modifiers: Vec<TermModifier>,
    key: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
enum TermAction {
    Disable,
    ClearScrollback,
    Copy,
    CopyOrSigint,
    Find,
    PaneFocusDown,
    PaneFocusLeft,
    PaneFocusRight,
    PaneFocusUp,
    PaneSplitHorizontal,
    PaneSplitVertical,
    PaneToggleMaximized,
    Paste,
    PastePrimary,
    PasswordManager,
    SelectAll,
    Settings,
    TabActivate0,
    TabActivate1,
    TabActivate2,
    TabActivate3,
    TabActivate4,
    TabActivate5,
    TabActivate6,
    TabActivate7,
    TabActivate8,
    TabClose,
    TabNew,
    TabNext,
    TabPrev,
    TabMoveLeft,
    TabMoveRight,
    ToggleFullscreen,
    WindowClose,
    WindowNew,
    ZoomIn,
    ZoomOut,
    ZoomReset,
}

type TermShortcuts = BTreeMap<TermBinding, TermAction>;

fn term_ours() -> [(TermBinding, TermAction); 6] {
    let ctrl = |key: &str| TermBinding { modifiers: vec![TermModifier::Ctrl], key: key.to_string() };
    [
        (ctrl("PageUp"), TermAction::TabPrev),
        (ctrl("PageDown"), TermAction::TabNext),
        (ctrl("v"), TermAction::Paste),
        (ctrl("t"), TermAction::TabNew),
        (ctrl("n"), TermAction::WindowNew),
        (ctrl("w"), TermAction::TabClose),
    ]
}

pub fn set_terminal(enabled: bool) -> Result<(), String> {
    let ctx = cosmic_config::Config::new("com.system76.CosmicTerm", 1)
        .map_err(|e| format!("cosmic-term config: {e}"))?;
    let mut custom: TermShortcuts =
        get_or_default("cosmic-term shortcuts", || ctx.get::<TermShortcuts>("shortcuts_custom"))?;
    for (b, a) in term_ours() {
        if enabled {
            custom.insert(b, a);
        } else {
            custom.remove(&b);
        }
    }
    ctx.set("shortcuts_custom", &custom)
        .map_err(|e| format!("cannot write cosmic-term shortcuts: {e}"))
}

// ------------------------------------------------------------------ touchpad

use cosmic_comp_config::input::{AccelConfig, AccelProfile, ClickMethod, InputConfig, ScrollConfig, TapConfig};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Touchpad {
    pub natural_scroll: bool,
    pub tap_to_click: bool,
    pub clickfinger: bool,
    pub scroll_factor: f64,
}

impl Default for Touchpad {
    fn default() -> Self {
        Self { natural_scroll: false, tap_to_click: false, clickfinger: false, scroll_factor: 1.0 }
    }
}

fn comp_ctx() -> Result<cosmic_config::Config, String> {
    cosmic_config::Config::new("com.system76.CosmicComp", 1).map_err(|e| format!("cosmic-comp config: {e}"))
}

fn read_input(ctx: &cosmic_config::Config) -> Result<InputConfig, String> {
    get_or_default("touchpad config", || ctx.get::<InputConfig>("input_touchpad"))
}

fn write_input(ctx: &cosmic_config::Config, cfg: &InputConfig) -> Result<(), String> {
    ctx.set("input_touchpad", cfg).map_err(|e| format!("cannot write touchpad config: {e}"))
}

pub fn read_touchpad() -> Touchpad {
    let Ok(ctx) = comp_ctx() else { return Touchpad::default() };
    let Ok(cfg) = read_input(&ctx) else { return Touchpad::default() };
    Touchpad {
        natural_scroll: cfg.scroll_config.as_ref().and_then(|s| s.natural_scroll).unwrap_or(false),
        tap_to_click: cfg.tap_config.as_ref().map(|t| t.enabled).unwrap_or(false),
        clickfinger: matches!(cfg.click_method, Some(ClickMethod::Clickfinger)),
        scroll_factor: cfg.scroll_config.as_ref().and_then(|s| s.scroll_factor).unwrap_or(1.0),
    }
}

fn edit_touchpad(f: impl FnOnce(&mut InputConfig)) -> Result<(), String> {
    let ctx = comp_ctx()?;
    let mut cfg = read_input(&ctx)?;
    f(&mut cfg);
    write_input(&ctx, &cfg)
}

pub fn set_natural_scroll(enabled: bool) -> Result<(), String> {
    edit_touchpad(|cfg| cfg.scroll_config.get_or_insert_with(ScrollConfig::default).natural_scroll = Some(enabled))
}

pub fn set_tap_to_click(enabled: bool) -> Result<(), String> {
    edit_touchpad(|cfg| match cfg.tap_config.as_mut() {
        Some(tap) => tap.enabled = enabled,
        None => cfg.tap_config = Some(TapConfig { enabled, button_map: None, drag: true, drag_lock: false }),
    })
}

pub fn set_clickfinger(enabled: bool) -> Result<(), String> {
    edit_touchpad(|cfg| {
        cfg.click_method = Some(if enabled { ClickMethod::Clickfinger } else { ClickMethod::ButtonAreas })
    })
}

pub fn set_scroll_factor(factor: f64) -> Result<(), String> {
    edit_touchpad(|cfg| cfg.scroll_config.get_or_insert_with(ScrollConfig::default).scroll_factor = Some(factor))
}

/// The whole macOS feel in one go: natural scrolling, tap to click with drag,
/// two-finger secondary click, adaptive acceleration, palm rejection.
pub fn apply_mac_trackpad() -> Result<(), String> {
    edit_touchpad(|cfg| {
        let scroll = cfg.scroll_config.get_or_insert_with(ScrollConfig::default);
        scroll.natural_scroll = Some(true);
        scroll.scroll_factor = Some(1.0);
        cfg.tap_config = Some(TapConfig { enabled: true, button_map: None, drag: true, drag_lock: false });
        cfg.click_method = Some(ClickMethod::Clickfinger);
        cfg.disable_while_typing = Some(true);
        cfg.acceleration = Some(AccelConfig { profile: Some(AccelProfile::Adaptive), speed: 0.3 });
    })
}

// -------------------------------------------------------------------- flatpak

pub const SMILE: &str = "it.mijorus.smile";
pub const CLIPBOARD: &str = "io.github.cosmic_utils.cosmic-ext-applet-clipboard-manager";

pub fn flatpak_installed(id: &str) -> bool {
    Command::new("flatpak")
        .args(["info", id])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Installs a Flatpak for the user (adds the COSMIC remote first when needed).
/// Blocking — call from a background task.
pub fn flatpak_install(id: &str) -> Result<(), String> {
    if id == CLIPBOARD {
        let _ = Command::new("flatpak")
            .args([
                "remote-add",
                "--if-not-exists",
                "--user",
                "cosmic",
                "https://apt.pop-os.org/cosmic/cosmic.flatpakrepo",
            ])
            .status();
    }
    let out = Command::new("flatpak")
        .args(["install", "-y", "--user", "--noninteractive", id])
        .output()
        .map_err(|e| format!("cannot run flatpak: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

// --------------------------------------------------------------------- checks

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Checks {
    pub keyd_installed: bool,
    pub keyd_running: bool,
    pub keyd_group: bool,
    pub keyd_layers: bool,
    /// zenity (Empty Trash dialog) and wl-clipboard (screenshot to clipboard) present.
    pub helpers: bool,
}

impl Checks {
    pub fn all_ok(&self) -> bool {
        self.keyd_installed && self.keyd_running && self.keyd_group && self.keyd_layers
    }
}

fn in_path(bin: &str) -> bool {
    std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).any(|d| d.join(bin).is_file()))
        .unwrap_or(false)
}

/// Installs zenity + wl-clipboard through one pkexec prompt. Blocking.
pub fn install_helpers() -> Result<(), String> {
    let out = Command::new("pkexec")
        .args(["apt-get", "install", "-y", "-q", "zenity", "wl-clipboard", "libnotify-bin", "libglib2.0-bin"])
        .env("DEBIAN_FRONTEND", "noninteractive")
        .output()
        .map_err(|e| format!("cannot run pkexec: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        Err(if err.is_empty() { "cancelled".into() } else { err })
    }
}

pub fn run_checks() -> Checks {
    let keyd_installed = ["/usr/bin/keyd", "/usr/local/bin/keyd"].iter().any(|p| Path::new(p).exists());
    let keyd_running = Path::new("/var/run/keyd.socket").exists() || Path::new("/run/keyd.socket").exists();
    let keyd_group = Command::new("id")
        .arg("-Gn")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).split_whitespace().any(|g| g == "keyd"))
        .unwrap_or(false);
    let keyd_layers = std::fs::read_to_string("/etc/keyd/default.conf")
        .map(|s| s.contains("[cmd:C]") && s.contains("[cmd+opt]") && s.contains("[cmd+shift+control]") && s.contains("[cmd+meta]") && s.contains("brightnessdown+brightnessup") && s.contains("[app_switch:A]"))
        .unwrap_or(false);
    let helpers = in_path("zenity") && in_path("wl-copy");
    Checks { keyd_installed, keyd_running, keyd_group, keyd_layers, helpers }
}

/// Where `just install` put the helper files (user prefix first, then system).
pub fn data_dir() -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(home) = std::env::var("HOME") {
        candidates.push(PathBuf::from(home).join(".local/share/clover"));
    }
    candidates.push(PathBuf::from("/usr/share/clover"));
    candidates.push(PathBuf::from("/usr/local/share/clover"));
    candidates.into_iter().find(|p| p.join("clover-fix").exists())
}

/// Runs the repair script as root through pkexec (one password prompt).
/// Blocking — call from a background task.
pub fn fix_system() -> Result<(), String> {
    let data = data_dir().ok_or("Clover's helper files were not found; run `just install` again")?;
    let user = std::env::var("USER").map_err(|_| "cannot determine user name")?;
    let out = Command::new("pkexec")
        .arg(data.join("clover-fix"))
        .arg(&user)
        .arg(&data)
        .output()
        .map_err(|e| format!("cannot run pkexec: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        Err(if err.is_empty() { "cancelled".into() } else { err })
    }
}

// --------------------------------------------------------------- export/import

pub fn export_path_display() -> String {
    let p = export_path();
    match std::env::var("HOME") {
        Ok(home) => p.display().to_string().replacen(&home, "~", 1),
        Err(_) => p.display().to_string(),
    }
}

pub fn export_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    let docs = PathBuf::from(&home).join("Documents");
    if docs.is_dir() { docs.join("clover-settings.txt") } else { PathBuf::from(home).join("clover-settings.txt") }
}

// ------------------------------------------------------------- notifications

pub fn notify(summary: &str, body: &str) {
    let _ = Command::new("notify-send")
        .args(["-a", "Clover", "-i", "input-keyboard-symbolic", "-t", "2500", summary, body])
        .spawn();
}

// --------------------------------------------------------------- Fn mode

/// `true` = F1–F12 are plain function keys (hid_apple fnmode=2); `false` = media keys first (1).
pub fn read_fn_standard() -> Option<bool> {
    std::fs::read_to_string("/sys/module/hid_apple/parameters/fnmode")
        .ok()
        .map(|s| s.trim() == "2")
}

/// Blocking (pkexec prompt).
pub fn set_fn_standard(standard: bool) -> Result<(), String> {
    let data = data_dir().ok_or("Clover's helper files were not found; run `just install` again")?;
    let out = Command::new("pkexec")
        .arg(data.join("clover-fnmode"))
        .arg(if standard { "2" } else { "1" })
        .output()
        .map_err(|e| format!("cannot run pkexec: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        Err(if err.is_empty() { "cancelled".into() } else { err })
    }
}

// ------------------------------------------------------------ sensor daemon

fn user_unit_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".config/systemd/user")
}

/// Starts or stops the ambient-light daemon depending on whether any of its features is on.
pub fn sync_sensord(cfg: &Config) -> Result<(), String> {
    let needed = cfg.auto_brightness || cfg.auto_kbd || cfg.kbd_idle_dim;
    let unit = user_unit_dir().join("clover-sensord.service");
    if needed {
        if !unit.exists() {
            let data = data_dir().ok_or("Clover's helper files were not found; run `just install` again")?;
            std::fs::create_dir_all(user_unit_dir()).map_err(|e| e.to_string())?;
            std::fs::copy(data.join("clover-sensord.service"), &unit)
                .map_err(|e| format!("cannot install user unit: {e}"))?;
            let _ = Command::new("systemctl").args(["--user", "daemon-reload"]).status();
        }
        let ok = Command::new("systemctl")
            .args(["--user", "enable", "--now", "clover-sensord.service"])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if ok { Ok(()) } else { Err("could not start clover-sensord (systemctl --user)".into()) }
    } else {
        let _ = Command::new("systemctl").args(["--user", "disable", "--now", "clover-sensord.service"]).status();
        Ok(())
    }
}

pub fn light_sensor_present() -> bool {
    Path::new("/sys/devices/platform/applesmc.768/light").exists()
}

pub fn swayidle_present() -> bool {
    in_path("swayidle")
}

// ------------------------------------------------------------- GPU rules

fn bin_path(name: &str) -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        let p = PathBuf::from(home).join(".local/bin").join(name);
        if p.exists() {
            return p;
        }
    }
    for d in ["/usr/local/bin", "/usr/bin"] {
        let p = PathBuf::from(d).join(name);
        if p.exists() {
            return p;
        }
    }
    PathBuf::from(name)
}

/// Rewrites the dGPU launch overrides from the current config. Returns the script's one-line summary.
pub fn sync_gpu_rules(cfg: &Config) -> Result<String, String> {
    let arg = if cfg.games_dgpu || !cfg.dgpu_apps.trim().is_empty() { "apply" } else { "clear" };
    let out = Command::new(bin_path("clover-gpu-rules"))
        .arg(arg)
        .output()
        .map_err(|e| format!("gpu rules: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

// ---------------------------------------------------------------- font

pub fn inter_font_installed() -> bool {
    Command::new("fc-list")
        .args([":", "family"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).lines().any(|l| l.split(',').any(|f| f.trim() == "Inter")))
        .unwrap_or(false)
}

/// Sets (or restores) COSMIC's interface font. Returns the family that was in place before
/// enabling, so the caller can remember it.
pub fn set_inter_font(enable: bool, prev: &str) -> Result<String, String> {
    use cosmic::config::FontConfig;
    use cosmic::iced::font::{Stretch, Style, Weight};
    let ctx = cosmic_config::Config::new(cosmic::config::ID, 1).map_err(|e| format!("CosmicTk config: {e}"))?;
    let normal = |family: String| FontConfig { family, weight: Weight::Normal, stretch: Stretch::Normal, style: Style::Normal };
    let current: FontConfig = ctx.get::<FontConfig>("interface_font").unwrap_or_else(|_| normal("Open Sans".into()));
    if enable {
        let before = current.family.clone();
        ctx.set("interface_font", &normal("Inter".into())).map_err(|e| format!("cannot set interface font: {e}"))?;
        Ok(before)
    } else {
        let family = if prev.is_empty() { "Open Sans".to_string() } else { prev.to_string() };
        ctx.set("interface_font", &normal(family)).map_err(|e| format!("cannot restore interface font: {e}"))?;
        Ok(String::new())
    }
}

/// Installs Debian packages through one pkexec prompt. Blocking.
pub fn apt_install(pkgs: &[&str]) -> Result<(), String> {
    let mut args: Vec<&str> = vec!["apt-get", "install", "-y", "-q"];
    args.extend(pkgs);
    let out = Command::new("pkexec")
        .args(&args)
        .env("DEBIAN_FRONTEND", "noninteractive")
        .output()
        .map_err(|e| format!("cannot run pkexec: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        Err(if err.is_empty() { "cancelled".into() } else { err })
    }
}

// ------------------------------------------------------------ Clover Dock

pub fn clover_dock_installed() -> bool {
    bin_path("clover-dock").exists()
}

/// Reads the stock app list's `hover_preview` setting (shared with Clover Dock).
pub fn hover_preview_get() -> bool {
    cosmic_config::Config::new("com.system76.CosmicAppList", 1)
        .ok()
        .and_then(|c| c.get::<bool>("hover_preview").ok())
        .unwrap_or(false)
}

pub fn hover_preview_set(on: bool) -> Result<(), String> {
    let c = cosmic_config::Config::new("com.system76.CosmicAppList", 1).map_err(|e| e.to_string())?;
    c.set("hover_preview", on).map_err(|e| format!("cannot set hover_preview: {e}"))
}

/// Opens a terminal running the Clover Dock installer (it builds from source, ~15 min first time).
pub fn install_clover_dock() -> Result<(), String> {
    let script = "curl -fsSL https://github.com/onuraksoy3/cosmic-ext-applet-clover-dock/raw/main/build.sh | bash; echo; read -rp 'Press Enter to close'";
    for term in ["cosmic-term", "x-terminal-emulator", "gnome-terminal", "xterm"] {
        let args: Vec<&str> = match term {
            "cosmic-term" => vec!["--", "bash", "-c", script],
            "gnome-terminal" => vec!["--", "bash", "-c", script],
            _ => vec!["-e", "bash", "-c", script],
        };
        if Command::new(term).args(&args).spawn().is_ok() {
            return Ok(());
        }
    }
    Err("no terminal emulator found".into())
}

// ------------------------------------------------------------- hardware

/// What this machine has, so the UI only offers what applies.
#[derive(Clone, Debug, Default)]
pub struct Hardware {
    /// Short name of the discrete GPU when the machine is a hybrid (switcheroo-control), e.g. "GeForce GT 750M".
    pub dgpu: Option<String>,
    /// Apple keyboard driver loaded (built-in MacBook keyboard or an external Apple keyboard).
    pub apple_keyboard: bool,
    pub kbd_backlight: bool,
    pub light_sensor: bool,
}

pub fn detect_hardware() -> Hardware {
    let dgpu = Command::new("switcherooctl")
        .arg("list")
        .output()
        .ok()
        .and_then(|o| {
            let text = String::from_utf8_lossy(&o.stdout);
            let mut name: Option<String> = None;
            let mut found = None;
            for line in text.lines() {
                let l = line.trim();
                if let Some(n) = l.strip_prefix("Name:") {
                    name = Some(n.trim().to_string());
                } else if let Some(d) = l.strip_prefix("Default:") {
                    if d.trim() == "no" {
                        if let Some(n) = name.take() {
                            found = Some(n);
                            break;
                        }
                    }
                }
            }
            found
        })
        .map(|full| {
            // "NVIDIA Corporation GK107M [GeForce GT 750M Mac Edition]" → "GeForce GT 750M Mac Edition"
            match (full.rfind('['), full.rfind(']')) {
                (Some(a), Some(b)) if b > a => full[a + 1..b].to_string(),
                _ => full,
            }
        });
    let kbd_backlight = std::fs::read_dir("/sys/class/leds")
        .map(|d| d.flatten().any(|e| e.file_name().to_string_lossy().contains("kbd_backlight")))
        .unwrap_or(false);
    let iio = std::fs::read_dir("/sys/bus/iio/devices")
        .map(|d| {
            d.flatten().any(|e| {
                e.path().join("in_illuminance_raw").exists() || e.path().join("in_illuminance_input").exists()
            })
        })
        .unwrap_or(false);
    Hardware {
        dgpu,
        apple_keyboard: Path::new("/sys/module/hid_apple").exists(),
        kbd_backlight,
        light_sensor: light_sensor_present() || iio,
    }
}
