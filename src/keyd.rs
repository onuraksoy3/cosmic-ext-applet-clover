// SPDX-License-Identifier: MIT

//! Talks to the running keyd daemon over its control socket (`keyd bind`).
//! Requires the user to be in the `keyd` group. Runtime bindings persist
//! until `keyd reload`, which is why the applet re-applies them on start,
//! on every config change, and on request.
//!
//! The layers referenced here (`cmd`, `cmd+shift`, `cmd+control`, `cmd+opt`,
//! `opt`, and keyd's built-in `control`) must exist in /etc/keyd/default.conf —
//! see examples/keyd-default.conf.

use crate::config::Config;
use std::process::Command;

pub fn bindings(cfg: &Config) -> Vec<&'static str> {
    let mut b: Vec<&'static str> = Vec::with_capacity(48);

    b.push(match (cfg.mac_keys, cfg.tap_super) {
        (true, true) => "leftmeta = overload(cmd, leftmeta)",
        (true, false) => "leftmeta = layer(cmd)",
        (false, _) => "leftmeta = leftmeta",
    });
    b.push(if cfg.ctrl_super { "leftcontrol = leftmeta" } else { "leftcontrol = leftcontrol" });
    // Right ⌘: plain Super by default, or the same Mac layer as the left one.
    b.push(match (cfg.right_cmd && cfg.mac_keys, cfg.tap_super) {
        (true, true) => "rightmeta = overload(cmd, rightmeta)",
        (true, false) => "rightmeta = layer(cmd)",
        (false, _) => "rightmeta = rightmeta",
    });
    // ⌘Space: launcher (macOS Spotlight habit) or plain Ctrl+Space for apps that use it.
    b.push(if cfg.cmd_space_launcher { "cmd.space = M-/" } else { "cmd.space = C-space" });

    // ⌃← ⌃→ switch workspaces, ⌃↑ overview (macOS Spaces / Mission Control).
    if cfg.ctrl_spaces {
        b.extend(["control.left = M-C-left", "control.right = M-C-right", "control.up = M-w"]);
    } else {
        b.extend(["control.left = C-left", "control.right = C-right", "control.up = C-up"]);
    }

    if cfg.text_editing {
        b.extend([
            "cmd.left = home",
            "cmd.right = end",
            "cmd.up = C-home",
            "cmd.down = C-end",
            "cmd.backspace = delete",
            "opt.left = C-left",
            "opt.right = C-right",
            "opt.backspace = C-backspace",
            "opt.delete = C-delete",
        ]);
    } else {
        b.extend([
            "cmd.left = C-left",
            "cmd.right = C-right",
            "cmd.up = C-up",
            "cmd.down = C-down",
            "cmd.backspace = C-backspace",
            "opt.left = A-left",
            "opt.right = A-right",
            "opt.backspace = A-backspace",
            "opt.delete = A-delete",
        ]);
    }

    if cfg.navigation {
        b.extend([
            "cmd.[ = A-left",
            "cmd.] = A-right",
            "cmd.m = M-m",
            "cmd.h = M-A-h",
            "cmd+shift.[ = C-pageup",
            "cmd+shift.] = C-pagedown",
            "cmd+control.f = M-f11",
            "cmd+shift.backspace = M-A-backspace",
        ]);
    } else {
        b.extend([
            "cmd.[ = C-[",
            "cmd.] = C-]",
            "cmd.m = C-m",
            "cmd.h = C-h",
            "cmd+shift.[ = C-S-[",
            "cmd+shift.] = C-S-]",
            "cmd+control.f = C-f",
            "cmd+shift.backspace = C-S-backspace",
        ]);
    }

    if cfg.screenshots {
        b.extend([
            "cmd+shift.3 = M-A-3",
            "cmd+shift.4 = M-A-4",
            "cmd+shift.5 = M-A-5",
            "cmd+shift+control.3 = M-A-S-3",
            "cmd+shift+control.4 = M-A-S-4",
        ]);
    } else {
        b.extend([
            "cmd+shift.3 = C-S-3",
            "cmd+shift.4 = C-S-4",
            "cmd+shift.5 = C-S-5",
            "cmd+shift+control.3 = C-S-3",
            "cmd+shift+control.4 = C-S-4",
        ]);
    }

    b.push(if cfg.dock_hotkey { "cmd+opt.d = M-A-d" } else { "cmd+opt.d = C-A-d" });
    b.push(if cfg.emoji { "cmd+control.space = M-A-space" } else { "cmd+control.space = C-space" });
    // When left Ctrl is Super, ⌘⌃ chords arrive as cmd+meta instead.
    b.push(if cfg.emoji { "cmd+meta.space = M-A-space" } else { "cmd+meta.space = M-space" });
    b.push(if cfg.navigation { "cmd+meta.f = M-f11" } else { "cmd+meta.f = M-f" });

    b.push("brightnessdown+brightnessup = M-A-f1");
    b.push("kbdillumdown+kbdillumup = M-A-f5");
    if cfg.fkeys {
        b.extend(["scale = M-w", "dashboard = M-a"]);
    } else {
        b.extend(["scale = scale", "dashboard = dashboard"]);
    }

    b
}

/// Applies the bindings. On failure returns a message that says what to fix.
pub fn apply(cfg: &Config) -> Result<(), String> {
    let output = match Command::new("keyd").arg("bind").args(bindings(cfg)).output() {
        Ok(o) => o,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err("keyd is not installed".into());
        }
        Err(e) => return Err(format!("cannot run keyd: {e}")),
    };

    if output.status.success() {
        return Ok(());
    }

    let raw = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stderr).trim(),
        String::from_utf8_lossy(&output.stdout).trim()
    );
    let lower = raw.to_lowercase();

    if lower.contains("permission denied") || lower.contains("connect") {
        Err("no access to keyd (not in the keyd group yet — use Fix it, then log out and in)".into())
    } else if lower.contains("is not a valid layer") {
        Err("keyd config is missing Clover's layers — use Fix it".into())
    } else if lower.contains("no such file") || lower.contains("socket") {
        Err("keyd is not running".into())
    } else {
        Err(raw)
    }
}
