#[cfg(target_os = "macos")]
use crate::identity::APP_ID;
#[cfg(target_os = "linux")]
use crate::identity::APP_NAME;

pub fn set_launch_at_login(enabled: bool) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use winreg::enums::*;
        use winreg::RegKey;
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let (key, _) = hkcu.create_subkey(r"Software\Microsoft\Windows\CurrentVersion\Run").map_err(|e| e.to_string())?;
        if enabled {
            let exe = std::env::current_exe().map_err(|e| e.to_string())?;
            key.set_value("BusinessFlow", &exe.to_string_lossy().to_string()).map_err(|e| e.to_string())?;
        } else {
            let _ = key.delete_value("BusinessFlow");
        }
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME").map_err(|e| e.to_string())?;
        let dir = format!("{home}/Library/LaunchAgents");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let path = format!("{dir}/{APP_ID}.plist");
        if !enabled {
            let _ = std::fs::remove_file(&path);
            return Ok(());
        }
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>Label</key><string>{APP_ID}</string>
<key>ProgramArguments</key><array><string>{}</string></array>
<key>RunAtLoad</key><true/>
</dict></plist>
"#,
            exe.display()
        );
        return std::fs::write(path, xml).map_err(|e| e.to_string());
    }
    #[cfg(target_os = "linux")]
    {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        let dir = format!("{home}/.config/autostart");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let path = format!("{dir}/business-flow.desktop");
        if !enabled {
            let _ = std::fs::remove_file(&path);
            return Ok(());
        }
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let desktop = format!("[Desktop Entry]\nType=Application\nName={APP_NAME}\nExec={}\nX-GNOME-Autostart-enabled=true\n", exe.display());
        return std::fs::write(path, desktop).map_err(|e| e.to_string());
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        let _ = enabled;
        Err("Ezen a rendszeren a bejelentkezéskor indulás nem elérhető.".into())
    }
}
