//! The ribbon as a menu bar, for hosts that have one (the macOS menu bar at the top of the screen).
//!
//! The menus mirror what the ribbon shows: a menu per visible tab (custom tabs and groups too) with
//! the tab's controls in drawing order (`keytips` lists them, as it does for Alt badges), separated
//! by ribbon group. Around them are the menus a Mac app always has: the app menu, File, Edit,
//! Window and Help. Labels follow the interface language, shortcuts follow Customize Keyboard, and
//! a menu item runs exactly what its ribbon button runs ([`WordApp::run`]).
//!
//! This module only describes the menus; the host builds them natively ([`MenuBar`]), asks
//! [`WordApp::menu_enabled`] for their state and hands chosen items to [`WordApp::menu_activate`].

use egui::{Key, Modifiers};
use serde_json::{Value, json};

use crate::WordApp;
use crate::keytips::{Target, menu_controls};

/// The whole menu bar, left to right.
#[derive(Clone, Debug, PartialEq)]
pub struct MenuBar {
    pub menus: Vec<Menu>,
}

/// One menu in the bar.
#[derive(Clone, Debug, PartialEq)]
pub struct Menu {
    pub title: String,
    pub role: Role,
    pub items: Vec<Item>,
}

/// Menus the system treats specially: the app menu comes first, the Window menu lists the
/// windows, and the Help menu gets the system's menu search.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    App,
    Normal,
    Window,
    Help,
}

/// A menu entry.
#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    Action(Action),
    Submenu {
        title: String,
        items: Vec<Item>,
    },
    Separator,
    /// An item the system provides (Hide, Services, Minimize…).
    Standard(Standard),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Standard {
    About,
    Services,
    Hide,
    HideOthers,
    ShowAll,
    Minimize,
    Zoom,
    Fullscreen,
    BringAllToFront,
}

/// What a menu item does.
#[derive(Clone, Debug, PartialEq)]
pub enum Does {
    /// Run a command, as its ribbon button does.
    Run { id: String, params: Value },
    /// Quit the app, asking about unsaved changes first (as the window's close button does).
    Quit,
}

/// A menu item that does something.
#[derive(Clone, Debug, PartialEq)]
pub struct Action {
    /// Stable and unique in the bar: the host's item id.
    pub key: String,
    pub label: String,
    /// The keyboard shortcut shown and bound, if any.
    pub chord: Option<Chord>,
    pub does: Does,
}

/// A keyboard shortcut a menu can show and bind: Command (or Ctrl elsewhere), Shift, Option,
/// Control and one key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Chord {
    pub command: bool,
    pub shift: bool,
    pub alt: bool,
    pub ctrl: bool,
    pub key: Key,
}

impl Chord {
    /// A registry shortcut (`Mod+Shift+S`, `Mod+Ctrl+F`, `F7`). Menus only take chords with
    /// Command or Control, or function keys: plain keys stay with the text being typed, and the
    /// editing keys (arrows, Delete, Return…) stay with the editor and text fields.
    pub fn parse(sc: &str) -> Option<Chord> {
        let mut c = Chord { command: false, shift: false, alt: false, ctrl: false, key: Key::A };
        let mut key = None;
        for part in sc.split('+') {
            match part {
                "Mod" => c.command = true,
                "Shift" => c.shift = true,
                "Alt" => c.alt = true,
                "Ctrl" => c.ctrl = true,
                // `Mod+Shift++` style plus keys don't occur; anything else is the key.
                k => key = Key::ALL.iter().copied().find(|x| crate::keys::key_name(*x) == Some(k)),
            }
        }
        c.key = key?;
        let function = c.key.name().starts_with('F') && c.key.name().len() > 1;
        let editing = matches!(
            c.key,
            Key::ArrowLeft
                | Key::ArrowRight
                | Key::ArrowUp
                | Key::ArrowDown
                | Key::Home
                | Key::End
                | Key::PageUp
                | Key::PageDown
                | Key::Backspace
                | Key::Delete
                | Key::Enter
                | Key::Tab
                | Key::Escape
                | Key::Space
                | Key::Insert
        );
        ((c.command || c.ctrl || function) && !editing).then_some(c)
    }

