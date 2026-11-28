//! The accent colour of the system under macOS: the frame of the active tile takes it, as the frame of today's app
//! takes `NSColor.controlAccentColor`. Elsewhere the frame takes the accent token of the colour set.
//!
//! The window reads the choice from the global defaults (`AppleAccentColor`), without a binding to `AppKit`: no new
//! crate, no unsafe code. The values are the accent colours of macOS in sRGB, converted into Display P3 like the
//! measured role colours of [`crate::gestalt`] (the reference pictures carry a Display profile). For blue the conversion
//! gives exactly the measured frame of today's app, #3478f6 from #007aff.

use egui::Color32;

/// Blue, the default of macOS and the measured frame of today's app (`haupt-worker-dunkel.png`, x=1200, y=104..107).
pub const BLAU: Color32 = Color32::from_rgb(0x34, 0x78, 0xf6);

/// The accent colour for a value of `AppleAccentColor`; `None` (the key is missing) is the multicolour default, blue.
/// An unknown value gives `None`, and the frame keeps the accent token.
#[must_use]
pub fn aus_wert(wert: Option<i64>) -> Option<Color32> {
    let c = |r, g, b| Some(Color32::from_rgb(r, g, b));
    match wert {
        None | Some(4) => Some(BLAU),
        // graphite #989898, red #e0383e, orange #f7821b, yellow #ffc600, green #62ba46, purple #953d96, pink #f74f9e
        Some(-1) => c(0x98, 0x98, 0x98),
        Some(0) => c(0xce, 0x47, 0x45),
        Some(1) => c(0xe8, 0x88, 0x3a),
        Some(2) => c(0xf6, 0xc8, 0x44),
        Some(3) => c(0x78, 0xb8, 0x56),
        Some(5) => c(0x8a, 0x42, 0x92),
        Some(6) => c(0xe4, 0x5c, 0x9c),
        Some(_) => None,
    }
}

/// The accent colour of the system: `defaults read -g AppleAccentColor`. A missing key (exit code 1, nothing on
/// stdout) is blue; if `defaults` cannot run, `None`.
#[cfg(target_os = "macos")]
#[must_use]
pub fn system() -> Option<Color32> {
    let aus = std::process::Command::new("/usr/bin/defaults")
        .args(["read", "-g", "AppleAccentColor"])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    if !aus.status.success() {
        return aus_wert(None);
    }
    let text = String::from_utf8_lossy(&aus.stdout);
    aus_wert(Some(text.trim().parse().ok()?))
}

/// Outside macOS the frame keeps the accent token of the colour set.
#[cfg(not(target_os = "macos"))]
#[must_use]
pub fn system() -> Option<Color32> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blue_is_the_default_and_the_measured_frame() {
        assert_eq!(aus_wert(None), Some(BLAU));
        assert_eq!(aus_wert(Some(4)), Some(BLAU));
        assert_eq!(BLAU, Color32::from_rgb(0x34, 0x78, 0xf6));
        assert_eq!(aus_wert(Some(7)), None);
        for w in -1..=6 {
            assert!(aus_wert(Some(w)).is_some(), "{w}");
        }
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn outside_macos_the_token_stays() {
        assert_eq!(system(), None);
    }
}
