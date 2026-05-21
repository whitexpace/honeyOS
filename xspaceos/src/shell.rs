//! Interactive text-mode shell for XSpace OS.
//!
//! This is the user-facing front end of the kernel. It draws a menu, lets the
//! user move a highlighted selection with the Up/Down arrow keys, and runs the
//! chosen file-system operation when Enter is pressed.
//!
//! Screen flow:
//!   * `draw_menu` paints the main menu plus the current list of files.
//!   * Choosing an action switches to a prompt screen where the user types a
//!     file name (and, where relevant, file contents) using `read_line`.
//!   * After each action the result is shown and the shell returns to the menu.

use core::fmt::Write;

use crate::{print, println};
use crate::fs::FileSystem;
use crate::keyboard::{self, Key};
use crate::vga_buffer::{self, Color};

/// The selectable actions on the main menu, in display order.
const MENU: [&str; 5] = [
    "Create a file",
    "Write / save a file",
    "Edit a file (append text)",
    "View a file",
    "Delete a file",
];

/// A fixed-capacity line buffer used to build strings without a heap.
///
/// XSpace OS is `no_std` with no allocator, so anything that needs a formatted
/// string (`write!(...)`) builds it into one of these stack buffers instead.
struct LineBuf {
    buf: [u8; 80],
    len: usize,
}

impl LineBuf {
    fn new() -> LineBuf {
        LineBuf {
            buf: [b' '; 80],
            len: 0,
        }
    }

    /// View the written bytes as a string slice.
    fn as_str(&self) -> &str {
        core::str::from_utf8(&self.buf[..self.len]).unwrap_or("")
    }

    /// Pad the buffer with spaces up to `width`, so a highlight bar can span a
    /// uniform width regardless of the menu label length.
    fn pad_to(&mut self, width: usize) {
        while self.len < width && self.len < self.buf.len() {
            self.buf[self.len] = b' ';
            self.len += 1;
        }
    }
}

impl Write for LineBuf {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for byte in s.bytes() {
            if self.len < self.buf.len() {
                self.buf[self.len] = byte;
                self.len += 1;
            }
        }
        Ok(())
    }
}

/// Run the shell forever. This is the kernel's main loop.
pub fn run(fs: &mut FileSystem) -> ! {
    let mut selected = 0usize;
    loop {
        draw_menu(fs, selected);
        match keyboard::read_key() {
            // Wrap around at the ends of the list.
            Key::Up => {
                selected = if selected == 0 {
                    MENU.len() - 1
                } else {
                    selected - 1
                };
            }
            Key::Down => {
                selected = (selected + 1) % MENU.len();
            }
            Key::Enter => match selected {
                0 => action_create(fs),
                1 => action_save(fs),
                2 => action_edit(fs),
                3 => action_view(fs),
                4 => action_delete(fs),
                _ => {}
            },
            _ => {}
        }
    }
}

/// Paint the main menu screen: title, selectable actions, and the file list.
fn draw_menu(fs: &FileSystem, selected: usize) {
    vga_buffer::clear_screen();

    vga_buffer::write_at(0, 2, "XSpace OS  --  File Manager", Color::Yellow, Color::Black);
    vga_buffer::write_at(
        1,
        2,
        "=========================================",
        Color::DarkGray,
        Color::Black,
    );
    vga_buffer::write_at(3, 2, "Main menu:", Color::White, Color::Black);

    // Draw each menu item; the selected one gets an inverted highlight bar.
    for (i, item) in MENU.iter().enumerate() {
        let mut line = LineBuf::new();
        let marker = if i == selected { '>' } else { ' ' };
        let _ = write!(line, "  {} {}", marker, item);
        line.pad_to(40);

        if i == selected {
            vga_buffer::write_at(5 + i, 2, line.as_str(), Color::Black, Color::Cyan);
        } else {
            vga_buffer::write_at(5 + i, 2, line.as_str(), Color::LightGray, Color::Black);
        }
    }

    // Show the current contents of the in-memory file system.
    let mut header = LineBuf::new();
    let _ = write!(header, "Files in memory ({}):", fs.file_count());
    vga_buffer::write_at(12, 2, header.as_str(), Color::White, Color::Black);

    if fs.file_count() == 0 {
        vga_buffer::write_at(13, 4, "(no files yet)", Color::DarkGray, Color::Black);
    } else {
        let mut row = 13usize;
        fs.for_each_file(|name, size| {
            let mut line = LineBuf::new();
            let _ = write!(line, "- {}  ({} bytes)", name, size);
            vga_buffer::write_at(row, 4, line.as_str(), Color::LightGreen, Color::Black);
            row += 1;
        });
    }

    vga_buffer::write_at(
        23,
        2,
        "Up / Down = move selection     Enter = choose",
        Color::White,
        Color::Black,
    );
}

/// Read a line of text typed by the user into `buf`, returning its length.
///
/// Characters are echoed to the screen as they are typed, Backspace erases the
/// last character, and Enter finishes the line.
fn read_line(buf: &mut [u8]) -> usize {
    let mut len = 0usize;
    loop {
        match keyboard::read_key() {
            Key::Enter => break,
            Key::Backspace => {
                if len > 0 {
                    len -= 1;
                    vga_buffer::backspace();
                }
            }
            Key::Char(c) => {
                if len < buf.len() {
                    buf[len] = c;
                    len += 1;
                    print!("{}", c as char);
                }
            }
            _ => {} // ignore arrows / Escape while editing a line
        }
    }
    len
}

