//! The macOS accent colour (System Settings › Appearance › Accent color), which the interface uses
//! in place of WordCraft's own blue (`theme::set_accent`).
//!
//! Read once at start-up through `NSColor::controlAccentColor`, converted to sRGB. The call is safe
//! in the `objc2-app-kit` bindings the app already uses for Reduce motion (our workspace forbids
//! unsafe). A change made while WordCraft is running shows after a restart.

use egui::Color32;
use objc2_app_kit::{NSColor, NSColorSpace};

/// The system accent colour, or `None` when AppKit can't give it in sRGB.
pub fn system() -> Option<Color32> {
    let accent = NSColor::controlAccentColor().colorUsingColorSpace(&NSColorSpace::sRGBColorSpace())?;
    Some(rgb(accent.redComponent(), accent.greenComponent(), accent.blueComponent()))
}

/// An sRGB colour from AppKit's components (0 to 1), clamped and rounded to bytes.
fn rgb(red: f64, green: f64, blue: f64) -> Color32 {
    let byte = |c: f64| (c.clamp(0.0, 1.0) * 255.0).round() as u8;
    Color32::from_rgb(byte(red), byte(green), byte(blue))
}

#[cfg(test)]
mod tests {
    use super::{rgb, system};
    use egui::Color32;

    /// AppKit's 0 to 1 components become bytes; out-of-range values are clamped, not wrapped.
    #[test]
    fn components_become_bytes() {
        assert_eq!(rgb(0.0, 0.5, 1.0), Color32::from_rgb(0, 128, 255));
        assert_eq!(rgb(-0.2, 1.4, f64::NAN), Color32::from_rgb(0, 255, 0));
    }

    /// Whatever the system reports, it is an opaque colour (or nothing, if AppKit can't convert it).
    #[test]
    fn system_accent_is_opaque_or_absent() {
        assert!(system().is_none_or(|c| c.a() == 255));
    }
}
