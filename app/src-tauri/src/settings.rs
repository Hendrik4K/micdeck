//! MicDeck's own settings (`~/.config/micdeck/settings.json`) and its
//! autostart entry (`~/.config/autostart/micdeck.desktop`). Both live in the
//! user's config folder; nothing else on the system is touched, and the
//! autostart entry only exists while the user has switched it on.

use std::fs;
use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    /// Desktop notification when the microphone stops being ready.
    pub notifications: bool,
    /// Taking transmitter 1 out of its charging case starts dictation in the
    /// Claude app; putting it back stops it and sends the message. Off unless
    /// the user switches it on.
    pub dock_dictation: bool,
    /// Mute the DJI input while transmitter 1 sits in its charging case,
    /// unmute when it is taken out. Off unless the user switches it on.
    pub dock_mute: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self { notifications: true, dock_dictation: false, dock_mute: false }
    }
}

/// One of MicDeck's own settings, as the window and the panel menu name it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AppSetting {
    Notifications,
    DockDictation,
    DockMute,
    Autostart,
}

/// MicDeck's own settings as the window shows them.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettingsView {
    pub notifications: bool,
    pub dock_dictation: bool,
    pub dock_mute: bool,
    pub autostart: bool,
    pub autostart_available: bool,
}

impl Settings {
    pub fn view(&self) -> AppSettingsView {
        AppSettingsView {
            notifications: self.notifications,
            dock_dictation: self.dock_dictation,
            dock_mute: self.dock_mute,
            autostart: autostart_enabled(),
            autostart_available: AUTOSTART_AVAILABLE,
        }
    }

    /// Sets `setting` (not autostart, which is a file of its own).
    pub fn set(&mut self, setting: AppSetting, on: bool) {
        match setting {
            AppSetting::Notifications => self.notifications = on,
            AppSetting::DockDictation => self.dock_dictation = on,
            AppSetting::DockMute => self.dock_mute = on,
            AppSetting::Autostart => {}
        }
    }
}

pub(crate) fn config_home() -> Option<PathBuf> {
    match std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from) {
        Some(dir) if dir.is_absolute() => Some(dir),
        _ => std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")),
    }
}

fn settings_path() -> Option<PathBuf> {
    config_home().map(|c| c.join("micdeck").join("settings.json"))
}

fn autostart_path() -> Option<PathBuf> {
    config_home().map(|c| c.join("autostart").join("micdeck.desktop"))
}

pub fn load() -> Settings {
    let Some(path) = settings_path() else { return Settings::default() };
    match fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_else(|e| {
            log::warn!("ignoring unreadable {}: {e}", path.display());
            Settings::default()
        }),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Settings::default(),
        Err(e) => {
            log::warn!("cannot read {}: {e}", path.display());
            Settings::default()
        }
    }
}

pub fn save(settings: &Settings) -> io::Result<()> {
    let path = settings_path().ok_or_else(|| io::Error::other("no config folder"))?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_string_pretty(settings).map_err(io::Error::other)?)?;
    fs::rename(tmp, path)
}

/// Autostart only makes sense for an installed build: a development build
/// loads its interface from the dev server, which isn't running at login.
/// (`tauri build` enables `tauri/custom-protocol`, not this crate's feature
/// of that name, so ask Tauri.)
pub const AUTOSTART_AVAILABLE: bool = !tauri::is_dev();

pub fn autostart_enabled() -> bool {
    autostart_path().is_some_and(|p| p.exists())
}

/// Quote a path for the `Exec` key of a desktop entry.
fn desktop_quote(path: &str) -> String {
    let mut out = String::from("\"");
    for c in path.chars() {
        if matches!(c, '"' | '`' | '$' | '\\') {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('"');
    // The whole value is unescaped once more for `\\` when read.
    out.replace('\\', "\\\\")
}

pub fn set_autostart(on: bool) -> io::Result<()> {
    let path = autostart_path().ok_or_else(|| io::Error::other("no config folder"))?;
    if !on {
        return match fs::remove_file(&path) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            other => other,
        };
    }
    let exe = std::env::current_exe()?;
    let entry = format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=MicDeck\n\
         Comment=Zeigt, ob das DJI Mic Mini verbunden ist und Ton liefert\n\
         Exec={} --hidden\n\
         Icon=micdeck\n\
         Terminal=false\n\
         X-GNOME-Autostart-enabled=true\n",
        desktop_quote(&exe.to_string_lossy()),
    );
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(path, entry)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exec_paths_with_spaces_and_specials_are_quoted() {
        assert_eq!(desktop_quote("/usr/bin/micdeck"), "\"/usr/bin/micdeck\"");
        assert_eq!(desktop_quote("/opt/My Apps/micdeck"), "\"/opt/My Apps/micdeck\"");
        // `$` gets a backslash, which itself is doubled for the file format.
        assert_eq!(desktop_quote("/a$b"), "\"/a\\\\$b\"");
    }

    #[test]
    fn unknown_or_missing_fields_fall_back_to_defaults() {
        let s: Settings = serde_json::from_str("{}").unwrap();
        assert!(s.notifications);
        let s: Settings = serde_json::from_str(r#"{"notifications":false,"future":1}"#).unwrap();
        assert!(!s.notifications);
    }
}
