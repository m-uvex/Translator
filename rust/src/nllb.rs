//! Startet den NLLB-200-Server der Einrichtung.
//!
//! Die Electron-Fassung liest `nllbAutostart` aus `config.json` und startet
//! `nllb-venv/Scripts/pythonw.exe nllb_server.py` ohne Fenster (`main.js`).
//! Hier entscheidet das Feld `Settings::nllb_autostart`; gesucht wird neben der
//! laufenden `uebersetzer.exe`, weil Installieren.bat beide Dateien in denselben
//! Ordner legt.

use std::path::{Path, PathBuf};
use std::process::{Child, Command};

/// Pfad der Python-Umgebung und des Servers neben der Exe.
fn paths() -> Option<(PathBuf, PathBuf)> {
    let dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    let py = dir.join("nllb-venv").join("Scripts").join("pythonw.exe");
    let script = dir.join("nllb_server.py");
    if py.exists() && script.exists() {
        Some((py, script))
    } else {
        None
    }
}

/// Wahr, wenn neben der Exe eine eingerichtete NLLB-Umgebung liegt.
pub fn installed() -> bool {
    paths().is_some()
}

/// Startet den Server, sofern er eingerichtet ist. Der Aufrufer behaelt das
/// Kind und beendet es beim Schliessen der App.
pub fn start() -> Option<Child> {
    let (py, script) = paths()?;
    let dir: &Path = py.parent()?.parent()?.parent()?;
    if reachable() {
        return None;
    }
    let mut cmd = Command::new(&py);
    cmd.arg(&script)
        .current_dir(dir)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: kein Konsolenfenster wie bei `windowsHide` in Electron.
        cmd.creation_flags(0x0800_0000);
    }
    cmd.spawn().ok()
}

/// Antwortet der Server schon auf seinem Port? Der Aufruf blockiert kurz.
pub fn reachable() -> bool {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::time::Duration;
    let Ok(mut s) = TcpStream::connect_timeout(
        &"127.0.0.1:8765".parse().expect("Adresse"),
        Duration::from_millis(400),
    ) else {
        return false;
    };
    let _ = s.set_read_timeout(Some(Duration::from_millis(600)));
    if s.write_all(b"GET /languages HTTP/1.0\r\nHost: 127.0.0.1\r\n\r\n")
        .is_err()
    {
        return false;
    }
    let mut buf = [0u8; 16];
    matches!(s.read(&mut buf), Ok(n) if n > 0)
}

/// Beendet den gestarteten Server, falls er noch laeuft.
pub fn stop(child: &mut Option<Child>) {
    if let Some(mut c) = child.take() {
        let _ = c.kill();
        let _ = c.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Im Entwicklungsbau liegt keine `nllb-venv` neben der Test-Exe; dann darf
    /// nichts gestartet werden.
    #[test]
    fn ohne_umgebung_startet_nichts() {
        if installed() {
            return;
        }
        assert!(start().is_none());
    }

    /// `reachable` fragt nur kurz an und blockiert nicht, wenn niemand lauscht.
    #[test]
    fn erreichbarkeit_blockiert_nicht() {
        let t0 = std::time::Instant::now();
        let _ = reachable();
        assert!(t0.elapsed() < std::time::Duration::from_secs(3));
    }
}
