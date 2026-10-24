// clover-corners — hot corners for COSMIC.
// SPDX-License-Identifier: GPL-3.0-only
//
// Pins small invisible layer-shell surfaces to the screen corners (the same approach as waycorner)
// and runs an action when the pointer dwells in one. "Show desktop" is built in: it minimizes every
// visible window through COSMIC's toplevel-management protocol and restores the same set next time.
//
// Configuration is read from Clover's cosmic-config files
// (~/.config/cosmic/io.github.onuraksoy3.Clover/v1/corner_{tl,tr,bl,br}, corner_size, corner_delay):
//   "off" | "library" | "overview" | "launcher" | "desktop" | "cmd:<shell command>"

use std::{collections::HashSet, path::PathBuf, process::Command, time::Duration};

#[allow(unused_imports)]
use cctk::sctk;
use cctk::{
    sctk::{
        compositor::{CompositorHandler, CompositorState},
        output::{OutputHandler, OutputState},
        reexports::{
            calloop::{
                self,
                timer::{TimeoutAction, Timer},
                EventLoop, LoopHandle, RegistrationToken,
            },
            calloop_wayland_source::WaylandSource,
        },
        registry::{ProvidesRegistryState, RegistryState},
        registry_handlers,
        seat::{
            pointer::{PointerEvent, PointerEventKind, PointerHandler},
            Capability, SeatHandler, SeatState,
        },
        shell::{
            wlr_layer::{Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface, LayerSurfaceConfigure},
            WaylandSurface,
        },
        shm::{slot::SlotPool, Shm, ShmHandler},
    },
    toplevel_info::{ToplevelInfoHandler, ToplevelInfoState},
    toplevel_management::{ToplevelManagerHandler, ToplevelManagerState},
    wayland_client::{
        globals::registry_queue_init,
        protocol::{wl_output, wl_pointer, wl_seat, wl_shm, wl_surface},
        Connection, QueueHandle, WEnum,
    },
    wayland_protocols::ext::foreign_toplevel_list::v1::client::ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1,
    workspace::{WorkspaceHandler, WorkspaceState},
};
use cosmic_protocols::{
    toplevel_info::v1::client::zcosmic_toplevel_handle_v1::{State as ToplevelState, ZcosmicToplevelHandleV1},
    toplevel_management::v1::client::zcosmic_toplevel_manager_v1,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Corner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl Corner {
    const ALL: [Corner; 4] = [Corner::TopLeft, Corner::TopRight, Corner::BottomLeft, Corner::BottomRight];
    fn key(self) -> &'static str {
        match self {
            Corner::TopLeft => "corner_tl",
            Corner::TopRight => "corner_tr",
            Corner::BottomLeft => "corner_bl",
            Corner::BottomRight => "corner_br",
        }
    }
    fn anchor(self) -> Anchor {
        match self {
            Corner::TopLeft => Anchor::TOP | Anchor::LEFT,
            Corner::TopRight => Anchor::TOP | Anchor::RIGHT,
            Corner::BottomLeft => Anchor::BOTTOM | Anchor::LEFT,
            Corner::BottomRight => Anchor::BOTTOM | Anchor::RIGHT,
        }
    }
}

#[derive(Clone, Debug)]
enum Action {
    Off,
    Library,
    Overview,
    Launcher,
    ShowDesktop,
    Command(String),
}

impl Action {
    fn parse(s: &str) -> Action {
        match s.trim() {
            "library" => Action::Library,
            "overview" => Action::Overview,
            "launcher" => Action::Launcher,
            "desktop" => Action::ShowDesktop,
            s if s.starts_with("cmd:") => Action::Command(s[4..].trim().to_string()),
            _ => Action::Off,
        }
    }
}

fn config_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".config/cosmic/io.github.onuraksoy3.Clover/v1")
}

/// cosmic-config stores strings as RON (`"library"`) and numbers bare (`10`).
fn read_key(key: &str) -> Option<String> {
    let s = std::fs::read_to_string(config_dir().join(key)).ok()?;
    Some(s.trim().trim_matches('"').to_string())
}

struct Config {
    actions: [(Corner, Action); 4],
    size: u32,
    delay_ms: u64,
}

impl Config {
    fn load() -> Config {
        let defaults = |c: Corner| match c {
            Corner::TopLeft => "library",
            Corner::TopRight => "overview",
            Corner::BottomRight => "desktop",
            Corner::BottomLeft => "off",
        };
        let actions = Corner::ALL.map(|c| (c, Action::parse(&read_key(c.key()).unwrap_or_else(|| defaults(c).into()))));
        let size = read_key("corner_size").and_then(|v| v.parse().ok()).unwrap_or(10u32).clamp(4, 100);
        let delay_ms = read_key("corner_delay").and_then(|v| v.parse().ok()).unwrap_or(200u64).clamp(0, 2000);
        Config { actions, size, delay_ms }
    }
    fn action(&self, c: Corner) -> &Action {
        &self.actions.iter().find(|(k, _)| *k == c).unwrap().1
    }
}

