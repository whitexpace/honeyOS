//! Interactive UI for HoneyOS.
//!
//! Screen flow:
//!   Desktop  →  File Manager  →  Editor
//!
//! Desktop: shows available apps; only "File Manager" is wired up.
//! File Manager: navigable file list; Enter opens/creates, Del removes.
//! Editor: full-screen text editor with Ctrl+S (save), Ctrl+X (close),
//!         Ctrl+R (rename).

use core::fmt::Write;

use crate::fs::{BlockKind, FileSystem, MAX_CONTENT_LEN, TOTAL_BLOCKS};
use crate::keyboard::{self, Key};
use crate::vga_buffer::{self, Color};
use crate::{print, println};

const DESKTOP_APPS: [&str; 2] = ["File Manager", "Allocation Table"];
/// First visible row used by the editor's text area.
const EDITOR_FIRST_ROW: usize = 2;
/// Last visible row used by the editor's text area.
const EDITOR_LAST_ROW: usize = 22;
/// Number of text columns in VGA text mode.
const EDITOR_WIDTH: usize = 80;
/// Number of visible rows available for editor text.
const EDITOR_VIEW_ROWS: usize = EDITOR_LAST_ROW - EDITOR_FIRST_ROW + 1;

// ── LineBuf ──────────────────────────────────────────────────────────────────

/// Fixed-capacity string buffer — the `no_std` stand-in for `String`.
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

    fn as_str(&self) -> &str {
        core::str::from_utf8(&self.buf[..self.len]).unwrap_or("")
    }

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

    vga_buffer::write_at(1, 2, "HoneyOS", Color::Yellow, Color::Black);
    vga_buffer::write_at(
        2,
        2,
        "=========================================",
        Color::DarkGray,
        Color::Black,
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
        23,
        2,
        "Up/Down = move selection     Enter = open",
        Color::DarkGray,
        Color::Black,
    );
}

fn desktop(fs: &mut FileSystem) {
    let mut selected = 0usize;
    loop {
        draw_desktop(selected);
        match keyboard::read_key() {
            Key::Up => {
                if selected > 0 {
                    selected -= 1;
                }
            }
            Key::Down => {
                if selected + 1 < DESKTOP_APPS.len() {
                    selected += 1;
                }
            }
            Key::Enter => match selected {
                0 => file_manager(fs),
                1 => allocation_table(fs),
                _ => {}
            },
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

    vga_buffer::write_at(
        0,
        2,
        "HoneyOS  --  File Manager",
        Color::Yellow,
        Color::Black,
    );
    vga_buffer::write_at(
        1,
        2,
        "=========================================",
        Color::DarkGray,
        Color::Black,
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

    vga_buffer::write_at(
        4,
        2,
        "-----------------------------------------",
        Color::DarkGray,
        Color::Black,
    );

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
        23,
        2,
        "Up/Down=move  Enter=open  Del=delete  Esc=back",
        Color::DarkGray,
        Color::Black,
    );
}

fn file_manager(fs: &mut FileSystem) {
    let mut selected = 0usize;
    loop {
        draw_file_manager(fs, selected);
        let file_count = fs.file_count();

        match keyboard::read_key() {
            Key::Up => {
                if selected > 0 {
                    selected -= 1;
                }
            }
            Key::Down => {
                if selected < file_count {
                    selected += 1;
                }
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
                        if selected > new_count {
                            selected = new_count;
                        }
                    }
                }
            }
            Key::Escape => return,
            _ => {}
        }
    }
}

// ── Allocation Table ────────────────────────────────────────────────────────

