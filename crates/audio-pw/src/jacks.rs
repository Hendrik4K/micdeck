//! Jack detection of analog inputs: is a plug in the socket? The sound card
//! reports it as read-only ALSA controls such as `Mic - Input Jack`; PipeWire
//! does not pass it on for every input (not at all for inputs set up by
//! hand), so MicDeck reads the controls itself with `amixer`.
//!
//! Used for the jack mode: a DJI receiver connected by its 3.5 mm output
//! instead of USB-C delivers sound, but no data.

use std::collections::HashMap;
use std::process::Command;

/// Plugged state of every jack control of one card: control name → plugged.
pub type CardJacks = HashMap<String, bool>;

/// Reads the jack controls of `card` (number or ALSA id like `Audio`).
pub fn read_card(card: &str) -> Option<CardJacks> {
    let out = Command::new("amixer").args(["-c", card, "contents"]).output().ok()?;
    if !out.status.success() {
        return None;
    }
    Some(parse_contents(&String::from_utf8_lossy(&out.stdout)))
}

/// Parses `amixer contents` output into jack name → plugged.
pub fn parse_contents(text: &str) -> CardJacks {
    let mut jacks = CardJacks::new();
    let mut current: Option<String> = None;
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("numid=") {
            current = rest
                .split("name='")
                .nth(1)
                .and_then(|n| n.split('\'').next())
                .filter(|n| n.ends_with(" Jack"))
                .map(str::to_string);
        } else if let (Some(name), Some(value)) = (&current, line.strip_prefix(": values=")) {
            jacks.insert(name.clone(), value.trim() == "on");
            current = None;
        }
    }
    jacks
}

/// The card an input belongs to, as `amixer -c` takes it.
pub fn card_of(alsa_card: Option<u32>, alsa_path: Option<&str>) -> Option<String> {
    if let Some(card) = alsa_card {
        return Some(card.to_string());
    }
    // "hw:Audio,0" → "Audio"
    let rest = alsa_path?.strip_prefix("hw:")?;
    Some(rest.split(',').next()?.to_string()).filter(|c| !c.is_empty())
}

/// The jack control that belongs to the input `node_name`, if it is an
/// analog socket MicDeck knows how to read.
pub fn control_for(node_name: &str) -> Option<&'static str> {
    let name = node_name.to_ascii_lowercase();
    if !name.starts_with("alsa_input.") {
        return None;
    }
    // „Analog In - Input Jack“ is left out on purpose: on an MSI board (0db0:422d)
    // it reports "on" with nothing plugged in (checked on real hardware).
    if name.contains("__line__") || name.ends_with("line") {
        Some("Line - Input Jack")
    } else if name.contains("__mic__") || name.ends_with("mic") {
        Some("Mic - Input Jack")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "numid=8,iface=CARD,name='Analog In - Input Jack'
  ; type=BOOLEAN,access=r-------,values=1
  : values=on
numid=13,iface=MIXER,name='Line Capture Switch'
  ; type=BOOLEAN,access=rw------,values=1
  : values=off
numid=12,iface=CARD,name='Line - Input Jack'
  ; type=BOOLEAN,access=r-------,values=1
  : values=off
";

    #[test]
    fn reads_jack_controls_only() {
        let jacks = parse_contents(SAMPLE);
        assert_eq!(jacks.get("Analog In - Input Jack"), Some(&true));
        assert_eq!(jacks.get("Line - Input Jack"), Some(&false));
        assert!(!jacks.contains_key("Line Capture Switch"));
    }

    #[test]
    fn finds_card_and_control() {
        assert_eq!(card_of(Some(4), Some("hw:Audio,1")).as_deref(), Some("4"));
        assert_eq!(card_of(None, Some("hw:Audio,0")).as_deref(), Some("Audio"));
        assert_eq!(control_for("alsa_input.usb-Generic_USB_Audio-00.HiFi__Mic__source"), Some("Mic - Input Jack"));
        assert_eq!(control_for("alsa_input.usb-Generic_USB_Audio-00.HiFi__Line__source"), Some("Line - Input Jack"));
        assert_eq!(control_for("alsa_input.usb-DJI_Wireless_Mic_Rx"), None);
    }
}
