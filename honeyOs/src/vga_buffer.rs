//! VGA text-mode driver for honeyOS.
//!
//! The PC VGA text buffer is memory-mapped at physical address `0xb8000`.
//! It is a grid of 80x25 cells; each cell is two bytes: an ASCII code byte
//! and an attribute byte (foreground/background color).
//!
//! This module exposes two ways to put text on screen:
//!   * `print!` / `println!` — terminal-style output through the global
//!     `WRITER`, which tracks a cursor and scrolls when it reaches the bottom.
//!   * `write_at` — draw a string at an exact (row, column), used by the
//!     interactive shell to paint menus and highlight bars.
//!
//! NOTE: the kernel is single-threaded and runs with interrupts disabled, so
//! the global `static mut WRITER` is only ever touched by one execution flow.
//! That is what makes the `unsafe` accesses below sound.

use core::fmt;
use volatile::Volatile;

/// The 16 colors supported by VGA text mode.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Color {
    Black = 0,
    Blue = 1,
    Green = 2,
    Cyan = 3,
    Red = 4,
    Magenta = 5,
    Brown = 6,
    LightGray = 7,
    DarkGray = 8,
    LightBlue = 9,
    LightGreen = 10,
    LightCyan = 11,
    LightRed = 12,
    Pink = 13,
    Yellow = 14,
    White = 15,
}

/// A packed foreground/background color attribute byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
struct ColorCode(u8);

impl ColorCode {
    /// Build an attribute byte: high nibble = background, low nibble = foreground.
    /// `const` so it can be used to initialize the global `WRITER` statically.
    const fn new(foreground: Color, background: Color) -> ColorCode {
        ColorCode((background as u8) << 4 | (foreground as u8))
    }
}

/// One on-screen cell: an ASCII character plus its color attribute.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
struct ScreenChar {
    ascii_character: u8,
    color_code: ColorCode,
}

const BUFFER_HEIGHT: usize = 25;
const BUFFER_WIDTH: usize = 80;

/// The raw VGA text buffer. `Volatile` prevents the compiler from optimizing
/// away writes it cannot see have any (RAM) effect.
#[repr(transparent)]
struct Buffer {
    chars: [[Volatile<ScreenChar>; BUFFER_WIDTH]; BUFFER_HEIGHT],
}

/// Get a reference to the memory-mapped VGA buffer.
///
/// SAFETY: `0xb8000` is the fixed hardware address of the VGA text buffer; it
/// is always valid for the lifetime of the kernel.
fn vga_buffer() -> &'static mut Buffer {
    unsafe { &mut *(0xb8000 as *mut Buffer) }
}

/// A text cursor over the VGA buffer for terminal-style output.
///
/// New text is written at `(row_position, column_position)`. When the cursor
/// reaches the bottom of the screen, `new_line` scrolls the whole screen up.
pub struct Writer {
    column_position: usize,
    row_position: usize,
    color_code: ColorCode,
}

/// The kernel-wide writer used by the `print!` / `println!` macros.
static mut WRITER: Writer = Writer {
    column_position: 0,
    row_position: 0,
    color_code: ColorCode::new(Color::Yellow, Color::Black),
};

impl Writer {
    /// Write a single byte, handling newlines, backspace and line wrapping.
    pub fn write_byte(&mut self, byte: u8) {
        match byte {
            b'\n' => self.new_line(),
            0x08 => self.backspace(),
            byte => {
                if self.column_position >= BUFFER_WIDTH {
                    self.new_line();
                }

                let row = self.row_position;
                let col = self.column_position;
                let color_code = self.color_code;

                vga_buffer().chars[row][col].write(ScreenChar {
                    ascii_character: byte,
                    color_code,
                });

                self.column_position += 1;
            }
        }
    }

    /// Move to the next line, scrolling the screen up if already at the bottom.
    fn new_line(&mut self) {
        if self.row_position < BUFFER_HEIGHT - 1 {
            self.row_position += 1;
        } else {
            let buffer = vga_buffer();
            for row in 1..BUFFER_HEIGHT {
                for col in 0..BUFFER_WIDTH {
                    let character = buffer.chars[row][col].read();
                    buffer.chars[row - 1][col].write(character);
                }
            }
            self.clear_row(BUFFER_HEIGHT - 1);
        }
        self.column_position = 0;
    }

