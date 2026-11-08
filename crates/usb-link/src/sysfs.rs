//! What the kernel made of a USB device: its interfaces, the driver bound to
//! each, and the ALSA sound cards it created. Read from sysfs, never written.

use std::fs;
use std::path::Path;

use micdeck_core::UsbInterface;

/// Interfaces (sorted by number) and ALSA card numbers (sorted, unique) of
/// the USB device whose sysfs directory is `device_dir`, e.g.
/// `/sys/bus/usb/devices/1-6`. Unreadable entries are skipped.
pub fn read_device(device_dir: &Path) -> (Vec<UsbInterface>, Vec<u32>) {
    let mut interfaces = Vec::new();
    let mut cards = Vec::new();
    let Some(name) = device_dir.file_name().and_then(|n| n.to_str()) else {
        return (interfaces, cards);
    };
    let prefix = format!("{name}:");
    let Ok(entries) = fs::read_dir(device_dir) else {
        return (interfaces, cards);
    };
    for entry in entries.flatten() {
        let file_name = entry.file_name();
        if !file_name.to_str().is_some_and(|n| n.starts_with(&prefix)) {
            continue;
        }
        let dir = entry.path();
        let number = read_hex_u8(&dir.join("bInterfaceNumber"));
        let class = read_hex_u8(&dir.join("bInterfaceClass"));
        if let (Some(number), Some(class)) = (number, class) {
            let driver = fs::read_link(dir.join("driver"))
                .ok()
                .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()));
            interfaces.push(UsbInterface { number, class, driver });
        }
        if let Ok(sound) = fs::read_dir(dir.join("sound")) {
            cards.extend(sound.flatten().filter_map(|s| {
                s.file_name().to_str()?.strip_prefix("card")?.parse::<u32>().ok()
            }));
        }
    }
    interfaces.sort_by_key(|i| i.number);
    cards.sort_unstable();
    cards.dedup();
    (interfaces, cards)
}

fn read_hex_u8(path: &Path) -> Option<u8> {
    u8::from_str_radix(fs::read_to_string(path).ok()?.trim(), 16).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// A throwaway directory shaped like a USB device's sysfs entry.
    struct FakeDevice(PathBuf);

    impl FakeDevice {
        fn new(test: &str) -> Self {
            let root = std::env::temp_dir().join(format!("micdeck-sysfs-{test}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&root);
            let dev = root.join("1-6");
            fs::create_dir_all(&dev).unwrap();
            Self(dev)
        }

        fn interface(&self, n: u8, class: u8, driver: Option<&str>, card: Option<u32>) {
            let dir = self.0.join(format!("1-6:1.{n}"));
            fs::create_dir_all(&dir).unwrap();
            fs::write(dir.join("bInterfaceNumber"), format!("{n:02x}\n")).unwrap();
            fs::write(dir.join("bInterfaceClass"), format!("{class:02x}\n")).unwrap();
            if let Some(d) = driver {
                let target = self.0.parent().unwrap().join("drivers").join(d);
                fs::create_dir_all(&target).unwrap();
                std::os::unix::fs::symlink(&target, dir.join("driver")).unwrap();
            }
            if let Some(c) = card {
                fs::create_dir_all(dir.join("sound").join(format!("card{c}"))).unwrap();
            }
        }
    }

    impl Drop for FakeDevice {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(self.0.parent().unwrap());
        }
    }

    #[test]
    fn finds_audio_driver_and_card() {
        let dev = FakeDevice::new("audio");
        dev.interface(6, 0xff, None, None);
        dev.interface(0, 0x03, Some("usbhid"), None);
        dev.interface(1, 0x01, Some("snd-usb-audio"), Some(5));
        dev.interface(2, 0x01, Some("snd-usb-audio"), None);
        // Unrelated entries in the same directory are ignored.
        fs::create_dir_all(dev.0.join("power")).unwrap();

        let (interfaces, cards) = read_device(&dev.0);
        assert_eq!(cards, vec![5]);
        assert_eq!(interfaces.iter().map(|i| i.number).collect::<Vec<_>>(), vec![0, 1, 2, 6]);
        assert_eq!(interfaces[1].driver.as_deref(), Some("snd-usb-audio"));
        assert_eq!(interfaces[3].driver, None);
        assert_eq!(interfaces[3].class, 0xff);
    }

    #[test]
    fn device_without_audio_driver_has_no_cards() {
        let dev = FakeDevice::new("noaudio");
        dev.interface(1, 0x01, None, None);
        let (interfaces, cards) = read_device(&dev.0);
        assert!(cards.is_empty());
        assert_eq!(interfaces.len(), 1);
        assert_eq!(interfaces[0].driver, None);
    }

    #[test]
    fn missing_directory_yields_nothing() {
        let (interfaces, cards) = read_device(Path::new("/nonexistent/micdeck/1-9"));
        assert!(interfaces.is_empty() && cards.is_empty());
    }
}
