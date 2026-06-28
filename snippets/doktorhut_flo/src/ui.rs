//! Menu state machine. The rotary task feeds `on_input`; the display reads
//! `view`. State lives behind a critical-section mutex (both run on core0,
//! access is synchronous with no await under the lock).

use core::cell::RefCell;

use critical_section::Mutex;
use embassy_time::Instant;

use crate::control;

fn now_ms() -> u32 {
    Instant::now().as_millis() as u32
}

pub enum Event {
    Left,
    Right,
    Click,
    Hold, // 3s button hold (easter egg trigger)
}

#[derive(Clone, Copy, PartialEq)]
enum Screen {
    Main,
    Controls,
    Fluids,
    Tilt,
    About,
    BeerManual,
    Party,
}

#[derive(Clone, Copy, PartialEq)]
pub enum ViewScreen {
    Main,
    Controls,
    Fluids,
    Tilt,
    About,
    BeerManual,
    Party,
}

pub const MAIN_ITEMS: [&str; 8] = [
    "BEER", "BEER MAN", "MUSIC", "IMU", "FLUIDS", "TILT", "ABOUT", "CONTROLS",
];
pub const CONTROL_ITEMS: [&str; 4] = ["Volume", "LED Speed", "LED Bright", "Back"];

/// The first PROCESS_COUNT MAIN_ITEMS are "processes" (highlight selection +
/// status rects); the rest are normal entries (">" cursor).
pub const PROCESS_COUNT: usize = 6;

/// Index of the IMU process in MAIN_ITEMS (its status rect blinks while ramping).
pub const MAIN_IMU: usize = 3;

const MAIN_BEER: usize = 0;
const MAIN_BEERMAN: usize = 1;
const MAIN_MUSIC: usize = 2;
const MAIN_FLUIDS: usize = 4;
const MAIN_TILT: usize = 5;
const MAIN_ABOUT: usize = 6;
const MAIN_CONTROLS: usize = 7;
const CONTROLS_BACK: usize = 3; // index of "Back" in CONTROL_ITEMS

struct Ui {
    screen: Screen,
    cursor: usize,
    editing: bool,
    pending: Option<Screen>, // screen to auto-enter once the IMU is ready
}

static UI: Mutex<RefCell<Ui>> = Mutex::new(RefCell::new(Ui {
    screen: Screen::Main,
    cursor: 0,
    editing: false,
    pending: None,
}));

/// Snapshot for the display.
pub struct View {
    pub screen: ViewScreen,
    pub cursor: usize,
    pub editing: bool,
}

pub fn view() -> View {
    critical_section::with(|cs| {
        let ui = UI.borrow_ref(cs);
        View {
            screen: match ui.screen {
                Screen::Main => ViewScreen::Main,
                Screen::Controls => ViewScreen::Controls,
                Screen::Fluids => ViewScreen::Fluids,
                Screen::Tilt => ViewScreen::Tilt,
                Screen::About => ViewScreen::About,
                Screen::BeerManual => ViewScreen::BeerManual,
                Screen::Party => ViewScreen::Party,
            },
            cursor: ui.cursor,
            editing: ui.editing,
        }
    })
}

