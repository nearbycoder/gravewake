//! The keyboard layout's own characters for the keys the game can bind, read
//! once at startup so key names are right before any key is pressed: an
//! AZERTY keyboard shows Z Q S D on the first banner.
//!
//! Linux under Wayland only. The compositor sends its keymap to every client
//! that asks for a keyboard; a second Wayland connection asks for it on a
//! thread of its own, and libxkbcommon (which winit already loads) reads the
//! character each key types on the first layout, unshifted. Elsewhere, and if
//! anything fails, a key shows its own character once it's been pressed, as
//! before.

use winit::keyboard::KeyCode;

/// The character each bindable key types, for keys that type one.
pub type Glyphs = Vec<(KeyCode, char)>;

/// Read the layout on a thread. The receiver gets one message if that works.
pub fn start() -> std::sync::mpsc::Receiver<Glyphs> {
    let (sender, receiver) = std::sync::mpsc::channel();
    #[cfg(target_os = "linux")]
    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        let _ = std::thread::Builder::new()
            .name("keymap".into())
            .spawn(move || match wayland::keymap().and_then(|map| glyphs(&map)) {
                Ok(glyphs) => {
                    let _ = sender.send(glyphs);
                }
                Err(error) => eprintln!(
                    "Keyboard layout not read ({error}); keys show their own character once pressed"
                ),
            });
    }
    #[cfg(not(target_os = "linux"))]
    drop(sender);
    receiver
}

/// The characters a keymap in XKB's text format gives the bindable keys.
#[cfg(target_os = "linux")]
pub fn glyphs(keymap: &str) -> Result<Glyphs, String> {
    use winit::{keyboard::PhysicalKey, platform::scancode::PhysicalKeyExtScancode};
    use xkbcommon_dl::{
        xkb_context_flags, xkb_keymap_compile_flags, xkb_keymap_format, xkb_keysym_t,
        xkbcommon_option,
    };
    let xkb = xkbcommon_option().ok_or("libxkbcommon isn't available")?;
    let text = std::ffi::CString::new(keymap.trim_end_matches('\0'))
        .map_err(|_| "the keymap has a stray NUL")?;
    let mut glyphs = Glyphs::new();
    // SAFETY: the context and keymap are checked for null, used only here,
    // and released before returning; `syms` points into the keymap.
    unsafe {
        let context = (xkb.xkb_context_new)(xkb_context_flags::XKB_CONTEXT_NO_FLAGS);
        if context.is_null() {
            return Err("libxkbcommon couldn't start".into());
        }
        let map = (xkb.xkb_keymap_new_from_string)(
            context,
            text.as_ptr(),
            xkb_keymap_format::XKB_KEYMAP_FORMAT_TEXT_V1,
            xkb_keymap_compile_flags::XKB_KEYMAP_COMPILE_NO_FLAGS,
        );
        if map.is_null() {
            (xkb.xkb_context_unref)(context);
            return Err("the compositor's keymap didn't compile".into());
        }
        for code in crate::controls::glyph_keys() {
            // Evdev codes, which XKB numbers from 8.
            let Some(scancode) = PhysicalKey::Code(code).to_scancode() else {
                continue;
            };
            let mut syms: *const xkb_keysym_t = std::ptr::null();
            let count = (xkb.xkb_keymap_key_get_syms_by_level)(map, scancode + 8, 0, 0, &mut syms);
            if count < 1 || syms.is_null() {
                continue;
            }
            // Dead keys and non-characters convert to 0.
            let glyph = char::from_u32((xkb.xkb_keysym_to_utf32)(*syms))
                .filter(|c| *c != '\0' && !c.is_control() && !c.is_whitespace());
            if let Some(glyph) = glyph {
                glyphs.push((code, glyph));
            }
        }
        (xkb.xkb_keymap_unref)(map);
        (xkb.xkb_context_unref)(context);
    }
    Ok(glyphs)
}

#[cfg(target_os = "linux")]
mod wayland {
    use std::os::{fd::OwnedFd, unix::fs::FileExt};
    use wayland_client::{
        Connection, Dispatch, Proxy, QueueHandle, WEnum,
        protocol::{wl_keyboard, wl_registry, wl_seat},
    };

    #[derive(Default)]
    struct State {
        seat: Option<wl_seat::WlSeat>,
        keyboard: bool,
        keymap: Option<(OwnedFd, u32)>,
    }
    impl Dispatch<wl_registry::WlRegistry, ()> for State {
        fn event(
            state: &mut Self,
            registry: &wl_registry::WlRegistry,
            event: wl_registry::Event,
            _: &(),
            _: &Connection,
            queue: &QueueHandle<Self>,
        ) {
            if let wl_registry::Event::Global {
                name,
                interface,
                version,
            } = event
            {
                if interface == "wl_seat" && state.seat.is_none() {
                    state.seat = Some(registry.bind(name, version.min(5), queue, ()));
                }
            }
        }
    }
    impl Dispatch<wl_seat::WlSeat, ()> for State {
        fn event(
            state: &mut Self,
            _: &wl_seat::WlSeat,
            event: wl_seat::Event,
            _: &(),
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
            if let wl_seat::Event::Capabilities {
                capabilities: WEnum::Value(capabilities),
            } = event
            {
                state.keyboard = capabilities.contains(wl_seat::Capability::Keyboard);
            }
        }
    }
    impl Dispatch<wl_keyboard::WlKeyboard, ()> for State {
        fn event(
            state: &mut Self,
            _: &wl_keyboard::WlKeyboard,
            event: wl_keyboard::Event,
            _: &(),
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
            if let wl_keyboard::Event::Keymap {
                format: WEnum::Value(wl_keyboard::KeymapFormat::XkbV1),
                fd,
                size,
            } = event
            {
                state.keymap = Some((fd, size));
            }
        }
    }

