//! The macOS menu bar: the ribbon as native menus at the top of the screen (`menu_bar` in the UI
//! crate describes them; this builds them with `muda`, whose API is safe).
//!
//! Built on the first frame, after AppKit has finished launching (winit sets up its own default
//! menu then). A few times a second the description is rebuilt and compared: a change of
//! language, ribbon or shortcuts rebuilds the menus, otherwise only the items' enabled states are
//! updated. Chosen items arrive through `muda`'s event handler, which wakes the UI.

use std::collections::HashMap;
use std::sync::mpsc::{Receiver, channel};

use muda::accelerator::{Accelerator, Code, Modifiers};
use muda::{AboutMetadata, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use wordcraft_ui_egui::WordApp;
use wordcraft_ui_egui::menu_bar::{Action, Chord, Item, MenuBar, Role, Standard};

/// How often the menus are checked against the app's state.
const REFRESH_SECONDS: f64 = 0.25;

pub struct NativeMenu {
    events: Receiver<String>,
    built: Option<Built>,
    last_check: f64,
}

/// The native menus as built, kept alive (AppKit doesn't own `muda`'s targets), with each
/// action's item and its last enabled state.
struct Built {
    _menu: Menu,
    model: MenuBar,
    items: HashMap<String, (MenuItem, Action, bool)>,
}

impl NativeMenu {
    /// Route chosen items to the UI thread and wake it.
    pub fn new(ctx: &egui::Context) -> Self {
        let (tx, events) = channel();
        let ctx = ctx.clone();
        MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
            let _ = tx.send(e.id.0);
            ctx.request_repaint();
        }));
        NativeMenu { events, built: None, last_check: f64::NEG_INFINITY }
    }

    /// Once a frame: run chosen items, and every so often bring the menus up to date.
    pub fn update(&mut self, app: &mut WordApp, ctx: &egui::Context) {
        while let Ok(key) = self.events.try_recv() {
            let action = self.built.as_ref().and_then(|b| b.items.get(&key)).map(|(_, a, _)| a.clone());
            if let Some(action) = action
                && app.menu_enabled(&action)
            {
                app.menu_activate(ctx, &action);
            }
        }
        let now = ctx.input(|i| i.time);
        if now - self.last_check < REFRESH_SECONDS {
            return;
        }
        self.last_check = now;
        let model = app.menu_bar();
        match &mut self.built {
            Some(b) if b.model == model => {
                for (item, action, enabled) in b.items.values_mut() {
                    let on = app.menu_enabled(action);
                    if on != *enabled {
                        item.set_enabled(on);
                        *enabled = on;
                    }
                }
            }
            _ => match build(&model, app) {
                Ok(b) => self.built = Some(b),
                Err(e) => log::warn!("menu bar: {e}"),
            },
        }
    }
}

fn build(model: &MenuBar, app: &WordApp) -> Result<Built, muda::Error> {
    let menu = Menu::new();
    let mut items = HashMap::new();
    let mut windows = None;
    let mut help = None;
    for m in &model.menus {
        let sub = Submenu::new(&m.title, true);
        fill(&sub, &m.items, app, &mut items)?;
        menu.append(&sub)?;
        match m.role {
            Role::Window => windows = Some(sub),
            Role::Help => help = Some(sub),
            Role::App | Role::Normal => {}
        }
    }
    menu.init_for_nsapp();
    // The Window menu lists the open windows; the Help menu gets the system's menu search.
    if let Some(w) = &windows {
        w.set_as_windows_menu_for_nsapp();
    }
    if let Some(h) = &help {
        h.set_as_help_menu_for_nsapp();
    }
    Ok(Built { _menu: menu, model: model.clone(), items })
}

fn fill(sub: &Submenu, entries: &[Item], app: &WordApp, items: &mut HashMap<String, (MenuItem, Action, bool)>) -> Result<(), muda::Error> {
    for e in entries {
        match e {
            Item::Action(a) => {
                let on = app.menu_enabled(a);
                let item = MenuItem::with_id(a.key.as_str(), &a.label, on, a.chord.and_then(accelerator));
                sub.append(&item)?;
                items.insert(a.key.clone(), (item, a.clone(), on));
            }
            Item::Submenu { title, items: inner } => {
                let s = Submenu::new(title, true);
                fill(&s, inner, app, items)?;
                sub.append(&s)?;
            }
            Item::Separator => sub.append(&PredefinedMenuItem::separator())?,
            Item::Standard(s) => sub.append(&standard(*s))?,
        }
    }
    Ok(())
}

