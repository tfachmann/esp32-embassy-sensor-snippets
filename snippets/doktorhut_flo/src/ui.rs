//! Menu state machine. The rotary task feeds `on_input`; the display reads
//! `view`. State lives behind a critical-section mutex (both run on core0,
//! access is synchronous with no await under the lock).

use core::cell::RefCell;

use critical_section::Mutex;

use crate::control;

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

pub const MAIN_ITEMS: [&str; 6] = ["BEER", "MUSIC", "FLUIDS", "TILT", "ABOUT", "CONTROLS"];
pub const CONTROL_ITEMS: [&str; 5] = ["Volume", "LED Speed", "LED Bright", "LED Effect", "Back"];

const MAIN_FLUIDS: usize = 2; // index of "FLUIDS" in MAIN_ITEMS
const MAIN_TILT: usize = 3; // index of "TILT" in MAIN_ITEMS
const MAIN_ABOUT: usize = 4; // index of "ABOUT" in MAIN_ITEMS
const MAIN_CONTROLS: usize = 5; // index of "CONTROLS" in MAIN_ITEMS
const CONTROLS_BACK: usize = 4; // index of "Back" in CONTROL_ITEMS

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
                    MAIN_CONTROLS => {
                        ui.screen = Screen::Controls;
                        ui.cursor = 0;
                    }
                    MAIN_FLUIDS => ui.screen = Screen::Fluids,
                    MAIN_TILT => ui.screen = Screen::Tilt,
                    MAIN_ABOUT => ui.screen = Screen::About,
                    _ => {} // BEER / MUSIC: no-op for now.
                },
            },
            Screen::Fluids => {
                // Rotation ignored; click returns to the main menu.
                if let Event::Click = ev {
                    ui.screen = Screen::Main;
                    ui.cursor = MAIN_FLUIDS;
                }
            }
            Screen::Tilt => {
                if let Event::Click = ev {
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
        3 => {
            if up {
                control::next_mode()
            } else {
                control::prev_mode()
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