struct CornerSurface {
    corner: Corner,
    layer: LayerSurface,
    output: wl_output::WlOutput,
}

struct App {
    registry_state: RegistryState,
    output_state: OutputState,
    seat_state: SeatState,
    compositor_state: CompositorState,
    shm: Shm,
    layer_shell: LayerShell,
    toplevel_info_state: ToplevelInfoState,
    toplevel_manager_state: ToplevelManagerState,
    workspace_state: WorkspaceState,
    pool: SlotPool,
    config: Config,
    surfaces: Vec<CornerSurface>,
    loop_handle: LoopHandle<'static, App>,
    /// Pending dwell timer for the corner the pointer is in.
    pending: Option<(Corner, RegistrationToken)>,
    /// Corner already fired; must be left before it fires again.
    fired: Option<Corner>,
    /// Windows we minimized for Show Desktop, to restore on the next trigger.
    hidden: Vec<ZcosmicToplevelHandleV1>,
    pointer: Option<wl_pointer::WlPointer>,
}

impl App {
    fn create_corners(&mut self, qh: &QueueHandle<Self>, output: &wl_output::WlOutput) {
        for (corner, action) in self.config.actions.iter() {
            if matches!(action, Action::Off) {
                continue;
            }
            let surface = self.compositor_state.create_surface(qh);
            let layer = self.layer_shell.create_layer_surface(qh, surface, Layer::Overlay, Some("clover-corners"), Some(output));
            layer.set_anchor(corner.anchor());
            layer.set_size(self.config.size, self.config.size);
            layer.set_exclusive_zone(-1);
            layer.set_keyboard_interactivity(KeyboardInteractivity::None);
            layer.commit();
            self.surfaces.push(CornerSurface { corner: *corner, layer, output: output.clone() });
        }
    }

    fn draw(&mut self, layer: &LayerSurface) {
        let size = self.config.size as i32;
        let stride = size * 4;
        if let Ok((buffer, canvas)) = self.pool.create_buffer(size, size, stride, wl_shm::Format::Argb8888) {
            canvas.fill(0); // fully transparent
            let _ = buffer.attach_to(layer.wl_surface());
            layer.wl_surface().damage_buffer(0, 0, size, size);
            layer.commit();
        }
    }

    fn corner_of(&self, surface: &wl_surface::WlSurface) -> Option<Corner> {
        self.surfaces.iter().find(|s| s.layer.wl_surface() == surface).map(|s| s.corner)
    }

    fn cancel_pending(&mut self) {
        if let Some((_, token)) = self.pending.take() {
            self.loop_handle.remove(token);
        }
    }

    fn fire(&mut self, corner: Corner) {
        let action = self.config.action(corner).clone();
        eprintln!("clover-corners: {corner:?} -> {action:?}");
        match action {
            Action::Off => {}
            Action::Library => spawn("cosmic-app-library"),
            Action::Overview => spawn("cosmic-workspaces"),
            Action::Launcher => spawn("cosmic-launcher"),
            Action::Command(cmd) => {
                let _ = Command::new("sh").arg("-c").arg(cmd).spawn();
            }
            Action::ShowDesktop => self.toggle_show_desktop(),
        }
    }

    fn toggle_show_desktop(&mut self) {
        let manager = self.toplevel_manager_state.manager.clone();
        let visible: Vec<ZcosmicToplevelHandleV1> = self
            .toplevel_info_state
            .toplevels()
            .filter(|t| !t.state.contains(&ToplevelState::Minimized))
            .filter_map(|t| t.cosmic_toplevel.clone())
            .collect();
        if !visible.is_empty() {
            for h in &visible {
                manager.set_minimized(h);
            }
            self.hidden = visible;
        } else if !self.hidden.is_empty() {
            let alive: HashSet<ZcosmicToplevelHandleV1> = self
                .toplevel_info_state
                .toplevels()
                .filter_map(|t| t.cosmic_toplevel.clone())
                .collect();
            for h in self.hidden.drain(..) {
                if alive.contains(&h) {
                    manager.unset_minimized(&h);
                }
            }
        }
    }
}

fn spawn(bin: &str) {
    let _ = Command::new(bin).spawn();
}

impl CompositorHandler for App {
    fn scale_factor_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: i32) {}
    fn transform_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: wl_output::Transform) {}
    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: u32) {}
    fn surface_enter(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: &wl_output::WlOutput) {}
    fn surface_leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: &wl_output::WlOutput) {}
}

impl OutputHandler for App {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }
    fn new_output(&mut self, _: &Connection, qh: &QueueHandle<Self>, output: wl_output::WlOutput) {
        self.create_corners(qh, &output);
    }
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, output: wl_output::WlOutput) {
        self.surfaces.retain(|s| s.output != output);
    }
}