    /// The modifiers egui reports for this chord (Command is egui's `command`, ⌘ on macOS).
    pub fn modifiers(self) -> Modifiers {
        let mac = cfg!(target_os = "macos");
        Modifiers { alt: self.alt, ctrl: self.ctrl || (self.command && !mac), shift: self.shift, mac_cmd: self.command && mac, command: self.command }
    }
}

/// Keys the system's own menu items keep on macOS (Hide, Hide Others, Minimize, Quit, Settings,
/// Full Screen): a ribbon command bound to one of them shows no shortcut in the menu bar.
fn system_chords() -> Vec<Chord> {
    let c = |shift, alt, ctrl, key| Chord { command: true, shift, alt, ctrl, key };
    vec![
        c(false, false, false, Key::H),
        c(false, true, false, Key::H),
        c(false, false, false, Key::M),
        c(false, false, false, Key::Q),
        c(false, false, false, Key::Comma),
        c(false, false, true, Key::F),
    ]
}

/// Builds the bar, giving each action a unique key and each chord to its first item only.
struct Builder<'a> {
    app: &'a WordApp,
    used: Vec<Chord>,
    keys: Vec<String>,
}

impl Builder<'_> {
    fn action(&mut self, label: String, chord: Option<Chord>, does: Does) -> Item {
        let chord = chord.filter(|c| !self.used.contains(c));
        if let Some(c) = chord {
            self.used.push(c);
        }
        let base = match &does {
            Does::Run { id, params } if params.as_object().is_none_or(|m| m.is_empty()) => id.clone(),
            Does::Run { id, params } => format!("{id} {params}"),
            Does::Quit => "app.quit".into(),
        };
        let mut key = base.clone();
        let mut n = 2;
        while self.keys.contains(&key) {
            key = format!("{base} #{n}");
            n += 1;
        }
        self.keys.push(key.clone());
        Item::Action(Action { key, label, chord, does })
    }

    /// The first shortcut the user's keymap gives a command, as a menu chord.
    fn chord_for(&self, id: &str) -> Option<Chord> {
        self.app.session.keymap.keys_for(&self.app.session.registry, id).iter().find_map(|k| Chord::parse(k))
    }

    /// A command by registry id, labelled as the registry (or `label`) says; `None` if this build
    /// doesn't have it.
    fn command(&mut self, id: &str, label: Option<&str>) -> Option<Item> {
        let spec = self.app.session.registry.get(id)?;
        let label = format!("{}{}", label.map_or_else(|| tl!(spec.label).to_string(), |l| tl!(l).to_string()), ellipsis(id, &json!({})));
        let chord = self.chord_for(id);
        Some(self.action(label, chord, Does::Run { id: id.to_string(), params: json!({}) }))
    }

    /// A ribbon control: its tooltip name, the command or dialog it runs, and its shortcut when it
    /// runs a command without parameters.
    fn control(&mut self, tip: &str, target: Target) -> Item {
        let (id, params) = match target {
            Target::Cmd { id, params } => (id.to_string(), params),
            Target::Dialog { name } => ("ui.dialog".to_string(), json!({ "name": name })),
            Target::Tab(tab) => ("ui.tab".to_string(), json!({ "tab": tab })),
        };
        let plain = params.as_object().is_none_or(|m| m.is_empty());
        let chord = if plain { self.chord_for(&id) } else { None };
        let label = format!("{}{}", tl!(tip), ellipsis(&id, &params));
        self.action(label, chord, Does::Run { id, params })
    }

    /// A ribbon tab's menu items: its controls separated by group, then its custom groups.
    fn tab_items(&mut self, tab: &str, custom_groups: &[wordcraft_engine::ribbon::CustomGroup]) -> Vec<Item> {
        let mut items = Vec::new();
        let controls = menu_controls(tab);
        if controls.is_empty() {
            // Tabs without a control list (Developer, the contextual Format tabs): the registry's
            // commands that live there and need no input.
            let mut group = String::new();
            let specs: Vec<_> = self
                .app
                .session
                .registry
                .all()
                .iter()
                .filter(|s| (s.location == tab || s.location.starts_with(&format!("{tab} › "))) && s.params == "{}")
                .map(|s| (s.id, s.label, s.location))
                .collect();
            for (id, label, location) in specs {
                let g = location.split(" › ").nth(1).unwrap_or("").to_string();
                if g != group && !items.is_empty() {
                    items.push(Item::Separator);
                }
                group = g;
                if let Some(item) = self.command(id, Some(label)) {
                    items.push(item);
                }
            }
        } else {
            let mut group: Option<String> = None;
            for (tip, target) in controls {
                // The ribbon group a command lives in (`Home › Font` → Font); dialogs and UI
                // commands stay with the group before them.
                let g = match &target {
                    Target::Cmd { id, .. } => self.app.session.registry.get(id).and_then(|s| s.location.split(" › ").nth(1)).map(str::to_string),
                    _ => None,
                };
                if let Some(g) = g {
                    if group.as_ref().is_some_and(|old| *old != g) {
                        items.push(Item::Separator);
                    }
                    group = Some(g);
                }
                items.push(self.control(tip, target));
            }
        }
        for g in custom_groups {
            let cmds: Vec<Item> = g.commands.iter().filter_map(|id| self.command(id, None)).collect();
            if cmds.is_empty() {
                continue;
            }
            if !items.is_empty() {
                items.push(Item::Separator);
            }
            items.push(Item::Submenu { title: g.name.clone(), items: cmds });
        }
        items
    }
}

