//! The native development run; the browser build starts in `web_start` (src/lib.rs) instead.

#![cfg(not(target_arch = "wasm32"))]

fn main() -> Result<(), eframe::Error> {
    horizon_web::run()
}
