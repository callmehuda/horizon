//! The JS glue (js/glue.js) bound for wasm: its module is inlined into the build's output by
//! wasm-bindgen, so the site stays the two files this crate's index.html loads. Compiled for
//! the browser only; the native build runs the same UI against platform.rs's stubs.

#![cfg(target_arch = "wasm32")]

use wasm_bindgen::prelude::*;

#[wasm_bindgen(module = "/js/glue.js")]
extern "C" {
    pub fn canPickFolder() -> bool;
    pub fn pickFolder() -> js_sys::Promise;
    pub fn hasFolder() -> bool;
    pub fn folderName() -> String;
    pub fn folderSha1(path: &str) -> js_sys::Promise;
    pub fn readFile(path: &str) -> js_sys::Promise;
    pub fn writeFile(path: &str, bytes: &[u8]) -> js_sys::Promise;
    pub fn fetchBytes(url: &str, onProgress: &js_sys::Function) -> js_sys::Promise;
    pub fn fetchText(url: &str) -> js_sys::Promise;
    pub fn openUrl(url: &str);
    pub fn userAgent() -> String;
    pub fn languages() -> String;
}
