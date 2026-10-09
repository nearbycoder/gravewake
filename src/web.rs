//! The browser build's ties to its page (`web/index.html`): saves and
//! settings in `localStorage`, the canvas the game draws on, the loading
//! screen, and a panic shown on the page instead of only in the console.
use wasm_bindgen::{JsCast, JsValue};

const PREFIX: &str = "gravewake/";

fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

/// A save file's contents, stored under `gravewake/<name>`.
pub fn load(name: &str) -> Option<Vec<u8>> {
    storage()?
        .get_item(&format!("{PREFIX}{name}"))
        .ok()
        .flatten()
        .map(String::into_bytes)
}

/// Store a save file. Every save is JSON, so it is kept as text.
pub fn store(name: &str, data: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    let text = std::str::from_utf8(data)?;
    storage()
        .ok_or("browser storage is unavailable")?
        .set_item(&format!("{PREFIX}{name}"), text)
        .map_err(|e| format!("browser storage refused the save: {e:?}"))?;
    Ok(())
}

/// The page's canvas, `<canvas id="gravewake">`.
pub fn canvas() -> Option<web_sys::HtmlCanvasElement> {
    web_sys::window()?
        .document()?
        .get_element_by_id("gravewake")?
        .dyn_into()
        .ok()
}

/// Whether the pointer is locked to the page, for mouse look.
pub fn pointer_locked() -> bool {
    web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.pointer_lock_element())
        .is_some()
}

/// Call `window.<name>(argument)` if the page defines it.
fn call_page(name: &str, argument: &str) {
    let Some(window) = web_sys::window() else {
        return;
    };
    if let Ok(f) = js_sys::Reflect::get(&window, &JsValue::from_str(name)) {
        if let Some(f) = f.dyn_ref::<js_sys::Function>() {
            let _ = f.call1(&JsValue::NULL, &JsValue::from_str(argument));
        }
    }
}

/// Tell the page the game is drawing, so it hides the loading screen.
pub fn ready() {
    call_page("gravewakeReady", "");
}

/// Tell the page which graphics backend the game got.
pub fn graphics(name: &str) {
    call_page("gravewakeGraphics", name);
}

/// Report loading steps to the page's progress line.
pub fn status(text: &str) {
    call_page("gravewakeStatus", text);
}

/// Show panics on the page, which has no terminal to print them to.
pub fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let message = info.to_string();
        web_sys::console::error_1(&JsValue::from_str(&message));
        call_page("gravewakeFail", &message);
    }));
}
