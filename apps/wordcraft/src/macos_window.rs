//! Tells macOS about the document in the window: the dot in the close button while there are
//! unsaved changes, the file the window shows (Mission Control, the Window menu, the title's
//! proxy icon), and each document opened or saved, for the Dock menu's recent documents.
//!
//! Synced when the document's state changes, through calls that are safe in the `objc2-app-kit`
//! bindings (our workspace forbids unsafe).

use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSDocumentController};
use objc2_foundation::{NSString, NSURL};
use wordcraft_ui_egui::WordApp;

/// What the window last told macOS.
#[derive(Default)]
pub struct WindowDocument {
    synced: Option<(bool, Option<std::path::PathBuf>)>,
}

impl WindowDocument {
    pub fn sync(&mut self, app: &WordApp) {
        let now = (app.session.dirty, app.session.path.clone());
        if self.synced.as_ref() == Some(&now) {
            return;
        }
        let Some(mtm) = MainThreadMarker::new() else { return };
        let Some(window) = NSApplication::sharedApplication(mtm).windows().firstObject() else { return };
        let (dirty, path) = &now;
        window.setDocumentEdited(*dirty);
        let shown = path.as_ref().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
        window.setRepresentedFilename(&NSString::from_str(&shown));
        // A new file in the window (opened, or saved under a new name) joins the recent documents.
        let new_file = path.is_some() && self.synced.as_ref().is_none_or(|(_, before)| before != path);
        if new_file && !shown.is_empty() {
            let url = NSURL::fileURLWithPath(&NSString::from_str(&shown));
            NSDocumentController::sharedDocumentController(mtm).noteNewRecentDocumentURL(&url);
        }
        self.synced = Some(now);
    }
}
