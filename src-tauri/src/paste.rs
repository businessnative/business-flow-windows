use std::thread;
use std::time::Duration;

pub fn paste_text(text: &str) -> Result<(), String> {
    if text.is_empty() {
        return Err("Üres a szöveg.".into());
    }
    let mut clipboard = arboard::Clipboard::new().map_err(|_| "A vágólap nem elérhető.".to_string())?;
    let previous = clipboard.get_text().ok();
    clipboard.set_text(text.to_string()).map_err(|_| "A vágólapra másolás nem sikerült.".to_string())?;
    thread::sleep(Duration::from_millis(40));
    if let Err(err) = send_paste() {
        return Err(format!("{err} A szöveg a vágólapon maradt: illeszd be kézzel."));
    }
    thread::sleep(Duration::from_millis(160));
    if let Some(prev) = previous {
        let _ = clipboard.set_text(prev);
    }
    Ok(())
}

/// Copies the current selection without leaving it on the clipboard.
/// Returns None when nothing new was copied (no selection, or the shortcut was ignored).
pub fn capture_selection() -> Option<String> {
    let mut clipboard = arboard::Clipboard::new().ok()?;
    let previous = clipboard.get_text().ok();
    send_copy().ok()?;
    thread::sleep(Duration::from_millis(80));
    let now = clipboard.get_text().ok();
    if let Some(prev) = previous.clone() {
        let _ = clipboard.set_text(prev);
    }
    let now = now?;
    if previous.as_ref() == Some(&now) || now.trim().is_empty() {
        None
    } else {
        Some(now)
    }
}

fn send_copy() -> Result<(), String> {
    use enigo::{Direction, Enigo, Key, Keyboard, Settings};
    let mut enigo = Enigo::new(&Settings::default()).map_err(|_| permission_message())?;
    let modifier = if cfg!(target_os = "macos") { Key::Meta } else { Key::Control };
    enigo.key(modifier, Direction::Press).map_err(|_| permission_message())?;
    enigo.key(Key::Unicode('c'), Direction::Click).map_err(|_| permission_message())?;
    enigo.key(modifier, Direction::Release).map_err(|_| permission_message())?;
    Ok(())
}

fn send_paste() -> Result<(), String> {
    use enigo::{Direction, Enigo, Key, Keyboard, Settings};
    let mut enigo = Enigo::new(&Settings::default()).map_err(|_| permission_message())?;
    let modifier = if cfg!(target_os = "macos") { Key::Meta } else { Key::Control };
    enigo.key(modifier, Direction::Press).map_err(|_| permission_message())?;
    enigo.key(Key::Unicode('v'), Direction::Click).map_err(|_| permission_message())?;
    enigo.key(modifier, Direction::Release).map_err(|_| permission_message())?;
    Ok(())
}

fn permission_message() -> String {
    if cfg!(target_os = "macos") {
        "A beillesztéshez Akadálymentesítés engedély kell (Rendszerbeállítások).".into()
    } else {
        "A beillesztés nem sikerült. Emelt jogosultságú ablakba a rendszer nem enged gépelni.".into()
    }
}
