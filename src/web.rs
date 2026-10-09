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

/// The asset pack (`scripts/build-pages.sh`): "GWPK", a u32 count, then for
/// each file a u16 name length, its name (its path under assets/), and u32
/// offset and length into the data that follows; all little-endian. The page
/// leaves it in `window.gravewakeAssets` before starting the game.
fn pack() -> &'static std::collections::HashMap<&'static str, &'static [u8]> {
    static PACK: std::sync::OnceLock<std::collections::HashMap<&'static str, &'static [u8]>> =
        std::sync::OnceLock::new();
    PACK.get_or_init(|| {
        let window = web_sys::window().expect("a browser window");
        let key = JsValue::from_str("gravewakeAssets");
        let array: js_sys::Uint8Array = js_sys::Reflect::get(&window, &key)
            .ok()
            .and_then(|v| v.dyn_into().ok())
            .expect("the page didn't load the asset pack");
        // Copied once into the game's memory; the page's copy can go.
        let bytes: &'static [u8] = Box::leak(array.to_vec().into_boxed_slice());
        let _ = js_sys::Reflect::delete_property(&window, &key);
        parse_pack(bytes).expect("the asset pack is damaged")
    })
}

fn parse_pack(bytes: &'static [u8]) -> Option<std::collections::HashMap<&'static str, &'static [u8]>> {
    let u16_at = |i: usize| Some(u16::from_le_bytes(bytes.get(i..i + 2)?.try_into().ok()?) as usize);
    let u32_at = |i: usize| Some(u32::from_le_bytes(bytes.get(i..i + 4)?.try_into().ok()?) as usize);
    if bytes.get(..4)? != b"GWPK" {
        return None;
    }
    let count = u32_at(4)?;
    let mut entries = Vec::with_capacity(count);
    let mut at = 8;
    for _ in 0..count {
        let length = u16_at(at)?;
        let name = std::str::from_utf8(bytes.get(at + 2..at + 2 + length)?).ok()?;
        at += 2 + length;
        entries.push((name, u32_at(at)?, u32_at(at + 4)?));
        at += 8;
    }
    let data = bytes.get(at..)?;
    entries
        .into_iter()
        .map(|(name, offset, length)| Some((name, data.get(offset..offset + length)?)))
        .collect()
}

/// An embedded file (`asset_bytes!`), by its path from `src/`.
pub fn asset(path: &str) -> &'static [u8] {
    let name = path.strip_prefix("../assets/").unwrap_or(path);
    pack()
        .get(name)
        .copied()
        .unwrap_or_else(|| panic!("{name} is missing from the asset pack"))
}

/// Let the browser run its other tasks, and come back in the next one.
pub async fn next_task() {
    let promise = js_sys::Promise::new(&mut |resolve, _| {
        if let Some(window) = web_sys::window() {
            let _ = window.set_timeout_with_callback(&resolve);
        }
    });
    let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
}

/// The page's canvas, `<canvas id="gravewake">`.
pub fn canvas() -> Option<web_sys::HtmlCanvasElement> {
    web_sys::window()?
        .document()?
        .get_element_by_id("gravewake")?
        .dyn_into()
        .ok()
}

/// Whether this browser has Pointer Lock (iPhones' browsers don't, and
/// calling it there throws).
pub fn pointer_lock_supported() -> bool {
    web_sys::window()
        .and_then(|w| w.document())
        .is_some_and(|d| js_sys::Reflect::has(&d, &JsValue::from_str("exitPointerLock")).unwrap_or(false))
}

/// Whether the pointer is locked to the page, for mouse look.
pub fn pointer_locked() -> bool {
    web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.pointer_lock_element())
        .is_some()
}

/// Whether the page set `window.gravewake.<name>` to true before loading.
fn page_flag(name: &str) -> bool {
    web_sys::window()
        .and_then(|w| js_sys::Reflect::get(&w, &JsValue::from_str("gravewake")).ok())
        .and_then(|g| js_sys::Reflect::get(&g, &JsValue::from_str(name)).ok())
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

/// A phone or tablet, by the page's judgement: a new player starts at Low
/// fidelity, and the large art is decoded at reduced size, to stay within
/// the memory such a browser gives one tab.
pub fn lite() -> bool {
    page_flag("lite")
}

/// Draw with WebGL2 even where the browser offers WebGPU: the page asks for
/// it on iOS, where WebGL2 is the long-established path.
pub fn webgl_only() -> bool {
    page_flag("webgl")
}

/// A touchscreen with no mouse or trackpad (the page's judgement): the game
/// starts with its on-screen controls.
pub fn touch_first() -> bool {
    page_flag("touch")
}

/// The screen's safe-area insets in CSS pixels (top, right, bottom, left),
/// which the page measures from `env(safe-area-inset-*)`.
pub fn safe_area() -> [f32; 4] {
    let mut insets = [0.; 4];
    let value = web_sys::window()
        .and_then(|w| js_sys::Reflect::get(&w, &JsValue::from_str("gravewake")).ok())
        .and_then(|g| js_sys::Reflect::get(&g, &JsValue::from_str("safe")).ok());
    if let Some(array) = value.and_then(|v| v.dyn_into::<js_sys::Array>().ok()) {
        for (i, inset) in insets.iter_mut().enumerate() {
            *inset = array.get(i as u32).as_f64().unwrap_or(0.).max(0.) as f32;
        }
    }
    insets
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

/// Report the game's state to the page as JSON, for the browser checks.
pub fn report(json: &str) {
    call_page("gravewakeReport", json);
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
