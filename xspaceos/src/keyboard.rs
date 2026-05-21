//! Polled PS/2 keyboard driver for XSpace OS.
//!
//! XSpace OS does not set up an interrupt descriptor table (IDT), so it cannot
//! use the usual interrupt-driven keyboard. Instead this driver *polls* the
//! PS/2 controller: it repeatedly reads the status port until a byte is
//! ready, then reads the scancode from the data port.
//!
//! Scancodes use "scan code set 1" (the default the BIOS / QEMU provides):
//!   * A "make code" is sent when a key is pressed.
//!   * The matching "break code" (make code | 0x80) is sent on release.
//!   * The arrow keys send a two-byte sequence starting with the `0xE0` prefix.

/// PS/2 controller status register. Bit 0 set => a byte is waiting on 0x60.
const STATUS_PORT: u16 = 0x64;
/// PS/2 data port: scancodes are read from here.
const DATA_PORT: u16 = 0x60;

/// Whether a Shift key is currently held, so letters can be upper/lower case.
static mut SHIFT_HELD: bool = false;
/// Whether a Ctrl key is currently held.
static mut CTRL_HELD: bool = false;

/// A decoded key press, returned by [`read_key`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    /// A printable ASCII character.
    Char(u8),
    Enter,
    Backspace,
    Escape,
    Up,
    Down,
    Left,
    Right,
    Delete,
    CtrlS,
    CtrlX,
    CtrlR,
}

/// Maps scancode set 1 "make codes" (index 0x00..0x39) to ASCII characters.
/// A `0` entry means the key has no printable character (or is handled
/// specially, like Enter or Backspace).
const SCANCODE_SET1: [u8; 0x3A] = [
    0, 0, // 0x00 (none), 0x01 (Esc)
    b'1', b'2', b'3', b'4', b'5', b'6', b'7', b'8', b'9', b'0', // 0x02-0x0B
    b'-', b'=', // 0x0C-0x0D
    0, 0, // 0x0E (Backspace), 0x0F (Tab)
    b'q', b'w', b'e', b'r', b't', b'y', b'u', b'i', b'o', b'p', // 0x10-0x19
    b'[', b']', // 0x1A-0x1B
    0, 0, // 0x1C (Enter), 0x1D (Ctrl)
    b'a', b's', b'd', b'f', b'g', b'h', b'j', b'k', b'l', // 0x1E-0x26
    b';', b'\'', b'`', // 0x27-0x29
    0,    // 0x2A (Left Shift)
    b'\\', // 0x2B
    b'z', b'x', b'c', b'v', b'b', b'n', b'm', // 0x2C-0x32
    b',', b'.', b'/', // 0x33-0x35
    0, 0, 0, // 0x36 (Right Shift), 0x37 (KP*), 0x38 (Alt)
    b' ', // 0x39 (Space)
];

/// Read one byte from an x86 I/O port.
///
/// SAFETY: the caller must pass a valid port. It is only ever called here with
/// the fixed PS/2 controller ports, which are always safe to read.
unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    unsafe {
        core::arch::asm!(
            "in al, dx",
            out("al") value,
            in("dx") port,
            options(nomem, nostack, preserves_flags),
        );
    }
    value
}

/// Block until the keyboard controller has a scancode, then return it.
fn wait_for_scancode() -> u8 {
    loop {
        // SAFETY: STATUS_PORT / DATA_PORT are the fixed PS/2 controller ports.
        let status = unsafe { inb(STATUS_PORT) };
        if status & 1 != 0 {
            return unsafe { inb(DATA_PORT) };
        }
    }
}

/// Translate a make code to an ASCII byte, applying the current Shift state.
/// Returns `0` if the key has no printable character.
fn decode(scancode: u8) -> u8 {
    let index = scancode as usize;
    if index >= SCANCODE_SET1.len() {
        return 0;
    }
    let ascii = SCANCODE_SET1[index];
    if ascii == 0 {
        return 0;
    }
    // SAFETY: plain read of a Copy static in a single-threaded kernel.
    if unsafe { SHIFT_HELD } && ascii.is_ascii_lowercase() {
        ascii - 32 // shift lowercase letter to uppercase
    } else {
        ascii
    }
}

/// Block until a meaningful key is pressed, then return it as a [`Key`].
///
/// Key *releases* are consumed silently (except Shift/Ctrl, whose state is
/// tracked) so callers only ever see actual key presses.
pub fn read_key() -> Key {
    loop {
        let code = wait_for_scancode();

        // Arrow keys and extended keys (Delete, Right Ctrl) arrive as a 0xE0
        // prefix followed by the real code.
        if code == 0xE0 {
            let extended = wait_for_scancode();
            match extended {
                0x48 => return Key::Up,
                0x50 => return Key::Down,
                0x4B => return Key::Left,
                0x4D => return Key::Right,
                0x53 => return Key::Delete,
                // Right Ctrl make / break
                0x1D => { unsafe { CTRL_HELD = true }; continue; }
                0x9D => { unsafe { CTRL_HELD = false }; continue; }
                _ => continue,
            }
        }

        // Update Shift and Ctrl state on press / release.
        match code {
            0x2A | 0x36 => { unsafe { SHIFT_HELD = true }; continue; }
            0xAA | 0xB6 => { unsafe { SHIFT_HELD = false }; continue; }
            // Left Ctrl make (0x1D) and break (0x9D) — must be before the
            // `& 0x80` break-code filter below since 0x9D has bit 7 set.
            0x1D => { unsafe { CTRL_HELD = true }; continue; }
            0x9D => { unsafe { CTRL_HELD = false }; continue; }
            _ => {}
        }

        // Ignore every other break (release) code; bit 7 marks a release.
        if code & 0x80 != 0 {
            continue;
        }

        match code {
            0x1C => return Key::Enter,
            0x0E => return Key::Backspace,
            0x01 => return Key::Escape,
            _ => {
                let ascii = decode(code);
                if ascii != 0 {
                    if unsafe { CTRL_HELD } {
                        // Normalize to lowercase to handle Shift+Ctrl combos.
                        match ascii | 0x20 {
                            b's' => return Key::CtrlS,
                            b'x' => return Key::CtrlX,
                            b'r' => return Key::CtrlR,
                            _ => {} // other Ctrl combos: ignore
                        }
                    } else {
                        return Key::Char(ascii);
                    }
                }
            }
        }
    }
}
