//! One interface for the browser's powers (glue.rs, wasm) and the native development run's
//! stubs: the native build compiles and runs the same UI with everything the browser-only
//! parts would do turned into notes, so `cargo run` previews the pages.

use std::future::Future;

/// What picking a folder came to (the wasm half constructs it; the native stub
/// answers anything else).
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub enum Picked {
    /// A folder is chosen; its name is remembered.
    Chosen,
    /// The player closed the picker.
    Cancelled,
    /// This browser has no File System Access API (Firefox, Safari): downloads and releases
    /// still work, writing a folder does not.
    Unsupported,
}

/// Runs `task` in the background: the browser's microtasks on wasm, nowhere (the native
/// development run has nothing to run: every stub below answers at once).
pub fn spawn<T>(task: T)
where
    T: Future<Output = ()> + 'static,
{
    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_futures::spawn_local(task);
    #[cfg(not(target_arch = "wasm32"))]
    drop(task); // Nothing to run it on, and nothing below would answer differently.
}

pub fn can_pick_folder() -> bool {
    #[cfg(target_arch = "wasm32")]
    return crate::glue::canPickFolder();
    #[cfg(not(target_arch = "wasm32"))]
    false
}

pub fn folder_name() -> String {
    #[cfg(target_arch = "wasm32")]
    return crate::glue::folderName();
    #[cfg(not(target_arch = "wasm32"))]
    String::new()
}

pub fn user_agent() -> String {
    #[cfg(target_arch = "wasm32")]
    return crate::glue::userAgent();
    #[cfg(not(target_arch = "wasm32"))]
    "native".into()
}

pub fn open_url(url: &str) {
    #[cfg(target_arch = "wasm32")]
    crate::glue::openUrl(url);
    #[cfg(not(target_arch = "wasm32"))]
    println!("open {url}"); // The development run only shows the UI.
}

pub async fn pick_folder() -> Result<Picked, String> {
    #[cfg(target_arch = "wasm32")]
    {
        let answer = await_js(crate::glue::pickFolder(), "choosing a folder").await?;
        match answer.as_string().as_deref() {
            Some("ok") => Ok(Picked::Chosen),
            Some("cancelled") => Ok(Picked::Cancelled),
            Some("unsupported") => Ok(Picked::Unsupported),
            _ => Err("choosing a folder: the browser's answer was not one".into()),
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Err("The native development run has no folder picker; the browser build does".into())
    }
}

/// The file's sha1, or `None` when there is no folder or no such file.
pub async fn folder_sha1(path: &str) -> Option<String> {
    #[cfg(target_arch = "wasm32")]
    {
        let promise = crate::glue::folderSha1(path);
        let value = await_js(promise, &format!("reading {path}")).await.ok()?;
        if value.is_null() || value.is_undefined() {
            None
        } else {
            value.as_string()
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = path;
        None
    }
}

pub async fn read_file(path: &str) -> Result<Vec<u8>, String> {
    #[cfg(target_arch = "wasm32")]
    {
        let promise = crate::glue::readFile(path);
        let value = await_js(promise, &format!("reading {path}")).await?;
        let bytes = js_sys::Uint8Array::new(&value).to_vec();
        Ok(bytes)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = path;
        Err("The native development run reads no files; the browser build does".into())
    }
}

/// Writes the file whole (a partial one never takes its place, as the desktop launcher's
/// `write_file` keeps it).
pub async fn write_file(path: &str, bytes: &[u8]) -> Result<(), String> {
    #[cfg(target_arch = "wasm32")]
    {
        let promise = crate::glue::writeFile(path, bytes);
        await_js(promise, &format!("writing {path}")).await?;
        Ok(())
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (path, bytes);
        Err("The native development run writes no files; the browser build does".into())
    }
}

/// The url's bytes, each arrival told to `progress` (bytes so far, of the size the server
/// lists).
pub async fn fetch_bytes<F>(url: &str, progress: F) -> Result<Vec<u8>, String>
where
    F: Fn(u64, u64) + 'static,
{
    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::closure::Closure;
        let told =
            move |loaded: f64, total: f64| progress(loaded.max(0.) as u64, total.max(0.) as u64);
        let closure: Closure<dyn FnMut(f64, f64)> = Closure::wrap(Box::new(told));
        let function = js_sys::Function::from(closure.as_ref().clone());
        let promise = crate::glue::fetchBytes(url, &function);
        let value = await_js(promise, &format!("downloading {url}")).await?;
        let bytes = js_sys::Uint8Array::new(&value).to_vec();
        drop(closure);
        Ok(bytes)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (url, &progress);
        Err("The native development run downloads nothing; the browser build does".into())
    }
}

pub async fn fetch_text(url: &str) -> Result<String, String> {
    #[cfg(target_arch = "wasm32")]
    {
        let promise = crate::glue::fetchText(url);
        let value = await_js(promise, &format!("loading {url}")).await?;
        value
            .as_string()
            .ok_or_else(|| format!("loading {url}: not text"))
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = url;
        Err("The releases load in the browser build".into())
    }
}

/// Awaits a JS promise, turning a rejection into a message (an Error's message, or a thrown
/// string) with `what` in front.
#[cfg(target_arch = "wasm32")]
pub(crate) async fn await_js(
    promise: js_sys::Promise,
    what: &str,
) -> Result<wasm_bindgen::JsValue, String> {
    match wasm_bindgen_futures::JsFuture::from(promise).await {
        Ok(value) => Ok(value),
        Err(error) => Err(format!("{what}: {}", js_message(&error))),
    }
}

#[cfg(target_arch = "wasm32")]
fn js_message(value: &wasm_bindgen::JsValue) -> String {
    if let Some(text) = value.as_string() {
        return text;
    }
    if let Ok(message) = js_sys::Reflect::get(value, &wasm_bindgen::JsValue::from_str("message")) {
        if let Some(text) = message.as_string() {
            return text;
        }
    }
    format!("{value:?}")
}
