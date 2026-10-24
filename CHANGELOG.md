# Changelog

## v0.6.0 — 2026-10-24
- Presets rescoped to keys and modifiers (Minimal, Hybrid, Mac purist), active preset highlighted; explanations moved to tooltips; clearer setting names; new sliders: pointer speed, keyboard backlight level, auto-brightness pause, dock hover delay, hot-corner dwell.
- Presets reworked: Mac purist, Hybrid and new Minimal cover hot corners, automation and GPU rules, and keep personal settings (hotkey, named apps, custom commands, brightness floor, font).
- New layout: macOS-style sidebar (Mac keys, Modifiers, Trackpad, Keyboard, Hot corners, Dock & look, Apps & graphics, Settings), a search field that finds settings across every page, and COSMIC-style section cards.
- Hot corners (clover-corners daemon): per-corner actions — App Library, Workspaces overview, Launcher, Show desktop (built in, minimizes and restores all windows), or any command; corner size and dwell time. Defaults match macOS: top-left App Library, top-right overview, bottom-right Show desktop.

## v0.5.1 — 2026-10-23
- F1+F2 together toggles auto screen brightness; F5+F6 together toggles auto keyboard backlight (was F5+F6 for screen brightness).

## v0.5.0 — 2026-10-23
- Hardware detection: GPU launch rules only on hybrid-graphics laptops, labelled with the detected GPU; Fn mode only with an Apple keyboard; backlight rows only with a keyboard backlight; auto brightness with Apple SMC or IIO light sensors. No MacBook-specific wording left in the UI.

## v0.4.0 — 2026-10-23
- Keyboard: Fn mode, auto screen brightness and auto keyboard backlight from the ambient light sensor (applesmc or IIO), keyboard backlight idle dimming, F5+F6 toggles auto brightness.
- Apps: games and named apps launch on the discrete GPU; Inter as interface font; install/toggle Clover Dock (hover window previews).
- Keys: Right ⌘ as Ctrl, ⌘Space launcher toggle (off by default), ⌘Tab holds Alt until released.
- Shortcuts: real screenshot modes (⌘⇧3/4/5, ⌃ for clipboard), Empty Trash dialog, ⌘⌃Space emoji, ⌘⌥D dock, cosmic-term ⌘N/⌘W.
- Trackpad: Mac preset, two-finger secondary click, scroll speed.
- First-run Fix it (keyd group, config, helpers).

## v0.3.0 — 2026-10-23
- Renamed from Mac Keys to Clover; tabbed UI.
