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
}

pub const MAIN_ITEMS: [&str; 3] = ["BEER", "MUSIC", "CONTROLS"];
pub const CONTROL_ITEMS: [&str; 5] = ["Volume", "LED Speed", "LED Bright", "LED Effect", "Back"];

const MAIN_CONTROLS: usize = 2; // index of "CONTROLS" in MAIN_ITEMS
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
    pub in_controls: bool,
    pub cursor: usize,
    pub editing: bool,
}

pub fn view() -> View {
    critical_section::with(|cs| {
        let ui = UI.borrow_ref(cs);
        View {
            in_controls: ui.screen == Screen::Controls,
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
                Event::Click => {
                    if ui.cursor == MAIN_CONTROLS {
                        ui.screen = Screen::Controls;
                        ui.cursor = 0;
                    }
                    // BEER / MUSIC: no-op for now.
                }
            },
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