/// Draw a compact two-column allocation table.
///
/// Each block row shows the block id, whether the block is free or used, the
/// block kind (`IDX` / `DAT`), and the owning file name when allocated.
fn draw_allocation_table(fs: &FileSystem, page: usize) {
    vga_buffer::clear_screen();

    vga_buffer::write_at(
        0,
        2,
        "HoneyOS  --  Allocation Table",
        Color::Yellow,
        Color::Black,
    );
    vga_buffer::write_at(
        1,
        2,
        "=========================================",
        Color::DarkGray,
        Color::Black,
    );

    let rows_per_page = 10usize;
    let blocks_per_page = rows_per_page * 2;
    let start_block = page * blocks_per_page;

    for row in 0..rows_per_page {
        for column in 0..2usize {
            let block_idx = start_block + row + column * rows_per_page;
            if block_idx >= TOTAL_BLOCKS {
                continue;
            }

            let info = fs.block_info(block_idx);
            let state = if info.used { "USED" } else { "FREE" };
            let kind = match info.kind {
                Some(BlockKind::Index) => "IDX",
                Some(BlockKind::Data) => "DAT",
                None => "---",
            };
            let owner = info.owner.unwrap_or("-");
            let mut line = LineBuf::new();
            let _ = write!(
                line,
                "B{:02} {:4} {:3} {:12.12}",
                block_idx, state, kind, owner
            );
            line.pad_to(34);
            vga_buffer::write_at(
                4 + row,
                2 + column * 38,
                line.as_str(),
                Color::White,
                Color::Black,
            );
        }
    }

    let total_pages = TOTAL_BLOCKS.div_ceil(rows_per_page * 2);
    let mut footer = LineBuf::new();
    let _ = write!(
        footer,
        "Left/Right = page   Esc = desktop   Page {}/{}",
        page + 1,
        total_pages,
    );
    footer.pad_to(80);
    vga_buffer::write_at(23, 0, footer.as_str(), Color::DarkGray, Color::Black);
}

fn allocation_table(fs: &FileSystem) {
    let rows_per_page = 10usize;
    let blocks_per_page = rows_per_page * 2;
    let total_pages = TOTAL_BLOCKS.div_ceil(blocks_per_page);
    let mut page = 0usize;

    loop {
        draw_allocation_table(fs, page);
        match keyboard::read_key() {
            Key::Left => {
                if page > 0 {
                    page -= 1;
                }
            }
            Key::Right => {
                if page + 1 < total_pages {
                    page += 1;
                }
            }
            Key::Escape => return,
            _ => {}
        }
    }
}

// ── Editor ───────────────────────────────────────────────────────────────────

/// Render the full-screen text editor.
///
/// The editor uses a logical insertion cursor, so text can be edited anywhere
/// in the buffer instead of only being appended at the end.
fn draw_editor(
    name_buf: &[u8],
    name_len: usize,
    content: &[u8],
    content_len: usize,
    cursor_idx: usize,
    viewport_top_row: usize,
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
    vga_buffer::write_at(
        0,
        38,
        "Ctrl+S:Save  Ctrl+X:Close  Ctrl+R:Rename",
        Color::Black,
        Color::Cyan,
    );

    // Separator.
    vga_buffer::write_at(
        1,
        0,
        "--------------------------------------------------------------------------------",
        Color::DarkGray,
        Color::Black,
    );

    // Content area: rows 2..=22 (21 rows × 80 cols).
    let mut text_row = 0usize;
    let mut col = 0usize;
    for i in 0..content_len {
        let byte = content[i];
        if byte == b'\n' {
            text_row += 1;
            col = 0;
        } else {
            if text_row >= viewport_top_row && text_row < viewport_top_row + EDITOR_VIEW_ROWS {
                let screen_row = EDITOR_FIRST_ROW + (text_row - viewport_top_row);
                vga_buffer::write_byte_at(screen_row, col, byte, Color::White, Color::Black);
            }
            col += 1;
            if col == EDITOR_WIDTH {
                text_row += 1;
                col = 0;
            }
        }
    }

    // Draw the insertion cursor by inverting the character at the current
    // position, or a blank cell if the cursor is at the end of a line/file.
    let (cursor_text_row, cursor_col) = editor_position_for_index(content, content_len, cursor_idx);
    if cursor_text_row >= viewport_top_row && cursor_text_row < viewport_top_row + EDITOR_VIEW_ROWS
    {
        let cursor_row = EDITOR_FIRST_ROW + (cursor_text_row - viewport_top_row);
        let cursor_byte = if cursor_idx < content_len {
            match content[cursor_idx] {
                b'\n' => b' ',
                byte => byte,
            }
        } else {
            b' '
        };
        vga_buffer::write_byte_at(
            cursor_row,
            cursor_col,
            cursor_byte,
            Color::Black,
            Color::White,
        );
    }

    // Status bar.
    let status_label = if saved { "Saved  " } else { "Unsaved" };
    let caps_label = if keyboard::caps_lock_on() {
        "CAPS"
    } else {
        "    "
    };
    let mut status = LineBuf::new();
    let _ = write!(
        status,
        " {} | {} | {} / {} bytes | Home End PgUp PgDn ",
        status_label, caps_label, content_len, MAX_CONTENT_LEN,
    );
    status.pad_to(80);
    let status_fg = if saved {
        Color::LightGreen
    } else {
        Color::Yellow
    };
    vga_buffer::write_at(23, 0, status.as_str(), Color::Black, status_fg);
}