fn standard(s: Standard) -> PredefinedMenuItem {
    match s {
        Standard::About => PredefinedMenuItem::about(
            None,
            Some(AboutMetadata { name: Some("WordCraft".into()), version: Some(env!("CARGO_PKG_VERSION").into()), ..Default::default() }),
        ),
        Standard::Services => PredefinedMenuItem::services(None),
        Standard::Hide => PredefinedMenuItem::hide(None),
        Standard::HideOthers => PredefinedMenuItem::hide_others(None),
        Standard::ShowAll => PredefinedMenuItem::show_all(None),
        Standard::Minimize => PredefinedMenuItem::minimize(None),
        Standard::Zoom => PredefinedMenuItem::maximize(None),
        Standard::Fullscreen => PredefinedMenuItem::fullscreen(None),
        Standard::BringAllToFront => PredefinedMenuItem::bring_all_to_front(None),
    }
}

/// A menu chord as an AppKit key equivalent, or `None` for a key a menu can't show.
fn accelerator(c: Chord) -> Option<Accelerator> {
    let mut mods = Modifiers::empty();
    if c.command {
        mods |= Modifiers::META;
    }
    if c.shift {
        mods |= Modifiers::SHIFT;
    }
    if c.alt {
        mods |= Modifiers::ALT;
    }
    if c.ctrl {
        mods |= Modifiers::CONTROL;
    }
    Some(Accelerator::new(mods, code(c.key)?))
}

fn code(k: egui::Key) -> Option<Code> {
    use egui::Key as K;
    Some(match k {
        K::A => Code::KeyA,
        K::B => Code::KeyB,
        K::C => Code::KeyC,
        K::D => Code::KeyD,
        K::E => Code::KeyE,
        K::F => Code::KeyF,
        K::G => Code::KeyG,
        K::H => Code::KeyH,
        K::I => Code::KeyI,
        K::J => Code::KeyJ,
        K::K => Code::KeyK,
        K::L => Code::KeyL,
        K::M => Code::KeyM,
        K::N => Code::KeyN,
        K::O => Code::KeyO,
        K::P => Code::KeyP,
        K::Q => Code::KeyQ,
        K::R => Code::KeyR,
        K::S => Code::KeyS,
        K::T => Code::KeyT,
        K::U => Code::KeyU,
        K::V => Code::KeyV,
        K::W => Code::KeyW,
        K::X => Code::KeyX,
        K::Y => Code::KeyY,
        K::Z => Code::KeyZ,
        K::Num0 => Code::Digit0,
        K::Num1 => Code::Digit1,
        K::Num2 => Code::Digit2,
        K::Num3 => Code::Digit3,
        K::Num4 => Code::Digit4,
        K::Num5 => Code::Digit5,
        K::Num6 => Code::Digit6,
        K::Num7 => Code::Digit7,
        K::Num8 => Code::Digit8,
        K::Num9 => Code::Digit9,
        K::Comma => Code::Comma,
        K::Period => Code::Period,
        K::Minus => Code::Minus,
        K::Equals | K::Plus => Code::Equal,
        K::OpenBracket => Code::BracketLeft,
        K::CloseBracket => Code::BracketRight,
        K::Slash | K::Questionmark => Code::Slash,
        K::Semicolon => Code::Semicolon,
        K::Quote => Code::Quote,
        K::Backslash => Code::Backslash,
        K::Backtick => Code::Backquote,
        K::F1 => Code::F1,
        K::F2 => Code::F2,
        K::F3 => Code::F3,
        K::F4 => Code::F4,
        K::F5 => Code::F5,
        K::F6 => Code::F6,
        K::F7 => Code::F7,
        K::F8 => Code::F8,
        K::F9 => Code::F9,
        K::F10 => Code::F10,
        K::F11 => Code::F11,
        K::F12 => Code::F12,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::{accelerator, code};
    use wordcraft_ui_egui::menu_bar::Chord;

    /// Every key a menu chord can name has an AppKit key equivalent.
    #[test]
    fn menu_chords_have_key_equivalents() {
        for name in ["Mod+B", "Mod+Shift+S", "Mod+,", "Mod+=", "Mod+[", "Mod+Alt+1", "Ctrl+Mod+F", "F7"] {
            let c = Chord::parse(name).unwrap_or_else(|| panic!("{name} parses"));
            assert!(code(c.key).is_some() && accelerator(c).is_some(), "{name}");
        }
    }
}
