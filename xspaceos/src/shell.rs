//! Interactive UI for XSpace OS.
//!
//! Screen flow:
//!   Desktop  →  File Manager  →  Editor
//!
//! Desktop: shows available apps; only "File Manager" is wired up.
//! File Manager: navigable file list; Enter opens/creates, Del removes.
//! Editor: full-screen text editor with Ctrl+S (save), Ctrl+X (close),
//!         Ctrl+R (rename).

use core::fmt::Write;

use crate::{print, println};
use crate::fs::{FileSystem, MAX_CONTENT_LEN};
use crate::keyboard::{self, Key};
use crate::vga_buffer::{self, Color};

const DESKTOP_APPS: [&str; 1] = ["File Manager"];

// ── LineBuf ──────────────────────────────────────────────────────────────────

/// Fixed-capacity string buffer — the `no_std` stand-in for `String`.
struct LineBuf {
    buf: [u8; 80],
    len: usize,
}

impl LineBuf {
    fn new() -> LineBuf {
        LineBuf { buf: [b' '; 80], len: 0 }
    }

    fn as_str(&self) -> &str {
        core::str::from_utf8(&self.buf[..self.len]).unwrap_or("")
    }

    fn pad_to(&mut self, width: usize) {
        while self.len < width && self.len < self.buf.len() {
            self.buf[self.len] = b' ';
            self.len += 1;
        }
    }

