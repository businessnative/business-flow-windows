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