fn editor(fs: &mut FileSystem, mut name_buf: [u8; 32], mut name_len: usize) {
    let mut content = [0u8; MAX_CONTENT_LEN];
    let mut content_len = 0usize;
    let mut cursor_idx = 0usize;
    let mut viewport_top_row = 0usize;
    let mut saved = true;

    // Load existing content into the edit buffer.
    {
        let name = core::str::from_utf8(&name_buf[..name_len]).unwrap_or("");
        if let Ok(len) = fs.read_into(name, &mut content) {
            content_len = len;
            cursor_idx = len;
        }
    }
    let mut preferred_col = cursor_visual_col(&content, content_len, cursor_idx);

    loop {
        viewport_top_row = clamp_viewport_top_row(
            content.as_slice(),
            content_len,
            cursor_idx,
            viewport_top_row,
        );
        draw_editor(
            &name_buf,
            name_len,
            &content,
            content_len,
            cursor_idx,
            viewport_top_row,
            saved,
        );

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
                        // Clear stale bytes only when the previous name was longer.
                        if new_len < name_len {
                            for b in name_buf[new_len..name_len].iter_mut() {
                                *b = 0;
                            }
                        }
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
                if cursor_idx > 0 {
                    let remove_at = cursor_idx - 1;
                    for i in remove_at..content_len.saturating_sub(1) {
                        content[i] = content[i + 1];
                    }
                    content_len -= 1;
                    cursor_idx -= 1;
                    preferred_col = cursor_visual_col(&content, content_len, cursor_idx);
                    saved = false;
                }
            }
            Key::Delete => {
                if cursor_idx < content_len {
                    for i in cursor_idx..content_len - 1 {
                        content[i] = content[i + 1];
                    }
                    content_len -= 1;
                    preferred_col = cursor_visual_col(&content, content_len, cursor_idx);
                    saved = false;
                }
            }
            Key::Left => {
                cursor_idx = cursor_idx.saturating_sub(1);
                preferred_col = cursor_visual_col(&content, content_len, cursor_idx);
            }
            Key::Right => {
                if cursor_idx < content_len {
                    cursor_idx += 1;
                }
                preferred_col = cursor_visual_col(&content, content_len, cursor_idx);
            }
            Key::Up => {
                let (row, _) = editor_position_for_index(&content, content_len, cursor_idx);
                if row > 0 {
                    cursor_idx =
                        editor_index_for_position(&content, content_len, row - 1, preferred_col);
                }
            }
            Key::Down => {
                let (row, _) = editor_position_for_index(&content, content_len, cursor_idx);
                let next_idx =
                    editor_index_for_position(&content, content_len, row + 1, preferred_col);
                if next_idx != cursor_idx {
                    cursor_idx = next_idx;
                }
            }
            Key::Home => {
                let (row, _) = editor_position_for_index(&content, content_len, cursor_idx);
                cursor_idx = editor_index_for_position(&content, content_len, row, 0);
                preferred_col = 0;
            }
            Key::End => {
                let (row, _) = editor_position_for_index(&content, content_len, cursor_idx);
                cursor_idx = line_end_index(&content, content_len, row);
                preferred_col = cursor_visual_col(&content, content_len, cursor_idx);
            }
            Key::PageUp => {
                let (row, _) = editor_position_for_index(&content, content_len, cursor_idx);
                let target_row = row.saturating_sub(EDITOR_VIEW_ROWS);
                cursor_idx =
                    editor_index_for_position(&content, content_len, target_row, preferred_col);
            }
            Key::PageDown => {
                let (row, _) = editor_position_for_index(&content, content_len, cursor_idx);
                let next_idx = editor_index_for_position(
                    &content,
                    content_len,
                    row + EDITOR_VIEW_ROWS,
                    preferred_col,
                );
                if next_idx != cursor_idx {
                    cursor_idx = next_idx;
                }
            }
            Key::Enter => {
                if content_len < MAX_CONTENT_LEN {
                    for i in (cursor_idx..content_len).rev() {
                        content[i + 1] = content[i];
                    }
                    content[cursor_idx] = b'\n';
                    content_len += 1;
                    cursor_idx += 1;
                    preferred_col = 0;
                    saved = false;
                }
            }
            Key::Char(c) => {
                if content_len < MAX_CONTENT_LEN {
                    for i in (cursor_idx..content_len).rev() {
                        content[i + 1] = content[i];
                    }
                    content[cursor_idx] = c;
                    content_len += 1;
                    cursor_idx += 1;
                    preferred_col = cursor_visual_col(&content, content_len, cursor_idx);
                    saved = false;
                }
            }
            _ => {}
        }
    }
}