pub fn on_input(ev: Event) {
    critical_section::with(|cs| {
        let mut ui = UI.borrow_ref_mut(cs);
        match ui.screen {
            Screen::Main => {
                ui.pending = None; // any menu interaction cancels a pending auto-launch
                match ev {
                Event::Left => ui.cursor = wrap_prev(ui.cursor, MAIN_ITEMS.len()),
                Event::Right => ui.cursor = wrap_next(ui.cursor, MAIN_ITEMS.len()),
                Event::Click => match ui.cursor {
                    MAIN_BEER => control::start_beer(),
                    MAIN_MUSIC => control::toggle_music(),
                    MAIN_IMU => control::toggle_imu(now_ms()),
                    MAIN_FLUIDS => enter_with_imu(&mut ui, Screen::Fluids),
                    MAIN_TILT => enter_with_imu(&mut ui, Screen::Tilt),
                    MAIN_BEERMAN => {
                        control::set_manual_active(true);
                        ui.screen = Screen::BeerManual;
                    }
                    MAIN_ABOUT => ui.screen = Screen::About,
                    MAIN_CONTROLS => {
                        ui.screen = Screen::Controls;
                        ui.cursor = 0;
                    }
                    _ => {}
                },
                Event::Hold => {}
                }
            }
            Screen::Fluids => {
                // Rotation ignored; click returns to the main menu.
                if let Event::Click = ev {
                    control::set_fluids_active(false);
                    ui.screen = Screen::Main;
                    ui.cursor = MAIN_FLUIDS;
                }
            }
            Screen::Tilt => {
                if let Event::Click = ev {
                    control::set_tilt_active(false);
                    ui.screen = Screen::Main;
                    ui.cursor = MAIN_TILT;
                }
            }
            Screen::About => match ev {
                Event::Click => {
                    ui.screen = Screen::Main;
                    ui.cursor = MAIN_ABOUT;
                }
                // Hidden easter egg: hold 3s -> PARTY (nyancat + music + party LEDs).
                Event::Hold => {
                    control::set_party(true);
                    control::set_music(true);
                    control::set_imu(true, now_ms());
                    ui.screen = Screen::Party;
                }
                _ => {}
            },
            Screen::Party => match ev {
                // Rotation adjusts LED brightness + volume together.
                Event::Right => {
                    control::brighter();
                    control::volume_up();
                }
                Event::Left => {
                    control::dimmer();
                    control::volume_down();
                }
                // Click fires the automatic beer pour instantly; exit via 3s hold.
                Event::Click => control::signal_beer_arrived(),
                Event::Hold => {
                    control::set_party(false);
                    control::set_music(false);
                    control::set_imu(false, now_ms());
                    ui.screen = Screen::Main;
                    ui.cursor = MAIN_ABOUT;
                }
            },
            Screen::BeerManual => match ev {
                // Rotation drives the servo and flows a (fast) beer byte; click exits.
                Event::Left => {
                    control::servo_step(true);
                    control::start_beer();
                }
                Event::Right => {
                    control::servo_step(false);
                    control::start_beer();
                }
                Event::Click => {
                    control::set_manual_active(false);
                    ui.screen = Screen::Main;
                    ui.cursor = MAIN_BEERMAN;
                }
                Event::Hold => {}
            },
            Screen::Controls if ui.editing => match ev {
                Event::Left => edit_value(ui.cursor, false),
                Event::Right => edit_value(ui.cursor, true),
                Event::Click => ui.editing = false,
                Event::Hold => {}
            },
            Screen::Controls => match ev {
                Event::Left => ui.cursor = wrap_prev(ui.cursor, CONTROL_ITEMS.len()),
                Event::Right => ui.cursor = wrap_next(ui.cursor, CONTROL_ITEMS.len()),
                Event::Click => {
                    if ui.cursor == CONTROLS_BACK {
                        ui.screen = Screen::Main;
                        ui.cursor = MAIN_CONTROLS;
                    } else {
                        ui.editing = true;
                    }
                }
                Event::Hold => {}
            },
        }
    });
}

fn activate(target: Screen) {
    match target {
        Screen::Fluids => control::set_fluids_active(true),
        Screen::Tilt => control::set_tilt_active(true),
        _ => {}
    }
}

/// Enter a screen that needs the IMU. If the IMU is ready, enter now; otherwise
/// start it and stay on the menu (the IMU block blinks while ramping). Once
/// ready, `poll()` auto-enters the pending screen.
fn enter_with_imu(ui: &mut Ui, target: Screen) {
    if control::imu_ready(now_ms()) {
        activate(target);
        ui.screen = target;
    } else {
        if !control::imu_on() {
            control::set_imu(true, now_ms());
        }
        ui.pending = Some(target);
    }
}

/// Called each display frame: auto-enter a pending IMU screen once ready.
pub fn poll() {
    critical_section::with(|cs| {
        let mut ui = UI.borrow_ref_mut(cs);
        if let Some(target) = ui.pending {
            if control::imu_ready(now_ms()) {
                activate(target);
                ui.screen = target;
                ui.pending = None;
            }
        }
    });
}

fn edit_value(item: usize, up: bool) {
    match item {
        0 => {
            if up {
                control::volume_up()
            } else {
                control::volume_down()
            }
        }
        1 => {
            if up {
                control::faster()
            } else {
                control::slower()
            }
        }
        2 => {
            if up {
                control::brighter()
            } else {
                control::dimmer()
            }
        }
        _ => {}
    }
}

fn wrap_next(i: usize, len: usize) -> usize {
    (i + 1) % len
}

fn wrap_prev(i: usize, len: usize) -> usize {
    (i + len - 1) % len
}
