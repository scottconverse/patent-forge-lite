//! Brand assets for the native desktop.
//!
//! The PatentForge identity guide supplies the symbol, the application icon and
//! the Barlow typefaces. Everything is embedded at compile time, so the app has
//! no runtime asset directory to lose.
use gpui::{AssetSource, Result, SharedString};
use std::borrow::Cow;

pub const SYMBOL_REVERSE: &str = "brand/symbol-reverse.svg";
pub const SYMBOL_PRIMARY: &str = "brand/symbol-primary.svg";
pub const APP_ICON: &str = "brand/app-256.png";
pub const SYMBOL_BODY: &str = "brand/symbol-body.svg";
pub const SYMBOL_FOLD: &str = "brand/symbol-fold.svg";

/// Assets embedded at build time. The SVG masters are the guide's own files.
pub struct BrandAssets;

impl AssetSource for BrandAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        let bytes: &'static [u8] = match path {
            SYMBOL_REVERSE => include_bytes!("../../../../assets/brand/symbol-reverse.svg"),
            SYMBOL_PRIMARY => include_bytes!("../../../../assets/brand/symbol-primary.svg"),
            APP_ICON => include_bytes!("../../../../assets/brand/app-256.png"),
            SYMBOL_BODY => include_bytes!("../../../../assets/brand/symbol-body.svg"),
            SYMBOL_FOLD => include_bytes!("../../../../assets/brand/symbol-fold.svg"),
            _ => return Ok(None),
        };
        Ok(Some(Cow::Borrowed(bytes)))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        if path.is_empty() || path == "brand" {
            return Ok(vec![
                SharedString::from(SYMBOL_REVERSE),
                SharedString::from(SYMBOL_PRIMARY),
                SharedString::from(APP_ICON),
                SharedString::from(SYMBOL_BODY),
                SharedString::from(SYMBOL_FOLD),
            ]);
        }
        Ok(Vec::new())
    }
}

/// Barlow, as specified by the guide: Bold for headings, Regular for the rest.
/// Both are SIL OFL licensed; `assets/fonts/OFL.txt` travels with them.
pub fn font_blobs() -> Vec<Cow<'static, [u8]>> {
    vec![
        Cow::Borrowed(include_bytes!(
            "../../../../assets/fonts/Barlow-Regular.ttf"
        )),
        Cow::Borrowed(include_bytes!("../../../../assets/fonts/Barlow-Bold.ttf")),
    ]
}

pub const FONT_FAMILY: &str = "Barlow";