    fn push(&mut self, byte: u8) {
        if self.len < self.buf.len() {
            self.buf[self.len] = byte;
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

// ── Entry point ──────────────────────────────────────────────────────────────

/// Run the OS UI forever. This is the kernel's main loop.
pub fn run(fs: &mut FileSystem) -> ! {
    loop {
        desktop(fs);
    }
}

// ── Desktop ──────────────────────────────────────────────────────────────────

fn draw_desktop(selected: usize) {
    vga_buffer::clear_screen();

    vga_buffer::write_at(1, 2, "XSpace OS", Color::Yellow, Color::Black);
    vga_buffer::write_at(
        2, 2,
        "=========================================",
        Color::DarkGray, Color::Black,
    );
    vga_buffer::write_at(4, 2, "Desktop", Color::White, Color::Black);

    for (i, app) in DESKTOP_APPS.iter().enumerate() {
        let mut line = LineBuf::new();
        let marker = if i == selected { '>' } else { ' ' };
        let _ = write!(line, "  {} {}", marker, app);
        line.pad_to(30);
        if i == selected {
            vga_buffer::write_at(6 + i, 2, line.as_str(), Color::Black, Color::Cyan);
        } else {
            vga_buffer::write_at(6 + i, 2, line.as_str(), Color::LightGray, Color::Black);
        }
    }

    vga_buffer::write_at(
        23, 2,
        "Up/Down = move selection     Enter = open",
        Color::DarkGray, Color::Black,
    );
}

fn desktop(fs: &mut FileSystem) {
    let mut selected = 0usize;
    loop {
        draw_desktop(selected);
        match keyboard::read_key() {
            Key::Up => {
                if selected > 0 { selected -= 1; }
            }
            Key::Down => {
                if selected + 1 < DESKTOP_APPS.len() { selected += 1; }
            }
            Key::Enter => {
                if selected == 0 { file_manager(fs); }
            }
            _ => {}
        }
    }
}

// ── File Manager ─────────────────────────────────────────────────────────────

/// Copy the name of the `idx`-th live file into a stack buffer.
/// Returns `([u8; 32], len)`; len == 0 means not found.
fn copy_file_name_at(fs: &FileSystem, idx: usize) -> ([u8; 32], usize) {
    let mut buf = [0u8; 32];
    let mut count = 0usize;
    let mut result_len = 0usize;
    fs.for_each_file(|name, _size| {
        if count == idx {
            let len = name.len().min(32);
            buf[..len].copy_from_slice(&name.as_bytes()[..len]);
            result_len = len;
        }
        count += 1;
    });
    (buf, result_len)
}

fn draw_file_manager(fs: &FileSystem, selected: usize) {
    vga_buffer::clear_screen();

    vga_buffer::write_at(0, 2, "XSpace OS  --  File Manager", Color::Yellow, Color::Black);
    vga_buffer::write_at(
        1, 2,
        "=========================================",
        Color::DarkGray, Color::Black,
    );

    // Index 0 is always "New File".
    let mut new_line = LineBuf::new();
    let new_marker = if selected == 0 { '>' } else { ' ' };
    let _ = write!(new_line, "  {} [ + New File ]", new_marker);
    new_line.pad_to(30);
    if selected == 0 {
        vga_buffer::write_at(3, 2, new_line.as_str(), Color::Black, Color::Green);
    } else {
        vga_buffer::write_at(3, 2, new_line.as_str(), Color::LightGreen, Color::Black);
    }

    vga_buffer::write_at(4, 2, "-----------------------------------------", Color::DarkGray, Color::Black);

    // Indices 1..=file_count are the files.
    let file_count = fs.file_count();
    if file_count == 0 {
        vga_buffer::write_at(5, 4, "(no files yet)", Color::DarkGray, Color::Black);
    } else {
        let mut row = 5usize;
        let mut file_idx = 0usize;
        fs.for_each_file(|name, size| {
            let list_idx = file_idx + 1;
            let marker = if list_idx == selected { '>' } else { ' ' };
            let mut line = LineBuf::new();
            let _ = write!(line, "  {} {}  ({} bytes)", marker, name, size);
            line.pad_to(50);
            if list_idx == selected {
                vga_buffer::write_at(row, 2, line.as_str(), Color::Black, Color::Cyan);
            } else {
                vga_buffer::write_at(row, 2, line.as_str(), Color::White, Color::Black);
            }
            row += 1;
            file_idx += 1;
        });
    }

    vga_buffer::write_at(
        23, 2,
        "Up/Down=move  Enter=open  Del=delete  Esc=back",
        Color::DarkGray, Color::Black,
    );
}

fn file_manager(fs: &mut FileSystem) {
    let mut selected = 0usize;
    loop {
        draw_file_manager(fs, selected);
        let file_count = fs.file_count();

        match keyboard::read_key() {
            Key::Up => {
                if selected > 0 { selected -= 1; }
            }
            Key::Down => {
                if selected < file_count { selected += 1; }
            }
            Key::Enter => {
                if selected == 0 {
                    // Prompt for a name, create the file, open editor.
                    vga_buffer::clear_screen();
                    vga_buffer::set_text_color(Color::Yellow, Color::Black);
                    println!("=== New File ===");
                    vga_buffer::set_text_color(Color::White, Color::Black);
                    println!();
                    print!("File name: ");
                    let mut name_buf = [0u8; 32];
                    let name_len = read_line(&mut name_buf);
                    println!();
                    if name_len == 0 {
                        continue;
                    }
                    let name = core::str::from_utf8(&name_buf[..name_len]).unwrap_or("");
                    match fs.create(name) {
                        Ok(()) => {
                            editor(fs, name_buf, name_len);
                        }
                        Err(e) => {
                            vga_buffer::set_text_color(Color::LightRed, Color::Black);
                            println!("Error: {}", e.as_str());
                            vga_buffer::set_text_color(Color::White, Color::Black);
                            println!();
                            println!("Press any key...");
                            keyboard::read_key();
                        }
                    }
                } else {
                    let (name_buf, name_len) = copy_file_name_at(fs, selected - 1);
                    if name_len > 0 {
                        editor(fs, name_buf, name_len);
                    }
                }
                selected = 0;
            }
            Key::Delete => {
                if selected > 0 {
                    let (name_buf, name_len) = copy_file_name_at(fs, selected - 1);
                    if name_len > 0 {
                        let name = core::str::from_utf8(&name_buf[..name_len]).unwrap_or("");
                        let _ = fs.delete(name);
                        let new_count = fs.file_count();
                        if selected > new_count { selected = new_count; }
                    }
                }
            }
            Key::Escape => return,
            _ => {}
        }
    }
}

// ── Editor ───────────────────────────────────────────────────────────────────

fn draw_editor(
    name_buf: &[u8],
    name_len: usize,
    content: &[u8],
    content_len: usize,
    saved: bool,
) {
    vga_buffer::clear_screen();

    // Header bar spanning the full width.
    let name = core::str::from_utf8(&name_buf[..name_len]).unwrap_or("?");
    let mut header = LineBuf::new();
    let _ = write!(header, " [ {} ] ", name);
    header.pad_to(80);
    vga_buffer::write_at(0, 0, header.as_str(), Color::Black, Color::Cyan);
    // Overwrite the right side with key hints.
    vga_buffer::write_at(0, 38, "Ctrl+S:Save  Ctrl+X:Close  Ctrl+R:Rename", Color::Black, Color::Cyan);

    // Separator.
    vga_buffer::write_at(
        1, 0,
        "--------------------------------------------------------------------------------",
        Color::DarkGray, Color::Black,
    );

    // Content area: rows 2..=22 (21 rows × 80 cols).
    let mut row = 2usize;
    let mut line = LineBuf::new();
    for i in 0..content_len {
        if row > 22 { break; }
        let byte = content[i];
        if byte == b'\n' {
            vga_buffer::write_at(row, 0, line.as_str(), Color::White, Color::Black);
            row += 1;
            line = LineBuf::new();
        } else {
            line.push(byte);
            if line.len == 80 {
                vga_buffer::write_at(row, 0, line.as_str(), Color::White, Color::Black);
                row += 1;
                line = LineBuf::new();
            }
        }
    }
    // Draw cursor at the current insertion point.
    if row <= 22 {
        line.push(b'_');
        vga_buffer::write_at(row, 0, line.as_str(), Color::White, Color::Black);
    }

    // Status bar.
    let status_label = if saved { "Saved  " } else { "Unsaved" };
    let mut status = LineBuf::new();
    let _ = write!(status, " {} | {} / {} bytes ", status_label, content_len, MAX_CONTENT_LEN);
    status.pad_to(80);
    let status_fg = if saved { Color::LightGreen } else { Color::Yellow };
    vga_buffer::write_at(23, 0, status.as_str(), Color::Black, status_fg);
}

fn editor(fs: &mut FileSystem, mut name_buf: [u8; 32], mut name_len: usize) {
    let mut content = [0u8; MAX_CONTENT_LEN];
    let mut content_len = 0usize;
    let mut saved = true;

    // Load existing content into the edit buffer.
    {
        let name = core::str::from_utf8(&name_buf[..name_len]).unwrap_or("");
        if let Ok(existing) = fs.read(name) {
            let bytes = existing.as_bytes();
            let len = bytes.len().min(MAX_CONTENT_LEN);
            content[..len].copy_from_slice(&bytes[..len]);
            content_len = len;
        }
    }

    loop {
        draw_editor(&name_buf, name_len, &content, content_len, saved);

        match keyboard::read_key() {
            Key::CtrlS => {
                let name = core::str::from_utf8(&name_buf[..name_len]).unwrap_or("");
                let text = core::str::from_utf8(&content[..content_len]).unwrap_or("");
                if fs.save(name, text).is_ok() {
                    saved = true;
                }
            }
            Key::CtrlX => return,
            Key::CtrlR => {
                vga_buffer::clear_screen();
                vga_buffer::set_text_color(Color::Yellow, Color::Black);
                println!("=== Rename File ===");
                vga_buffer::set_text_color(Color::White, Color::Black);
                println!();
                let old = core::str::from_utf8(&name_buf[..name_len]).unwrap_or("");
                println!("Current name: {}", old);
                println!();
                print!("New name: ");
                let mut new_buf = [0u8; 32];
                let new_len = read_line(&mut new_buf);
                println!();

                if new_len == 0 {
                    continue;
                }

                let new_name = core::str::from_utf8(&new_buf[..new_len]).unwrap_or("");
                let old_name = core::str::from_utf8(&name_buf[..name_len]).unwrap_or("");
                match fs.rename(old_name, new_name) {
                    Ok(()) => {
                        // Clear stale bytes from the longer old name first.
                        for b in name_buf[new_len..name_len].iter_mut() { *b = 0; }
                        name_buf[..new_len].copy_from_slice(&new_buf[..new_len]);
                        name_len = new_len;
                    }
                    Err(e) => {
                        vga_buffer::set_text_color(Color::LightRed, Color::Black);
                        println!("Error: {}", e.as_str());
                        vga_buffer::set_text_color(Color::White, Color::Black);
                        println!();
                        println!("Press any key...");
                        keyboard::read_key();
                    }
                }
            }
            Key::Backspace => {
                if content_len > 0 {
                    content_len -= 1;
                    saved = false;
                }
            }
            Key::Enter => {
                if content_len < MAX_CONTENT_LEN {
                    content[content_len] = b'\n';
                    content_len += 1;
                    saved = false;
                }
            }
            Key::Char(c) => {
                if content_len < MAX_CONTENT_LEN {
                    content[content_len] = c;
                    content_len += 1;
                    saved = false;
                }
            }
            _ => {}
        }
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Read a typed line into `buf`, echoing characters to the terminal.
/// Returns the number of bytes written.
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
            _ => {}
        }
    }
    len
}
