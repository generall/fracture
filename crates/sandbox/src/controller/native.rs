//! SDL owns only controller input; Macroquad continues to own window/audio.
//! Loaded at runtime so keyboard play still works when SDL2 is unavailable.
use super::*;
use libloading::Library;
use std::ffi::{c_char, c_int, c_void};
const CONTROLLER_SUBSYSTEM: u32 = 0x2000;
type Pad = *mut c_void;
struct Sdl {
    _library: Library,
    count: unsafe extern "C" fn() -> c_int,
    is_controller: unsafe extern "C" fn(c_int) -> c_int,
    open: unsafe extern "C" fn(c_int) -> Pad,
    close: unsafe extern "C" fn(Pad),
    attached: unsafe extern "C" fn(Pad) -> c_int,
    update: unsafe extern "C" fn(),
    pump: unsafe extern "C" fn(),
    axis: unsafe extern "C" fn(Pad, c_int) -> i16,
    button: unsafe extern "C" fn(Pad, c_int) -> u8,
    quit: unsafe extern "C" fn(u32),
    pad: Pad,
}
impl Sdl {
    unsafe fn load(name: &str) -> Result<Self, Box<dyn std::error::Error>> {
        // All symbols have the SDL2 C ABI. The library outlives their pointers.
        unsafe {
            let library = Library::new(name)?;
            let init: unsafe extern "C" fn(u32) -> c_int = *library.get(b"SDL_InitSubSystem\0")?;
            let hint: unsafe extern "C" fn(*const c_char, *const c_char) -> c_int =
                *library.get(b"SDL_SetHint\0")?;
            let events: unsafe extern "C" fn(c_int) -> c_int =
                *library.get(b"SDL_GameControllerEventState\0")?;
            let sdl = Self {
                count: *library.get(b"SDL_NumJoysticks\0")?,
                is_controller: *library.get(b"SDL_IsGameController\0")?,
                open: *library.get(b"SDL_GameControllerOpen\0")?,
                close: *library.get(b"SDL_GameControllerClose\0")?,
                attached: *library.get(b"SDL_GameControllerGetAttached\0")?,
                update: *library.get(b"SDL_GameControllerUpdate\0")?,
                pump: *library.get(b"SDL_PumpEvents\0")?,
                axis: *library.get(b"SDL_GameControllerGetAxis\0")?,
                button: *library.get(b"SDL_GameControllerGetButton\0")?,
                quit: *library.get(b"SDL_QuitSubSystem\0")?,
                pad: std::ptr::null_mut(),
                _library: library,
            };
            // SDL has no window: input belongs to Macroquad's window instead.
            hint(
                c"SDL_JOYSTICK_ALLOW_BACKGROUND_EVENTS".as_ptr(),
                c"1".as_ptr(),
            );
            // SDL_QUIT is not our application event loop. Keep normal Unix
            // termination instead of converting signals into unread events.
            hint(c"SDL_NO_SIGNAL_HANDLERS".as_ptr(), c"1".as_ptr());
            if init(CONTROLLER_SUBSYSTEM) != 0 {
                return Err(std::io::Error::other("SDL controller initialization failed").into());
            }
            events(0); // Poll state without accumulating unused controller events.
            Ok(sdl)
        }
    }
    fn poll(&mut self) -> Snapshot {
        unsafe {
            (self.pump)();
            (self.update)();
            if !self.pad.is_null() && (self.attached)(self.pad) == 0 {
                (self.close)(self.pad);
                self.pad = std::ptr::null_mut();
                return Snapshot::default();
            }
            if self.pad.is_null() {
                for index in 0..(self.count)() {
                    if (self.is_controller)(index) != 0 {
                        self.pad = (self.open)(index);
                        if !self.pad.is_null() {
                            break;
                        }
                    }
                }
            }
            if self.pad.is_null() {
                return Snapshot::default();
            }
            let axis = |i| ((self.axis)(self.pad, i) as f32 / 32767.0).clamp(-1.0, 1.0);
            let mut buttons = 0;
            for (index, action) in [
                (0, CONFIRM),
                (1, BACK),
                (2, HELP),
                (3, SETTINGS),
                (6, MENU),
                (10, AMMO),
                (11, UP),
                (12, DOWN),
                (13, LEFT),
                (14, RIGHT),
            ] {
                if (self.button)(self.pad, index) != 0 {
                    buttons |= action;
                }
            }
            if axis(5) > 0.25 {
                buttons |= FIRE;
            }
            if axis(4) > 0.25 {
                buttons |= SPECIAL;
            }
            Snapshot {
                connected: true,
                left: vec2(axis(0), axis(1)),
                right: vec2(axis(2), axis(3)),
                buttons,
            }
        }
    }
}
impl Drop for Sdl {
    fn drop(&mut self) {
        unsafe {
            if !self.pad.is_null() {
                (self.close)(self.pad);
            }
            (self.quit)(CONTROLLER_SUBSYSTEM);
        }
    }
}
pub struct Backend(Option<Sdl>);
impl Backend {
    pub fn new() -> Self {
        let names = if cfg!(target_os = "windows") {
            vec!["SDL2.dll"]
        } else if cfg!(target_os = "macos") {
            vec!["libSDL2.dylib", "/opt/homebrew/lib/libSDL2.dylib"]
        } else {
            vec!["libSDL2-2.0.so.0", "libSDL2.so"]
        };
        Self(
            names
                .into_iter()
                .find_map(|name| unsafe { Sdl::load(name).ok() }),
        )
    }
    pub fn poll(&mut self) -> Snapshot {
        self.0.as_mut().map_or_else(Snapshot::default, Sdl::poll)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires the SDL2 runtime; uses an SDL virtual controller, no hardware"]
    fn virtual_controller_axes_buttons_and_disconnect() {
        let mut sdl = unsafe { Sdl::load("libSDL2-2.0.so.0") }.expect("SDL2 runtime");
        unsafe {
            let attach: unsafe extern "C" fn(c_int, c_int, c_int, c_int) -> c_int =
                *sdl._library.get(b"SDL_JoystickAttachVirtual\0").unwrap();
            let detach: unsafe extern "C" fn(c_int) -> c_int =
                *sdl._library.get(b"SDL_JoystickDetachVirtual\0").unwrap();
            let open: unsafe extern "C" fn(c_int) -> Pad =
                *sdl._library.get(b"SDL_JoystickOpen\0").unwrap();
            let close: unsafe extern "C" fn(Pad) =
                *sdl._library.get(b"SDL_JoystickClose\0").unwrap();
            let axis: unsafe extern "C" fn(Pad, c_int, i16) -> c_int =
                *sdl._library.get(b"SDL_JoystickSetVirtualAxis\0").unwrap();
            let button: unsafe extern "C" fn(Pad, c_int, u8) -> c_int =
                *sdl._library.get(b"SDL_JoystickSetVirtualButton\0").unwrap();
            let index = attach(1, 6, 15, 0);
            assert!(index >= 0);
            let joystick = open(index);
            assert!(!joystick.is_null());
            axis(joystick, 4, -32768);
            axis(joystick, 5, -32768);
            assert!(sdl.poll().connected);
            axis(joystick, 0, 16384);
            axis(joystick, 1, -32768);
            axis(joystick, 2, -32768);
            axis(joystick, 3, 16384);
            axis(joystick, 5, 32767);
            button(joystick, 0, 1);
            button(joystick, 10, 1);
            let state = sdl.poll();
            assert!((state.left.x - 0.5).abs() < 0.01);
            assert_eq!(state.left.y, -1.0);
            assert_eq!(state.right.x, -1.0);
            assert!(state.buttons & FIRE != 0);
            assert!(state.buttons & CONFIRM != 0);
            assert!(state.buttons & AMMO != 0);
            detach(index);
            close(joystick);
            assert!(!sdl.poll().connected);
        }
    }
}
