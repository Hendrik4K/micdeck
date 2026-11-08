//! Device pictures the user puts into `~/.config/micdeck/devices/`
//! (`receiver` and `transmitter`, as .png, .webp or .jpg; best with a
//! transparent background). They replace the silhouettes MicDeck ships with
//! and never leave this computer.

use std::fs;
use std::path::Path;

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

pub fn load_all() -> DeviceImages {
    let Some(dir) = crate::settings::config_home().map(|c| c.join("micdeck").join("devices")) else {
        return DeviceImages::default();
    };
    DeviceImages { receiver: load(&dir, "receiver"), transmitter: load(&dir, "transmitter") }
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
}