impl LayerShellHandler for App {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, layer: &LayerSurface) {
        self.surfaces.retain(|s| &s.layer != layer);
    }
    fn configure(&mut self, _: &Connection, _: &QueueHandle<Self>, layer: &LayerSurface, _: LayerSurfaceConfigure, _: u32) {
        self.draw(layer);
    }
}

impl SeatHandler for App {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }
    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
    fn new_capability(&mut self, _: &Connection, qh: &QueueHandle<Self>, seat: wl_seat::WlSeat, capability: Capability) {
        if capability == Capability::Pointer && self.pointer.is_none() {
            self.pointer = self.seat_state.get_pointer(qh, &seat).ok();
        }
    }
    fn remove_capability(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat, capability: Capability) {
        if capability == Capability::Pointer {
            if let Some(p) = self.pointer.take() {
                p.release();
            }
        }
    }
    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl PointerHandler for App {
    fn pointer_frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_pointer::WlPointer, events: &[PointerEvent]) {
        for ev in events {
            match ev.kind {
                PointerEventKind::Enter { .. } => {
                    if let Some(corner) = self.corner_of(&ev.surface) {
                        if self.fired == Some(corner) {
                            continue;
                        }
                        self.cancel_pending();
                        let delay = Duration::from_millis(self.config.delay_ms);
                        let token = self
                            .loop_handle
                            .insert_source(Timer::from_duration(delay), move |_, _, app: &mut App| {
                                app.pending = None;
                                app.fired = Some(corner);
                                app.fire(corner);
                                TimeoutAction::Drop
                            })
                            .expect("timer");
                        self.pending = Some((corner, token));
                    }
                }
                PointerEventKind::Leave { .. } => {
                    self.cancel_pending();
                    self.fired = None;
                }
                _ => {}
            }
        }
    }
}

impl ShmHandler for App {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl ToplevelInfoHandler for App {
    fn toplevel_info_state(&mut self) -> &mut ToplevelInfoState {
        &mut self.toplevel_info_state
    }
    fn new_toplevel(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &ExtForeignToplevelHandleV1) {}
    fn update_toplevel(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &ExtForeignToplevelHandleV1) {}
    fn toplevel_closed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &ExtForeignToplevelHandleV1) {}
}

impl ToplevelManagerHandler for App {
    fn toplevel_manager_state(&mut self) -> &mut ToplevelManagerState {
        &mut self.toplevel_manager_state
    }
    fn capabilities(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: Vec<WEnum<zcosmic_toplevel_manager_v1::ZcosmicToplelevelManagementCapabilitiesV1>>,
    ) {
    }
}

impl WorkspaceHandler for App {
    fn workspace_state(&mut self) -> &mut WorkspaceState {
        &mut self.workspace_state
    }
    fn done(&mut self) {}
}

impl ProvidesRegistryState for App {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState, SeatState];
}

sctk::delegate_compositor!(App);
sctk::delegate_output!(App);
sctk::delegate_seat!(App);
sctk::delegate_pointer!(App);
sctk::delegate_layer!(App);
sctk::delegate_shm!(App);
sctk::delegate_registry!(App);
cctk::delegate_toplevel_info!(App);
cctk::delegate_toplevel_manager!(App);
cctk::delegate_workspace!(App);

fn main() {
    let conn = Connection::connect_to_env().expect("no Wayland display");
    let (globals, event_queue) = registry_queue_init::<App>(&conn).expect("registry");
    let qh = event_queue.handle();
    let mut event_loop: EventLoop<'static, App> = EventLoop::try_new().expect("event loop");
    WaylandSource::new(conn.clone(), event_queue).insert(event_loop.handle()).expect("wayland source");

    let registry_state = RegistryState::new(&globals);
    let shm = Shm::bind(&globals, &qh).expect("wl_shm");
    let pool = SlotPool::new(100 * 100 * 4, &shm).expect("shm pool");
    let config = Config::load();
    eprintln!(
        "clover-corners: size={} delay={}ms actions={:?}",
        config.size,
        config.delay_ms,
        config.actions.iter().map(|(c, a)| format!("{c:?}={a:?}")).collect::<Vec<_>>()
    );
    let mut app = App {
        output_state: OutputState::new(&globals, &qh),
        seat_state: SeatState::new(&globals, &qh),
        compositor_state: CompositorState::bind(&globals, &qh).expect("wl_compositor"),
        layer_shell: LayerShell::bind(&globals, &qh).expect("layer shell not available"),
        toplevel_info_state: ToplevelInfoState::new(&registry_state, &qh),
        toplevel_manager_state: ToplevelManagerState::new(&registry_state, &qh),
        workspace_state: WorkspaceState::new(&registry_state, &qh),
        registry_state,
        shm,
        pool,
        config,
        surfaces: Vec::new(),
        loop_handle: event_loop.handle(),
        pending: None,
        fired: None,
        hidden: Vec::new(),
        pointer: None,
    };
    loop {
        event_loop.dispatch(None, &mut app).expect("dispatch");
    }
}
