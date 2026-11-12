//! Fensterecken unter Windows.
//!
//! Die Electron-Fassung laeuft in einem Fenster, das Windows wie jedes andere
//! Programm mit abgerundeten Ecken zeichnet. Die Rust-Fassung bringt ihre
//! eigene Fensterleiste mit (`with_decorations(false)`) und bekommt diese
//! Rundung deshalb nicht von selbst — das Fenster bleibt eckig. Hier wird sie
//! ueber den Fenstermanager (DWM) angefordert. Unter Windows 10 gibt es die
//! Einstellung nicht; der Aufruf bleibt dann wirkungslos.

#[cfg(target_os = "windows")]
mod win {
    use std::sync::atomic::{AtomicIsize, Ordering};

    type Hwnd = isize;

    #[link(name = "user32")]
    extern "system" {
        fn GetCurrentProcessId() -> u32;
        fn EnumWindows(cb: extern "system" fn(Hwnd, isize) -> i32, lparam: isize) -> i32;
        fn GetWindowThreadProcessId(h: Hwnd, pid: *mut u32) -> u32;
    }

    #[link(name = "dwmapi")]
    extern "system" {
        fn DwmSetWindowAttribute(
            hwnd: Hwnd,
            attr: u32,
            wert: *const core::ffi::c_void,
            groesse: u32,
        ) -> i32;
    }

    /// `DWMWA_WINDOW_CORNER_PREFERENCE` und `DWMWCP_ROUND` aus `dwmapi.h`.
    const WINDOW_CORNER_PREFERENCE: u32 = 33;
    const ROUND: i32 = 2;

    /// Merkt sich das erste Fenster des eigenen Prozesses.
    static GEFUNDEN: AtomicIsize = AtomicIsize::new(0);

    extern "system" fn merken(hwnd: Hwnd, _lparam: isize) -> i32 {
        let mut pid: u32 = 0;
        // Safety: gueltiges Fensterhandle, Zeiger auf eine lokale Variable.
        unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
        // Safety: nur ein Abruf der eigenen Prozesskennung.
        if pid == unsafe { GetCurrentProcessId() } {
            GEFUNDEN.store(hwnd, Ordering::SeqCst);
            return 0; // Suche beenden
        }
        1
    }

    pub fn runde_ecken() {
        // Safety: reine Abfrage von Fensterhandles, keine Speicherzugriffe
        // ausserhalb der hier angelegten Werte.
        unsafe {
            EnumWindows(merken, 0);
            let hwnd = GEFUNDEN.load(Ordering::SeqCst);
            if hwnd != 0 {
                let wert = ROUND;
                DwmSetWindowAttribute(
                    hwnd,
                    WINDOW_CORNER_PREFERENCE,
                    &wert as *const i32 as *const core::ffi::c_void,
                    std::mem::size_of::<i32>() as u32,
                );
            }
        }
    }
}

/// Fordert fuer das eigene Fenster abgerundete Ecken an.
pub fn runde_ecken() {
    #[cfg(target_os = "windows")]
    win::runde_ecken();
}