/// Convert a byte index in the editor buffer into a logical `(row, col)`
/// position in the text area.
///
/// Rows are logical editor rows, not absolute VGA rows. Newlines move to the
/// next row, and long lines wrap at 80 columns to match VGA text-mode width.
fn editor_position_for_index(content: &[u8], content_len: usize, index: usize) -> (usize, usize) {
    let mut row = 0usize;
    let mut col = 0usize;
    let stop = index.min(content_len);

    for byte in content.iter().take(stop) {
        if *byte == b'\n' {
            row += 1;
            col = 0;
        } else {
            col += 1;
            if col == EDITOR_WIDTH {
                row += 1;
                col = 0;
            }
        }
    }

    (row, col)
}

/// Find the closest insertion index for a target `(row, col)` in the editor.
///
/// This is used by the Up/Down arrow keys to preserve the cursor's visual
/// column when moving between wrapped lines.
fn editor_index_for_position(
    content: &[u8],
    content_len: usize,
    target_row: usize,
    target_col: usize,
) -> usize {
    let mut row = 0usize;
    let mut col = 0usize;
    let mut last_on_row = None;

    for idx in 0..=content_len {
        if row == target_row {
            if col >= target_col {
                return idx;
            }
            last_on_row = Some(idx);
        } else if row > target_row {
            return last_on_row.unwrap_or(idx);
        }

        if idx == content_len {
            break;
        }

        if content[idx] == b'\n' {
            row += 1;
            col = 0;
        } else {
            col += 1;
            if col == EDITOR_WIDTH {
                row += 1;
                col = 0;
            }
        }
    }

    last_on_row.unwrap_or(content_len)
}

/// Return the cursor's current visual column.
fn cursor_visual_col(content: &[u8], content_len: usize, cursor_idx: usize) -> usize {
    editor_position_for_index(content, content_len, cursor_idx).1
}

/// Find the insertion index at the end of a logical line.
fn line_end_index(content: &[u8], content_len: usize, target_row: usize) -> usize {
    let mut row = 0usize;
    let mut col = 0usize;

    for idx in 0..=content_len {
        if idx == content_len {
            return idx;
        }

        if row == target_row {
            if content[idx] == b'\n' {
                return idx;
            }
            if col + 1 == EDITOR_WIDTH {
                return idx + 1;
            }
        }

        if content[idx] == b'\n' {
            row += 1;
            col = 0;
        } else {
            col += 1;
            if col == EDITOR_WIDTH {
                row += 1;
                col = 0;
            }
        }
    }

    content_len
}

/// Keep the viewport anchored so the cursor stays within the visible window.
fn clamp_viewport_top_row(
    content: &[u8],
    content_len: usize,
    cursor_idx: usize,
    current_top: usize,
) -> usize {
    let (cursor_row, _) = editor_position_for_index(content, content_len, cursor_idx);
    if cursor_row < current_top {
        return cursor_row;
    }
    if cursor_row >= current_top + EDITOR_VIEW_ROWS {
        return cursor_row + 1 - EDITOR_VIEW_ROWS;
    }
    current_top
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
