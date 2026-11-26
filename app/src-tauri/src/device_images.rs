//! Device pictures the user puts into `~/.config/micdeck/devices/`
//! (`receiver` and `transmitter`, as .png, .webp or .jpg; best with a
//! transparent background). They replace the pictures MicDeck ships with
//! and never leave this computer. An optional `leds.json` there says where
//! the status lights sit on them (see `LedLayout` in the UI).

use std::fs;
use std::path::{Path, PathBuf};

use base64::Engine;
use serde::Serialize;

/// Larger files are skipped; a picture for the window needs far less.
const MAX_BYTES: u64 = 10 * 1024 * 1024;
const FORMATS: [(&str, &str); 4] =
    [("png", "image/png"), ("webp", "image/webp"), ("jpg", "image/jpeg"), ("jpeg", "image/jpeg")];

/// The user's pictures as data URLs; `None` keeps the built-in one.
#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceImages {
    pub receiver: Option<String>,
    pub transmitter: Option<String>,
    pub leds: Option<serde_json::Value>,
}

fn load(dir: &Path, name: &str) -> Option<String> {
    FORMATS.iter().find_map(|(ext, mime)| {
        let path = dir.join(format!("{name}.{ext}"));
        let meta = fs::metadata(&path).ok().filter(|m| m.is_file())?;
        if meta.len() > MAX_BYTES {
            log::warn!("{} is larger than 10 MB, ignored", path.display());
            return None;
        }
        match fs::read(&path) {
            Ok(bytes) => Some(format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes))),
            Err(e) => {
                log::warn!("cannot read {}: {e}", path.display());
                None
            }
        }
    })
}

fn devices_dir() -> Option<PathBuf> {
    crate::settings::config_home().map(|c| c.join("micdeck").join("devices"))
}

pub fn load_all() -> DeviceImages {
    let Some(dir) = devices_dir() else {
        return DeviceImages::default();
    };
    DeviceImages { receiver: load(&dir, "receiver"), transmitter: load(&dir, "transmitter"), leds: load_leds(&dir) }
}

fn load_leds(dir: &Path) -> Option<serde_json::Value> {
    let path = dir.join("leds.json");
    let text = fs::read_to_string(&path).ok()?;
    match serde_json::from_str::<serde_json::Value>(&text) {
        Ok(v) if v.is_object() => Some(v),
        Ok(_) => None,
        Err(e) => {
            log::warn!("{} is not valid JSON: {e}", path.display());
            None
        }
    }
}

/// The file type from the first bytes; the file name may lie.
fn sniff(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("png")
    } else if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("webp")
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("jpg")
    } else {
        None
    }
}

fn check_device(device: &str) -> Result<(), String> {
    match device {
        "receiver" | "transmitter" => Ok(()),
        _ => Err(format!("unknown device {device:?}")),
    }
}

/// Moves the current picture of `device` aside as `<device>-vorher.<ext>`
/// (the previous one there is replaced), so nothing the user had is lost.
fn put_aside(dir: &Path, device: &str) -> Result<(), String> {
    for (ext, _) in FORMATS {
        let _ = fs::remove_file(dir.join(format!("{device}-vorher.{ext}")));
    }
    for (ext, _) in FORMATS {
        let from = dir.join(format!("{device}.{ext}"));
        if from.is_file() {
            fs::rename(&from, dir.join(format!("{device}-vorher.{ext}")))
                .map_err(|e| format!("cannot move {}: {e}", from.display()))?;
        }
    }
    Ok(())
}

/// Light positions belong to one picture; a new picture starts without.
fn forget_leds(dir: &Path, device: &str) {
    let path = dir.join("leds.json");
    let Ok(text) = fs::read_to_string(&path) else { return };
    let Ok(mut value) = serde_json::from_str::<serde_json::Value>(&text) else { return };
    if value.as_object_mut().and_then(|o| o.remove(device)).is_some()
        && let Ok(text) = serde_json::to_string_pretty(&value)
    {
        let _ = fs::write(&path, text + "\n");
    }
}

/// Copies a picture the user picked into MicDeck's own folder, so it stays
/// even if the original in Downloads goes away.
pub fn save(device: &str, bytes: &[u8]) -> Result<DeviceImages, String> {
    check_device(device)?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err("Das Bild ist größer als 10 MB.".into());
    }
    let ext = sniff(bytes).ok_or("Bitte ein PNG-, WebP- oder JPEG-Bild wählen.")?;
    let dir = devices_dir().ok_or("Kein Konfigurationsordner gefunden.")?;
    save_in(&dir, device, ext, bytes)?;
    Ok(load_all())
}

fn save_in(dir: &Path, device: &str, ext: &str, bytes: &[u8]) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    put_aside(dir, device)?;
    let path = dir.join(format!("{device}.{ext}"));
    fs::write(&path, bytes).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    forget_leds(dir, device);
    log::info!("own picture for {device} saved as {}", path.display());
    Ok(())
}

/// Back to the built-in picture; the user's one is kept as `-vorher`.
pub fn reset(device: &str) -> Result<DeviceImages, String> {
    check_device(device)?;
    if let Some(dir) = devices_dir() {
        put_aside(&dir, device)?;
        forget_leds(&dir, device);
    }
    Ok(load_all())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_any_supported_format_and_skips_missing_ones() {
        let dir = std::env::temp_dir().join(format!("micdeck-images-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("receiver.webp"), b"RIFF").unwrap();
        assert_eq!(load(&dir, "receiver").as_deref(), Some("data:image/webp;base64,UklGRg=="));
        assert_eq!(load(&dir, "transmitter"), None);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn saving_keeps_the_old_picture_and_drops_its_lights() {
        let dir = std::env::temp_dir().join(format!("micdeck-save-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("receiver.webp"), b"old").unwrap();
        fs::write(dir.join("leds.json"), r#"{"receiver":{"power":[1,2]},"transmitter":{"status":[3,4]}}"#).unwrap();
        let png = b"\x89PNG\r\n\x1a\nrest";
        assert_eq!(sniff(png), Some("png"));
        save_in(&dir, "receiver", "png", png).unwrap();
        assert_eq!(fs::read(dir.join("receiver.png")).unwrap(), png);
        assert!(!dir.join("receiver.webp").exists());
        assert_eq!(fs::read(dir.join("receiver-vorher.webp")).unwrap(), b"old");
        let leds: serde_json::Value = serde_json::from_str(&fs::read_to_string(dir.join("leds.json")).unwrap()).unwrap();
        assert!(leds.get("receiver").is_none() && leds.get("transmitter").is_some());
        assert_eq!(sniff(b"GIF89a"), None);
        assert!(check_device("../x").is_err());
        fs::remove_dir_all(&dir).unwrap();
    }
}