/// File › Open Recent: the documents File › Home lists, by name (with the folder when two share a
/// name).
fn open_recent(b: &mut Builder<'_>) -> Item {
    let paths: Vec<String> = b.app.ui.recent.iter().take(b.app.ui.recent_shown()).cloned().collect();
    let name = |p: &str| std::path::Path::new(p).file_name().map_or_else(|| p.to_string(), |n| n.to_string_lossy().to_string());
    let mut items = Vec::new();
    for p in &paths {
        let n = name(p);
        let label = if paths.iter().filter(|q| name(q) == n).count() > 1 {
            let folder = std::path::Path::new(p).parent().and_then(|d| d.file_name()).map(|d| d.to_string_lossy().to_string()).unwrap_or_default();
            format!("{n} — {folder}")
        } else {
            n
        };
        items.push(b.action(label, None, Does::Run { id: "file.open".into(), params: json!({ "path": p }) }));
    }
    Item::Submenu { title: tl!("Open Recent").to_string(), items }
}

/// `…` after an item that asks for more before it acts (a dialog), as Mac menus write it.
fn ellipsis(id: &str, params: &Value) -> &'static str {
    let dialog = id == "ui.dialog" || id.ends_with("Dialog") || crate::input_dialog(id, params).is_some();
    if dialog { "…" } else { "" }
}