    /// The compositor's keymap, in XKB's text format.
    pub fn keymap() -> Result<String, String> {
        let connection = Connection::connect_to_env().map_err(|e| e.to_string())?;
        let mut queue = connection.new_event_queue();
        let handle = queue.handle();
        connection.display().get_registry(&handle, ());
        let mut state = State::default();
        // The globals, then the seat's capabilities.
        for _ in 0..2 {
            queue.roundtrip(&mut state).map_err(|e| e.to_string())?;
        }
        let seat = state.seat.as_ref().ok_or("no seat")?;
        if !state.keyboard {
            return Err("the seat has no keyboard".into());
        }
        let keyboard = seat.get_keyboard(&handle, ());
        queue.roundtrip(&mut state).map_err(|e| e.to_string())?;
        let (fd, size) = state.keymap.take().ok_or("no keymap was sent")?;
        // Read at an offset: the descriptor may be shared with other clients.
        let mut text = vec![0; size as usize];
        std::fs::File::from(fd)
            .read_exact_at(&mut text, 0)
            .map_err(|e| e.to_string())?;
        if keyboard.version() >= 3 {
            keyboard.release();
        }
        Ok(String::from_utf8_lossy(&text).into_owned())
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use xkbcommon_dl::{
        xkb_context_flags, xkb_keymap_compile_flags, xkb_keymap_format, xkb_rule_names,
        xkbcommon_option,
    };

    /// A layout's keymap as a compositor would send it, or `None` without
    /// libxkbcommon or its layout data.
    fn keymap(layout: &str) -> Option<String> {
        let xkb = xkbcommon_option()?;
        let layout = std::ffi::CString::new(layout).unwrap();
        let empty = c"";
        let names = xkb_rule_names {
            rules: empty.as_ptr(),
            model: empty.as_ptr(),
            layout: layout.as_ptr(),
            variant: empty.as_ptr(),
            options: empty.as_ptr(),
        };
        // SAFETY: as in `glyphs`; the string is copied before it's freed.
        unsafe {
            let context =
                (xkb.xkb_context_new)(xkb_context_flags::XKB_CONTEXT_NO_ENVIRONMENT_NAMES);
            if context.is_null() {
                return None;
            }
            let map = (xkb.xkb_keymap_new_from_names)(
                context,
                &names,
                xkb_keymap_compile_flags::XKB_KEYMAP_COMPILE_NO_FLAGS,
            );
            let text = (!map.is_null()).then(|| {
                let raw = (xkb.xkb_keymap_get_as_string)(
                    map,
                    xkb_keymap_format::XKB_KEYMAP_FORMAT_TEXT_V1,
                );
                let text = std::ffi::CStr::from_ptr(raw).to_string_lossy().into_owned();
                libc_free(raw as *mut _);
                (xkb.xkb_keymap_unref)(map);
                text
            });
            (xkb.xkb_context_unref)(context);
            text
        }
    }
    unsafe extern "C" {
        #[link_name = "free"]
        fn libc_free(pointer: *mut std::ffi::c_void);
    }

    fn movement(glyphs: &Glyphs) -> String {
        let mut bindings = crate::controls::Bindings::default();
        bindings.learn_layout(glyphs);
        bindings.movement_label()
    }

    #[test]
    fn key_names_follow_the_compositors_layout() {
        let (Some(us), Some(fr)) = (keymap("us"), keymap("fr")) else {
            eprintln!("skipped: libxkbcommon or its layouts aren't installed");
            return;
        };
        let us = glyphs(&us).unwrap();
        let fr = glyphs(&fr).unwrap();
        assert_eq!(movement(&us), "W A S D");
        assert_eq!(movement(&fr), "Z Q S D");
        let typed = |glyphs: &Glyphs, code| glyphs.iter().find(|g| g.0 == code).map(|g| g.1);
        // AZERTY: Q types a, M's key types a comma, the digit row types
        // symbols unshifted, and the dead circumflex has no character.
        assert_eq!(typed(&fr, KeyCode::KeyQ), Some('a'));
        assert_eq!(typed(&fr, KeyCode::KeyM), Some(','));
        assert_eq!(typed(&fr, KeyCode::Digit1), Some('&'));
        assert_eq!(typed(&fr, KeyCode::BracketLeft), None);
        assert_eq!(typed(&us, KeyCode::Semicolon), Some(';'));
        assert!(glyphs("not a keymap").is_err());
    }
}
