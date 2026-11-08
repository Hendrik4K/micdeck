//! Starts and stops dictation in the Claude desktop app by pressing its
//! dictation shortcut (Ctrl+D by default, a start/stop toggle) through XTest.
//!
//! Keys only ever go to the Claude window: MicDeck brings it to the front
//! first and presses nothing if that doesn't work, because Ctrl+D means
//! something else elsewhere (in a terminal it closes the shell).

use std::thread::sleep;
use std::time::{Duration, Instant};

use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    AtomEnum, ClientMessageEvent, ConnectionExt as _, EventMask, Window, KEY_PRESS_EVENT, KEY_RELEASE_EVENT,
};
use x11rb::protocol::xtest::ConnectionExt as _;
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;
use x11rb::CURRENT_TIME;

const KEYSYM_CONTROL_L: u32 = 0xffe3;
const KEYSYM_D: u32 = 0x0064;
const KEYSYM_RETURN: u32 = 0xff0d;

/// After stopping, Claude still turns the last words into text; Enter must
/// wait for that. Not measured precisely yet: chosen generously.
const FINAL_TRANSCRIPT_WAIT: Duration = Duration::from_millis(2_000);
/// Words spoken while putting the transmitter back still count: wait this
/// long before stopping. (A first test lost the end of a sentence.)
const STOP_GRACE: Duration = Duration::from_millis(1_000);
const ACTIVATE_TIMEOUT: Duration = Duration::from_millis(1_500);

#[derive(Debug, Clone, Copy)]
pub enum Action {
    Start,
    StopAndSend,
}

pub fn run(action: Action) -> Result<(), String> {
    let (conn, screen) = RustConnection::connect(None).map_err(|e| format!("keine Verbindung zum Desktop ({e})"))?;
    let root = conn.setup().roots[screen].root;
    let active_atom = atom(&conn, b"_NET_ACTIVE_WINDOW")?;
    let claude = find_claude(&conn, root)?;

    if let Action::StopAndSend = action {
        sleep(STOP_GRACE);
    }
    activate(&conn, root, active_atom, claude)?;
    press(&conn, root, &[KEYSYM_CONTROL_L, KEYSYM_D])?;
    if let Action::StopAndSend = action {
        sleep(FINAL_TRANSCRIPT_WAIT);
        // The user may have switched windows meanwhile: never send Enter elsewhere.
        if active_window(&conn, root, active_atom)? != Some(claude) {
            return Err("Diktat gestoppt, aber nicht abgeschickt: Claude war nicht mehr im Vordergrund".into());
        }
        press(&conn, root, &[KEYSYM_RETURN])?;
    }
    Ok(())
}

fn x_err(e: impl std::fmt::Display) -> String {
    format!("X11-Fehler ({e})")
}

fn atom(conn: &RustConnection, name: &[u8]) -> Result<u32, String> {
    Ok(conn.intern_atom(false, name).map_err(x_err)?.reply().map_err(x_err)?.atom)
}

/// The Claude desktop window, by its WM_CLASS (`com.anthropic.Claude`).
fn find_claude(conn: &RustConnection, root: Window) -> Result<Window, String> {
    let list_atom = atom(conn, b"_NET_CLIENT_LIST")?;
    let list = conn
        .get_property(false, root, list_atom, AtomEnum::WINDOW, 0, 4096)
        .map_err(x_err)?
        .reply()
        .map_err(x_err)?;
    for win in list.value32().into_iter().flatten() {
        let Ok(reply) = conn.get_property(false, win, AtomEnum::WM_CLASS, AtomEnum::STRING, 0, 256) else { continue };
        let Ok(class) = reply.reply() else { continue };
        let class = String::from_utf8_lossy(&class.value).to_lowercase();
        if class.split('\0').any(|part| part == "com.anthropic.claude") {
            return Ok(win);
        }
    }
    Err("Claude-Fenster nicht gefunden – läuft die Claude-App?".into())
}

fn active_window(conn: &RustConnection, root: Window, active_atom: u32) -> Result<Option<Window>, String> {
    let reply = conn
        .get_property(false, root, active_atom, AtomEnum::WINDOW, 0, 1)
        .map_err(x_err)?
        .reply()
        .map_err(x_err)?;
    Ok(reply.value32().and_then(|mut v| v.next()).filter(|&w| w != 0))
}

fn activate(conn: &RustConnection, root: Window, active_atom: u32, win: Window) -> Result<(), String> {
    if active_window(conn, root, active_atom)? != Some(win) {
        // Source 2 = "pager": a request on the user's behalf, which window
        // managers honour even with focus-stealing prevention.
        let msg = ClientMessageEvent::new(32, win, active_atom, [2, CURRENT_TIME, 0, 0, 0]);
        conn.send_event(false, root, EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY, msg)
            .map_err(x_err)?;
        conn.flush().map_err(x_err)?;
        let start = Instant::now();
        while active_window(conn, root, active_atom)? != Some(win) {
            if start.elapsed() > ACTIVATE_TIMEOUT {
                return Err("Claude-Fenster ließ sich nicht in den Vordergrund holen".into());
            }
            sleep(Duration::from_millis(50));
        }
    }
    // Give the app a moment to put the keyboard focus back into its window.
    sleep(Duration::from_millis(200));
    Ok(())
}

/// Press the keys in order, then release them in reverse (a chord).
fn press(conn: &RustConnection, root: Window, keysyms: &[u32]) -> Result<(), String> {
    let setup = conn.setup();
    let (min, max) = (setup.min_keycode, setup.max_keycode);
    let map = conn.get_keyboard_mapping(min, max - min + 1).map_err(x_err)?.reply().map_err(x_err)?;
    let per = usize::from(map.keysyms_per_keycode).max(1);
    let keycode = |sym: u32| {
        map.keysyms.chunks(per).position(|syms| syms.first() == Some(&sym)).map(|i| min + i as u8)
    };
    let codes = keysyms
        .iter()
        .map(|&s| keycode(s).ok_or_else(|| format!("Taste {s:#x} nicht auf der Tastatur")))
        .collect::<Result<Vec<_>, _>>()?;
    for &code in &codes {
        conn.xtest_fake_input(KEY_PRESS_EVENT, code, CURRENT_TIME, root, 0, 0, 0).map_err(x_err)?;
    }
    conn.flush().map_err(x_err)?;
    sleep(Duration::from_millis(30));
    for &code in codes.iter().rev() {
        conn.xtest_fake_input(KEY_RELEASE_EVENT, code, CURRENT_TIME, root, 0, 0, 0).map_err(x_err)?;
    }
    conn.flush().map_err(x_err)?;
    conn.sync().map_err(x_err)?;
    Ok(())
}