/// Show a `label`, read a typed line, and return the raw bytes plus length.
/// The const parameter `N` bounds how many characters can be typed.
fn prompt<const N: usize>(label: &str) -> ([u8; N], usize) {
    let mut buf = [0u8; N];
    print!("{}", label);
    let len = read_line(&mut buf);
    println!();
    (buf, len)
}

/// Wait for any key, used to hold a result screen until the user is ready.
fn pause() {
    vga_buffer::set_text_color(Color::White, Color::Black);
    println!();
    println!("Press any key to return to the menu...");
    keyboard::read_key();
}

/// Print a result line in green (success) or red (failure).
fn report(ok: bool, message: &str) {
    if ok {
        vga_buffer::set_text_color(Color::LightGreen, Color::Black);
    } else {
        vga_buffer::set_text_color(Color::LightRed, Color::Black);
    }
    println!("{}", message);
}

/// Begin a prompt screen with a colored heading.
fn begin_screen(title: &str) {
    vga_buffer::clear_screen();
    vga_buffer::set_text_color(Color::Yellow, Color::Black);
    println!("=== {} ===", title);
    vga_buffer::set_text_color(Color::White, Color::Black);
    println!();
}

/// CREATE: ask for a name and register a new, empty file.
fn action_create(fs: &mut FileSystem) {
    begin_screen("Create a file");
    let (name_buf, name_len) = prompt::<32>("New file name: ");
    let name = core::str::from_utf8(&name_buf[..name_len]).unwrap_or("");

    match fs.create(name) {
        Ok(()) => {
            let mut msg = LineBuf::new();
            let _ = write!(msg, "Created file '{}'.", name);
            report(true, msg.as_str());
        }
        Err(e) => {
            let mut msg = LineBuf::new();
            let _ = write!(msg, "Could not create file: {}", e.as_str());
            report(false, msg.as_str());
        }
    }
    pause();
}

/// SAVE: ask for a name and new contents, then replace the file's contents.
fn action_save(fs: &mut FileSystem) {
    begin_screen("Write / save a file");
    let (name_buf, name_len) = prompt::<32>("File name: ");
    let name = core::str::from_utf8(&name_buf[..name_len]).unwrap_or("");
    let (text_buf, text_len) = prompt::<256>("Contents: ");
    let text = core::str::from_utf8(&text_buf[..text_len]).unwrap_or("");

    match fs.save(name, text) {
        Ok(()) => report(true, "File saved."),
        Err(e) => {
            let mut msg = LineBuf::new();
            let _ = write!(msg, "Could not save file: {}", e.as_str());
            report(false, msg.as_str());
        }
    }
    pause();
}

/// EDIT: ask for a name and extra text, then append it to the file.
fn action_edit(fs: &mut FileSystem) {
    begin_screen("Edit a file (append text)");
    let (name_buf, name_len) = prompt::<32>("File name: ");
    let name = core::str::from_utf8(&name_buf[..name_len]).unwrap_or("");

    // Show the current contents so the user knows what they are extending.
    match fs.read(name) {
        Ok(text) => println!("Current contents: \"{}\"", text),
        Err(e) => println!("(could not read file: {})", e.as_str()),
    }
    println!();

    let (text_buf, text_len) = prompt::<256>("Text to append: ");
    let text = core::str::from_utf8(&text_buf[..text_len]).unwrap_or("");

    match fs.edit(name, text) {
        Ok(()) => report(true, "File updated."),
        Err(e) => {
            let mut msg = LineBuf::new();
            let _ = write!(msg, "Could not edit file: {}", e.as_str());
            report(false, msg.as_str());
        }
    }
    pause();
}

/// VIEW: ask for a name and print the file's contents.
fn action_view(fs: &mut FileSystem) {
    begin_screen("View a file");
    let (name_buf, name_len) = prompt::<32>("File name: ");
    let name = core::str::from_utf8(&name_buf[..name_len]).unwrap_or("");

    match fs.read(name) {
        Ok(text) => {
            vga_buffer::set_text_color(Color::LightCyan, Color::Black);
            println!("Contents of '{}':", name);
            vga_buffer::set_text_color(Color::White, Color::Black);
            println!("{}", text);
        }
        Err(e) => {
            let mut msg = LineBuf::new();
            let _ = write!(msg, "Could not read file: {}", e.as_str());
            report(false, msg.as_str());
        }
    }
    pause();
}

/// DELETE: ask for a name and remove the file.
fn action_delete(fs: &mut FileSystem) {
    begin_screen("Delete a file");
    let (name_buf, name_len) = prompt::<32>("File name: ");
    let name = core::str::from_utf8(&name_buf[..name_len]).unwrap_or("");

    match fs.delete(name) {
        Ok(()) => {
            let mut msg = LineBuf::new();
            let _ = write!(msg, "Deleted file '{}'.", name);
            report(true, msg.as_str());
        }
        Err(e) => {
            let mut msg = LineBuf::new();
            let _ = write!(msg, "Could not delete file: {}", e.as_str());
            report(false, msg.as_str());
        }
    }
    pause();
}
