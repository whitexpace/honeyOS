//! honeyOS — kernel entry point.
//!
//! This is a bare-metal Rust kernel (`no_std`, `no_main`): there is no
//! operating system underneath it and no C runtime. Execution begins at
//! `_start`, which the bootloader jumps to after setting up the machine.
//!
//! Modules:
//!   * `vga_buffer` — VGA text-mode driver (screen output).
//!   * `keyboard`   — polled PS/2 keyboard driver (keyboard input).
//!   * `fs`         — in-memory file system.
//!   * `shell`      — interactive menu the user navigates to manage files.
//!
//! On boot the kernel hands control to the shell, which runs forever.

#![no_std]
#![no_main]

mod fs;
mod keyboard;
mod shell;
mod vga_buffer;

use core::panic::PanicInfo;

use fs::FileSystem;
use vga_buffer::Color;

/// The kernel's file system instance.
///
/// It lives in static (`.bss`) memory rather than on the stack because a file
/// system is long-lived kernel state and the table is too large to want on
/// the boot stack. `FileSystem::new()` is a `const fn`, so no runtime
/// initialization is needed.
static mut FILE_SYSTEM: FileSystem = FileSystem::new();

/// Called by the Rust runtime whenever the kernel panics.
///
/// There is nothing to unwind to, so we report the panic and halt forever.
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    vga_buffer::set_text_color(Color::White, Color::Red);
    println!("KERNEL PANIC: {}", info);
    loop {}
}

/// Kernel entry point. The bootloader transfers control here.
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    // Disable hardware interrupts. honeyOS has no interrupt descriptor table,
    // so any fired interrupt would fault the CPU. The keyboard is polled
    // instead (see `keyboard.rs`), so interrupts are not needed.
    //
    // SAFETY: `cli` only clears the interrupt flag; it has no memory effect.
    unsafe {
        core::arch::asm!("cli", options(nomem, nostack));
    }

    vga_buffer::clear_screen();

    // SAFETY: the kernel is single-threaded with interrupts disabled, so this
    // is the only `&mut` reference to `FILE_SYSTEM` that will ever exist.
    let fs = unsafe { &mut *(&raw mut FILE_SYSTEM) };

    // Hand control to the interactive shell — this never returns.
    shell::run(fs)
}