    /// Erase the character before the cursor (used to echo a Backspace key).
    fn backspace(&mut self) {
        if self.column_position > 0 {
            self.column_position -= 1;
            let blank = ScreenChar {
                ascii_character: b' ',
                color_code: self.color_code,
            };
            vga_buffer().chars[self.row_position][self.column_position].write(blank);
        }
    }

    /// Blank out a single row using the current background color.
    fn clear_row(&mut self, row: usize) {
        let blank = ScreenChar {
            ascii_character: b' ',
            color_code: self.color_code,
        };
        let buffer = vga_buffer();
        for col in 0..BUFFER_WIDTH {
            buffer.chars[row][col].write(blank);
        }
    }

    /// Write a string, substituting `0xfe` (■) for non-printable bytes.
    pub fn write_string(&mut self, s: &str) {
        for byte in s.bytes() {
            match byte {
                0x20..=0x7e | b'\n' | 0x08 => self.write_byte(byte),
                _ => self.write_byte(0xfe),
            }
        }
    }
}

/// Lets `write!` / `writeln!` and the print macros target the `Writer`.
impl fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_string(s);
        Ok(())
    }
}

/// Backing function for the `print!` macro. Not meant to be called directly.
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    // SAFETY: single-threaded kernel with interrupts disabled — no aliasing.
    unsafe {
        let writer = &mut *(&raw mut WRITER);
        writer.write_fmt(args).unwrap();
    }
}

/// Clear the entire screen and move the cursor back to the top-left.
pub fn clear_screen() {
    // SAFETY: single-threaded kernel with interrupts disabled — no aliasing.
    unsafe {
        let writer = &mut *(&raw mut WRITER);
        for row in 0..BUFFER_HEIGHT {
            writer.clear_row(row);
        }
        writer.row_position = 0;
        writer.column_position = 0;
    }
}

/// Change the color used by `print!` / `println!` from now on.
pub fn set_text_color(foreground: Color, background: Color) {
    // SAFETY: single-threaded kernel with interrupts disabled — no aliasing.
    unsafe {
        let writer = &mut *(&raw mut WRITER);
        writer.color_code = ColorCode::new(foreground, background);
    }
}

/// Erase the character before the cursor (public wrapper for the shell's
/// line editor, so it can visually undo a typed character on Backspace).
pub fn backspace() {
    // SAFETY: single-threaded kernel with interrupts disabled — no aliasing.
    unsafe {
        let writer = &mut *(&raw mut WRITER);
        writer.backspace();
    }
}

/// Draw `text` starting at an exact (row, column) with the given colors.
///
/// Unlike `print!`, this does not move the global cursor or scroll — it is
/// meant for painting fixed UI elements such as the shell's menu. Drawing is
/// clipped to the screen edges.
pub fn write_at(row: usize, col: usize, text: &str, foreground: Color, background: Color) {
    if row >= BUFFER_HEIGHT {
        return;
    }
    let color_code = ColorCode::new(foreground, background);
    let buffer = vga_buffer();
    let mut column = col;
    for byte in text.bytes() {
        if column >= BUFFER_WIDTH {
            break;
        }
        let ascii = match byte {
            0x20..=0x7e => byte,
            _ => 0xfe,
        };
        buffer.chars[row][column].write(ScreenChar {
            ascii_character: ascii,
            color_code,
        });
        column += 1;
    }
}

/// Draw a single byte at an exact (row, column) with the given colors.
///
/// Non-printable bytes are rendered as spaces so callers can safely use it for
/// cursor cells and editor overlays.
pub fn write_byte_at(row: usize, col: usize, byte: u8, foreground: Color, background: Color) {
    if row >= BUFFER_HEIGHT || col >= BUFFER_WIDTH {
        return;
    }

    let ascii = match byte {
        0x20..=0x7e => byte,
        _ => b' ',
    };

    vga_buffer().chars[row][col].write(ScreenChar {
        ascii_character: ascii,
        color_code: ColorCode::new(foreground, background),
    });
}

/// Print formatted text to the VGA buffer (no trailing newline).
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::vga_buffer::_print(format_args!($($arg)*)));
}

/// Print formatted text to the VGA buffer followed by a newline.
#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}