impl WordApp {
    /// The menu bar as it stands: the interface language, the user's ribbon (hidden and custom
    /// tabs) and keyboard shortcuts. Cheap enough to rebuild a few times a second and compare.
    pub fn menu_bar(&self) -> MenuBar {
        let mut b = Builder { app: self, used: system_chords(), keys: Vec::new() };
        let mut menus = Vec::new();
        let run = |id: &str, params: Value| Does::Run { id: id.to_string(), params };

        // The app menu: About, Settings, Services, Hide, Quit.
        let settings = b.action(
            format!("{}…", tl!("Settings")),
            Some(Chord { command: true, shift: false, alt: false, ctrl: false, key: Key::Comma }),
            run("ui.backstage", json!({"value": true, "page": "options"})),
        );
        let quit = b.action(
            crate::i18n::fmt(tl!("Quit {app}"), &[("app", "WordCraft")]),
            Some(Chord { command: true, shift: false, alt: false, ctrl: false, key: Key::Q }),
            Does::Quit,
        );
        menus.push(Menu {
            title: "WordCraft".into(),
            role: Role::App,
            items: vec![
                Item::Standard(Standard::About),
                Item::Separator,
                settings,
                Item::Separator,
                Item::Standard(Standard::Services),
                Item::Separator,
                Item::Standard(Standard::Hide),
                Item::Standard(Standard::HideOthers),
                Item::Standard(Standard::ShowAll),
                Item::Separator,
                quit,
            ],
        });

        // File and Edit, in the order Mac apps use.
        let section = |b: &mut Builder<'_>, ids: &[&str]| -> Vec<Item> { ids.iter().filter_map(|id| b.command(id, None)).collect() };
        let mut file = section(&mut b, &["file.new", "file.newFromTemplate", "file.open"]);
        file.push(open_recent(&mut b));
        for ids in [
            &["file.close", "file.save", "file.saveAs", "file.saveTemplate", "file.versions"][..],
            &["file.share", "file.exportPdf", "file.exportPng"],
            &["file.properties", "file.protect", "file.inspect"],
            &["file.print"],
        ] {
            let s = section(&mut b, ids);
            if !s.is_empty() {
                if !file.is_empty() {
                    file.push(Item::Separator);
                }
                file.extend(s);
            }
        }
        menus.push(Menu { title: tl!("File").to_string(), role: Role::Normal, items: file });
        let mut edit = section(&mut b, &["edit.undo", "edit.redo"]);
        edit.push(Item::Separator);
        edit.extend(section(&mut b, &["edit.cut", "edit.copy", "edit.paste", "edit.pasteSpecial", "select.all"]));
        edit.push(Item::Separator);
        edit.extend(section(&mut b, &["edit.find", "edit.findReplace", "edit.goTo"]));
        menus.push(Menu { title: tl!("Edit").to_string(), role: Role::Normal, items: edit });

        // The ribbon's tabs, as the user arranged them; Help comes last, as the Help menu.
        let tabs = self.session.ribbon.tabs().into_owned();
        for entry in tabs.iter().filter(|t| !t.hidden && t.name != "Help") {
            let title = if entry.custom { entry.name.clone() } else { tl!(&entry.name).to_string() };
            let items = if entry.custom { b.tab_items("", &entry.groups) } else { b.tab_items(&entry.name, &entry.groups) };
            if !items.is_empty() {
                menus.push(Menu { title, role: Role::Normal, items });
            }
        }
        // The contextual tabs, always there (their items are off until the selection fits).
        let table: Vec<Item> = ["Table Layout", "Table Design"]
            .into_iter()
            .map(|t| Item::Submenu { title: tl!(t).to_string(), items: b.tab_items(t, &[]) })
            .filter(|i| matches!(i, Item::Submenu { items, .. } if !items.is_empty()))
            .collect();
        if !table.is_empty() {
            menus.push(Menu { title: tl!("Table").to_string(), role: Role::Normal, items: table });
        }
        let format: Vec<Item> = ["Picture Format", "Shape Format", "Chart Design", "SmartArt Design", "Equation"]
            .into_iter()
            .map(|t| Item::Submenu { title: tl!(t).to_string(), items: b.tab_items(t, &[]) })
            .filter(|i| matches!(i, Item::Submenu { items, .. } if !items.is_empty()))
            .collect();
        if !format.is_empty() {
            menus.push(Menu { title: tl!("Format").to_string(), role: Role::Normal, items: format });
        }

        menus.push(Menu {
            title: tl!("Window").to_string(),
            role: Role::Window,
            items: vec![
                Item::Standard(Standard::Minimize),
                Item::Standard(Standard::Zoom),
                Item::Separator,
                Item::Standard(Standard::Fullscreen),
                Item::Separator,
                Item::Standard(Standard::BringAllToFront),
            ],
        });
        let help_groups = tabs.iter().find(|t| t.name == "Help").map(|t| t.groups.clone()).unwrap_or_default();
        menus.push(Menu { title: tl!("Help").to_string(), role: Role::Help, items: b.tab_items("Help", &help_groups) });
        MenuBar { menus }
    }

