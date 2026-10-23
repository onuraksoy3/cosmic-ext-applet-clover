# Clover — Mac-style keys for the COSMIC™ panel

A ⌘ button in the panel (⌃ when off). Click it for a small settings popup:

**Keys** — Mac keys (⌘ acts as Ctrl), tap ⌘ to open the launcher, left Ctrl as Super, ⌃arrows for Spaces.

**Shortcuts** — Mac text editing (⌘/⌥ arrows, ⌥⌫, ⌘⌫), Mac navigation (⌘[ ⌘] ⌘⇧[ ⌘⇧] ⌘H ⌘M ⌘⌃F ⌘⇧⌫),
screenshots (⌘⇧3 screen, ⌘⇧4 area, ⌘⇧5 window; add ⌃ to copy to the clipboard instead of saving), F3/F4 as printed, ⌘⌥D Dock auto-hide, ⌘⌃Space emoji picker, cosmic-term tabs/paste.

**Trackpad** — one-click Mac-style preset, natural scrolling, tap to click, two-finger secondary click, scroll speed.

**MacBook** — F5/F6 keyboard backlight through UPower (COSMIC ignores the keys otherwise, but draws the slider), clipboard-history applet install.

**Applet** — a global toggle shortcut, notifications, presets (Mac purist / Hybrid), export/import.

Keyboard behaviour is driven through the running `keyd` daemon over its control socket
(`keyd bind`), so no root is needed day to day. Screenshots, minimize, backlight keys and the
toggle shortcut are written as COSMIC custom shortcuts, so they appear in Settings → Keyboard too.
Trackpad settings are COSMIC's own.

Clover can also install and toggle [Clover Dock](https://github.com/onuraksoy3/cosmic-ext-applet-clover-dock),
the COSMIC dock app list with window previews on hover.

## Install

1. keyd (not packaged for Ubuntu 24.04):
   ```sh
   git clone https://github.com/rvaiya/keyd && cd keyd && make && sudo make install && sudo systemctl enable --now keyd
   ```
2. Rust (Ubuntu's packaged rustc is too old), `just`, and the libcosmic build dependencies:
   ```sh
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y && source ~/.cargo/env
   sudo apt install -y just pkg-config cmake libxkbcommon-dev libwayland-dev libfontconfig-dev \
     libfreetype-dev libexpat1-dev libinput-dev libudev-dev libdbus-1-dev libssl-dev libglib2.0-bin libnotify-bin
   ```
3. Build and install for your user:
   ```sh
   just build-release
   just prefix=$HOME/.local install
   ```
4. Settings → Desktop → Panel → Configure applets → Add → **Clover**.
5. Click ⌘ in the panel. The first-run page lists what is missing; **Fix it** adds you to the
   `keyd` group, and installs the companion keyd config (`examples/keyd-default.conf`, with a backup of
   the old one), with one password prompt. Log out and in once
   after the group change.

## Scripts

- `clover [toggle|on|off|status|dock] [--hotkey]` — flips the saved state; the applet follows.
- `clover-empty-trash` — ⌘⇧⌫, with a confirmation dialog (zenity) or two presses without it.
- `clover-kbd-backlight up|down` — F5/F6, through UPower.
- `clover-screenshot screen|rect|window [clip]` — presets COSMIC's screenshot picker to the wanted mode, then opens it.
- `clover-sensord` — user service behind the auto-brightness/backlight features (reads the applesmc light sensor,
  drives COSMIC's brightness D-Bus API and UPower; uses swayidle for idle dimming).
- `clover-gpu-rules apply|clear|list` — writes/removes the dGPU launch overrides.
- `clover-fnmode 1|2` — Fn mode, run as root by the applet.
- `clover-fix <user> <datadir>` — what Fix it runs as root.

## Not possible (yet)

- Per-app exceptions (keyd cannot see the focused window on COSMIC).
- Three-finger drag and hot corners (not exposed by COSMIC).
- Opening the clipboard-history applet with a key (it only opens from its panel icon).

## Licence

MIT.
