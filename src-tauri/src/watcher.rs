//! Background thread that polls the system clipboard and saves new text
//! and images to the store.

use std::io::Cursor;
use std::thread;
use std::time::Duration;

use arboard::Clipboard;
use tauri::{AppHandle, Emitter, Manager};

use crate::db::{hash_bytes, NewClip, Store};
use crate::platform;

const POLL_INTERVAL: Duration = Duration::from_millis(400);
/// Text longer than this (in bytes) is not recorded.
const MAX_TEXT_BYTES: usize = 1_000_000;
/// Images larger than this (in pixels) are not recorded.
const MAX_IMAGE_PIXELS: usize = 40_000_000;

/// Event sent to the UI whenever the history changes.
pub const CLIPS_CHANGED: &str = "clips-changed";

enum Captured {
    Text(String),
    Image {
        rgba: Vec<u8>,
        width: u32,
        height: u32,
    },
}

impl Captured {
    fn key(&self) -> String {
        match self {
            Captured::Text(t) => hash_bytes(t.as_bytes()),
            Captured::Image { rgba, width, height } => image_key(rgba, *width, *height),
        }
    }
}

pub fn image_key(rgba: &[u8], width: u32, height: u32) -> String {
    let mut buf = Vec::with_capacity(rgba.len() + 8);
    buf.extend_from_slice(&width.to_le_bytes());
    buf.extend_from_slice(&height.to_le_bytes());
    buf.extend_from_slice(rgba);
    hash_bytes(&buf)
}

pub fn spawn(app: AppHandle) {
    thread::spawn(move || {
        let mut clipboard = loop {
            match Clipboard::new() {
                Ok(c) => break c,
                Err(e) => {
                    eprintln!("clipboard unavailable, retrying: {e}");
                    thread::sleep(Duration::from_secs(2));
                }
            }
        };

        let mut last_count = platform::change_count();
        // Seed with whatever is on the clipboard at launch so it isn't
        // re-saved (and bumped to the top) every time the app starts.
        let mut last_key = capture(&mut clipboard).map(|c| c.key());

        loop {
            thread::sleep(POLL_INTERVAL);

            // Where the OS gives us a change counter, skip reading the
            // clipboard entirely until it moves.
            let count = platform::change_count();
            if count.is_some() && count == last_count {
                continue;
            }
            last_count = count;

            if platform::should_skip() {
                continue;
            }
            let Some(captured) = capture(&mut clipboard) else {
                continue;
            };
            let key = captured.key();
            if last_key.as_deref() == Some(key.as_str()) {
                continue;
            }
            last_key = Some(key.clone());

            let Some(clip) = to_new_clip(captured, key) else {
                continue;
            };
            match app.state::<Store>().insert(clip) {
                Ok(id) => {
                    let _ = app.emit(CLIPS_CHANGED, id);
                }
                Err(e) => eprintln!("failed to save clip: {e}"),
            }
        }
    });
}

/// Reads the clipboard, preferring text when both text and an image are
/// present (e.g. cells copied from a spreadsheet).
fn capture(clipboard: &mut Clipboard) -> Option<Captured> {
    if let Ok(text) = clipboard.get_text() {
        if !text.trim().is_empty() && text.len() <= MAX_TEXT_BYTES {
            return Some(Captured::Text(text));
        }
    }
    let img = clipboard.get_image().ok()?;
    if img.width == 0 || img.height == 0 || img.width * img.height > MAX_IMAGE_PIXELS {
        return None;
    }
    Some(Captured::Image {
        rgba: img.bytes.into_owned(),
        width: img.width as u32,
        height: img.height as u32,
    })
}

fn to_new_clip(captured: Captured, key: String) -> Option<NewClip> {
    match captured {
        Captured::Text(t) => Some(NewClip::Text(t)),
        Captured::Image { rgba, width, height } => {
            let png = encode_png(rgba, width, height)?;
            Some(NewClip::Image { png, width, height, key })
        }
    }
}

fn encode_png(rgba: Vec<u8>, width: u32, height: u32) -> Option<Vec<u8>> {
    let img = image::RgbaImage::from_raw(width, height, rgba)?;
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).ok()?;
    Some(out.into_inner())
}

/// Puts a saved clip back on the system clipboard.
pub fn write_back(clip: &crate::db::Clip) -> Result<(), String> {
    let mut clipboard = Clipboard::new().map_err(|e| e.to_string())?;
    match &clip.image_path {
        Some(path) => {
            let img = image::open(path).map_err(|e| e.to_string())?.into_rgba8();
            let (width, height) = img.dimensions();
            clipboard
                .set_image(arboard::ImageData {
                    width: width as usize,
                    height: height as usize,
                    bytes: img.into_raw().into(),
                })
                .map_err(|e| e.to_string())
        }
        None => clipboard.set_text(clip.text.clone()).map_err(|e| e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_round_trip_keeps_key() {
        let rgba: Vec<u8> = (0..4 * 3 * 2).map(|i| i as u8).collect();
        let key = image_key(&rgba, 3, 2);
        let png = encode_png(rgba.clone(), 3, 2).unwrap();
        let decoded = image::load_from_memory(&png).unwrap().into_rgba8();
        assert_eq!(image_key(decoded.as_raw(), 3, 2), key);
    }

    #[test]
    fn key_depends_on_dimensions() {
        let rgba = vec![0u8; 4 * 6];
        assert_ne!(image_key(&rgba, 3, 2), image_key(&rgba, 2, 3));
    }
}