    /// Whether a menu item can be chosen now. While a dialog is open only the editing commands
    /// (which reach the dialog's text fields) and the app's own items are on, as with a Mac sheet.
    pub fn menu_enabled(&self, action: &Action) -> bool {
        match &action.does {
            Does::Quit => true,
            Does::Run { id, params } => {
                if self.dialog.is_some() && !edit_in_text_field(id) && id != "ui.backstage" {
                    return false;
                }
                match id.as_str() {
                    "ui.dialog" | "ui.tab" | "ui.backstage" => true,
                    _ if crate::input_dialog(id, params).is_some() => crate::widgets::enabled(self, id),
                    _ => crate::widgets::enabled(self, id),
                }
            }
        }
    }

    /// Do what a menu item says. With a text field focused (a dialog's, the font box, Search), an
    /// item with a shortcut acts as its key would, so Copy copies the field's text rather than the
    /// document's; otherwise it runs its command, as the ribbon does.
    pub fn menu_activate(&mut self, ctx: &egui::Context, action: &Action) {
        let Does::Run { id, params } = &action.does else {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        };
        let text_field = ctx.egui_wants_keyboard_input() && !self.canvas.focused;
        if text_field && let Some(chord) = action.chord {
            match id.as_str() {
                "edit.copy" => self.synthetic.push(egui::Event::Copy),
                "edit.cut" => self.synthetic.push(egui::Event::Cut),
                "edit.paste" => {
                    if let Some(text) = self.services.clipboard_text.as_ref().and_then(|f| f()) {
                        self.synthetic.push(egui::Event::Paste(text));
                    }
                }
                _ => {
                    let m = chord.modifiers();
                    self.synthetic.push(egui::Event::Key { key: chord.key, physical_key: None, pressed: true, repeat: false, modifiers: m });
                    self.synthetic.push(egui::Event::Key { key: chord.key, physical_key: None, pressed: false, repeat: false, modifiers: m });
                }
            }
            ctx.request_repaint();
            return;
        }
        if let Err(e) = self.run(id, params.clone()) {
            self.status(e);
        }
        ctx.request_repaint();
    }
}

/// Commands a dialog's text fields answer to, which stay on while a dialog is open.
fn edit_in_text_field(id: &str) -> bool {
    matches!(id, "edit.undo" | "edit.redo" | "edit.cut" | "edit.copy" | "edit.paste" | "select.all")
}

#[cfg(test)]
mod tests {
    use super::*;
    use wordcraft_engine::Session;

    fn app() -> WordApp {
        WordApp::new(Session::new(wordcraft_engine::sample::sample_document()), crate::Services::default())
    }

    fn actions(items: &[Item], out: &mut Vec<Action>) {
        for i in items {
            match i {
                Item::Action(a) => out.push(a.clone()),
                Item::Submenu { items, .. } => actions(items, out),
                _ => {}
            }
        }
    }

    fn all_actions(bar: &MenuBar) -> Vec<Action> {
        let mut out = Vec::new();
        for m in &bar.menus {
            actions(&m.items, &mut out);
        }
        out
    }

    #[test]
    fn mirrors_the_ribbon_tabs_between_the_mac_menus() {
        let bar = app().menu_bar();
        let titles: Vec<&str> = bar.menus.iter().map(|m| m.title.as_str()).collect();
        assert_eq!(titles.first(), Some(&"WordCraft"));
        assert_eq!(titles.get(1..3), Some(&["File", "Edit"][..]));
        for tab in ["Home", "Insert", "Layout", "References", "Review", "View"] {
            assert!(titles.contains(&tab), "{tab} in {titles:?}");
        }
        assert_eq!(titles.last(), Some(&"Help"));
        assert_eq!(bar.menus.last().map(|m| m.role), Some(Role::Help));
        assert!(bar.menus.iter().any(|m| m.role == Role::Window));
    }

