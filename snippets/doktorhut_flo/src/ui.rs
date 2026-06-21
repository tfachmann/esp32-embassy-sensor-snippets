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
}

#[derive(Clone, Copy, PartialEq)]
enum Screen {
    Main,
    Controls,
    Fluids,
    Tilt,
    About,
}

#[derive(Clone, Copy, PartialEq)]
pub enum ViewScreen {
    Main,
    Controls,
    Fluids,
    Tilt,
    About,
}

pub const MAIN_ITEMS: [&str; 7] =
    ["BEER", "MUSIC", "IMU", "FLUIDS", "TILT", "ABOUT", "CONTROLS"];
pub const CONTROL_ITEMS: [&str; 4] = ["Volume", "LED Speed", "LED Bright", "Back"];

/// The first PROCESS_COUNT MAIN_ITEMS are "processes" (highlight selection +
/// status rects); the rest are normal entries (">" cursor).
pub const PROCESS_COUNT: usize = 5;

const MAIN_BEER: usize = 0;
const MAIN_MUSIC: usize = 1;
const MAIN_IMU: usize = 2;
const MAIN_FLUIDS: usize = 3;
const MAIN_TILT: usize = 4;
const MAIN_ABOUT: usize = 5;
const MAIN_CONTROLS: usize = 6;
const CONTROLS_BACK: usize = 3; // index of "Back" in CONTROL_ITEMS

struct Ui {
    screen: Screen,
    cursor: usize,
    editing: bool,
}

static UI: Mutex<RefCell<Ui>> = Mutex::new(RefCell::new(Ui {
    screen: Screen::Main,
    cursor: 0,
    editing: false,
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
            Screen::Main => match ev {
                Event::Left => ui.cursor = wrap_prev(ui.cursor, MAIN_ITEMS.len()),
                Event::Right => ui.cursor = wrap_next(ui.cursor, MAIN_ITEMS.len()),
                Event::Click => match ui.cursor {
                    MAIN_BEER => control::start_beer(),
                    MAIN_MUSIC => control::toggle_music(),
                    MAIN_IMU => control::toggle_imu(now_ms()),
                    MAIN_FLUIDS => enter_with_imu(&mut ui, Screen::Fluids),
                    MAIN_TILT => enter_with_imu(&mut ui, Screen::Tilt),
                    MAIN_ABOUT => ui.screen = Screen::About,
                    MAIN_CONTROLS => {
                        ui.screen = Screen::Controls;
                        ui.cursor = 0;
                    }
                    _ => {}
                },
            },
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
            Screen::About => {
                if let Event::Click = ev {
                    ui.screen = Screen::Main;
                    ui.cursor = MAIN_ABOUT;
                }
            }
            Screen::Controls if ui.editing => match ev {
                Event::Left => edit_value(ui.cursor, false),
                Event::Right => edit_value(ui.cursor, true),
                Event::Click => ui.editing = false,
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
            },
        }
    });
}

/// Enter a screen that needs the IMU. If the IMU is ready, enter it; otherwise
/// (first click) just start the IMU and stay on the menu -- the IMU status block
/// animates "not ready" during the ramp. Click again once ready to enter.
fn enter_with_imu(ui: &mut Ui, target: Screen) {
    if control::imu_ready(now_ms()) {
        match target {
            Screen::Fluids => control::set_fluids_active(true),
            Screen::Tilt => control::set_tilt_active(true),
            _ => {}
        }
        ui.screen = target;
    } else if !control::imu_on() {
        control::set_imu(true, now_ms()); // start ramping; stay on the menu
    }
    // ramping (on but not ready): stay on the menu, ignore.
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