    #[test]
    fn keys_are_unique_and_chords_are_bound_once() {
        let bar = app().menu_bar();
        let acts = all_actions(&bar);
        let mut keys: Vec<&str> = acts.iter().map(|a| a.key.as_str()).collect();
        let n = keys.len();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), n, "duplicate item keys");
        let mut chords: Vec<Chord> = acts.iter().filter_map(|a| a.chord).collect();
        let n = chords.len();
        chords.dedup_by(|a, b| a == b);
        let mut seen = Vec::new();
        for c in &chords {
            assert!(!seen.contains(c), "{c:?} bound twice");
            seen.push(*c);
        }
        assert!(n > 20, "the common shortcuts are shown");
    }

    #[test]
    fn home_runs_what_its_ribbon_buttons_run_with_their_shortcuts() {
        let bar = app().menu_bar();
        let home = bar.menus.iter().find(|m| m.title == "Home").expect("Home menu");
        let mut acts = Vec::new();
        actions(&home.items, &mut acts);
        let bold = acts.iter().find(|a| a.does == Does::Run { id: "format.bold".into(), params: json!({}) }).expect("Bold");
        assert_eq!(bold.label, "Bold");
        assert_eq!(bold.chord, Chord::parse("Mod+B"));
        // Replace opens a dialog, and says so.
        assert!(acts.iter().any(|a| a.label == "Replace…"));
        // Groups are separated.
        assert!(home.items.iter().filter(|i| **i == Item::Separator).count() >= 3);
    }

    #[test]
    fn system_keys_stay_with_the_system_items() {
        let bar = app().menu_bar();
        for a in all_actions(&bar) {
            if let Some(c) = a.chord
                && system_chords().contains(&c)
            {
                assert!(matches!(a.does, Does::Quit) || a.key == "ui.backstage {\"page\":\"options\",\"value\":true}", "{} took {c:?}", a.key);
            }
        }
    }

    #[test]
    fn chords_leave_typing_and_editing_keys_alone() {
        assert!(Chord::parse("Mod+B").is_some());
        assert!(Chord::parse("Mod+Shift+S").is_some_and(|c| c.shift && c.key == Key::S));
        assert!(Chord::parse("F7").is_some());
        assert!(Chord::parse("Ctrl+Mod+F").is_some_and(|c| c.ctrl && c.command));
        assert_eq!(Chord::parse("Alt+Q"), None, "Option+letter types a character");
        assert_eq!(Chord::parse("Mod+Left"), None);
        assert_eq!(Chord::parse("Mod+Backspace"), None);
        assert_eq!(Chord::parse("Shift+Enter"), None);
        assert_eq!(Chord::parse(""), None);
    }

    #[test]
    fn open_recent_lists_the_recent_documents() {
        let mut a = app();
        a.ui.recent = vec!["/a/Notes.docx".into(), "/b/Letter.docx".into(), "/c/Notes.docx".into()];
        let bar = a.menu_bar();
        let file = bar.menus.iter().find(|m| m.title == "File").expect("File");
        let Some(Item::Submenu { items, .. }) = file.items.iter().find(|i| matches!(i, Item::Submenu { title, .. } if title == "Open Recent")) else {
            panic!("Open Recent in {:?}", file.items);
        };
        let mut acts = Vec::new();
        actions(items, &mut acts);
        let labels: Vec<&str> = acts.iter().map(|x| x.label.as_str()).collect();
        assert_eq!(labels, ["Notes.docx — a", "Letter.docx", "Notes.docx — c"]);
        assert_eq!(acts.get(1).map(|x| &x.does), Some(&Does::Run { id: "file.open".into(), params: json!({"path": "/b/Letter.docx"}) }));
    }

    #[test]
    fn hidden_tabs_leave_the_bar() {
        let mut a = app();
        a.session.ribbon.set_hidden("Mailings", true).expect("hide Mailings");
        let titles: Vec<String> = a.menu_bar().menus.into_iter().map(|m| m.title).collect();
        assert!(!titles.iter().any(|t| t == "Mailings"), "{titles:?}");
    }

    #[test]
    fn a_dialog_leaves_only_editing_on() {
        let mut a = app();
        let bar = a.menu_bar();
        let acts = all_actions(&bar);
        let find = |id: &str| acts.iter().find(|x| x.does == Does::Run { id: id.into(), params: json!({}) }).cloned().expect(id);
        assert!(a.menu_enabled(&find("format.bold")));
        a.dialog = crate::dialogs::Dialog::open("zoom", &mut a);
        assert!(a.dialog.is_some());
        assert!(!a.menu_enabled(&find("format.bold")));
        assert!(a.menu_enabled(&find("edit.copy")));
    }
}
