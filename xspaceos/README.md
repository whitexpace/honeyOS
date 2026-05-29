# XSpace OS

XSpace OS is a small experimental operating system kernel written in Rust. It
uses a custom `x86_64` target, the `bootloader` crate, and QEMU to boot the
kernel in a virtual machine.

It includes a simple **in-memory file system** and an **interactive shell**:
on boot you get a keyboard-driven desktop, a file manager, and a text editor
for creating, saving, editing, renaming, viewing, and deleting text files.

For file allocation, this version uses **indexed allocation** with:

- `128`-byte blocks
- `1` index block per file
- up to `4` data blocks per file
- a visible allocation-table screen for checking used and unused blocks

## Prerequisites

Install these tools before building or running the project:

- Rust and Cargo through `rustup`
- A nightly Rust toolchain
- Rust source support for `build-std`
- LLVM tools for boot image generation
- `bootimage`
- QEMU

### macOS

```sh
brew install qemu
rustup toolchain install nightly
rustup override set nightly
rustup component add rust-src llvm-tools-preview
cargo install bootimage
```

### Linux

Install QEMU with your distro package manager, then install the Rust tooling:

```sh
sudo apt install qemu-system-x86
rustup toolchain install nightly
rustup override set nightly
rustup component add rust-src llvm-tools-preview
cargo install bootimage
```

If your distro does not use `apt`, install the package that provides
`qemu-system-x86_64`.

## Build

From the repository root:

```sh
cargo build
```

This uses `.cargo/config.toml`, which points Cargo at the custom target file:

```text
x86_64-xspaceos.json
```

## Create a Bootable Image

```sh
cargo bootimage
```

The generated disk image is written to:

```text
target/x86_64-xspaceos/debug/bootimage-xspaceos.bin
```

## Run in QEMU

The easiest way to boot the OS is:

```sh
cargo run
```

Cargo uses the configured runner:

```text
bootimage runner
```

That runner starts QEMU with the generated boot image. When it boots correctly,
you should see the XSpace OS File Manager menu in the QEMU window.

You can also run the image manually:

```sh
qemu-system-x86_64 -drive format=raw,file=target/x86_64-xspaceos/debug/bootimage-xspaceos.bin
```

## Run in VirtualBox

This project does not produce an installer ISO. Instead, it builds a bootable
raw disk image:

```text
target/x86_64-xspaceos/debug/bootimage-xspaceos.bin
```

QEMU can boot that raw image directly. VirtualBox is easier to use if you
convert the raw image into a `vdi` disk first.

### Build a VirtualBox Disk

This repository includes a helper script:

```sh
./scripts/build-virtualbox-disk.sh
```

The script:

1. runs `cargo bootimage`
2. reads `target/x86_64-xspaceos/debug/bootimage-xspaceos.bin`
3. converts it to `xspaceos.vdi` using `qemu-img`

If you prefer the manual conversion step:

```sh
qemu-img convert -f raw -O vdi \
  target/x86_64-xspaceos/debug/bootimage-xspaceos.bin \
  xspaceos.vdi
```

### Boot in VirtualBox

Use these settings when creating the VM:

- Type: `Other` or `Unknown`
- Version: `Other/Unknown (64-bit)` or the closest `x86_64` option
- Hard disk: use `xspaceos.vdi`
- System firmware: disable EFI and use legacy BIOS boot

Then power on the VM. If the firmware cooperates with the bootloader, the VM
should boot into the XSpace OS file manager.

### Compatibility Note

QEMU is still the reference platform for this project. VirtualBox may work, but
it is not the primary environment used by the current build setup. If
VirtualBox fails to boot the image, test the same kernel image in QEMU first to
confirm that the build output is valid.

## Project Layout

```text
.
├── .cargo/config.toml       # Cargo target and runner configuration
├── Cargo.toml               # Rust package metadata and dependencies
├── src/main.rs              # Kernel entry point
├── src/vga_buffer.rs        # VGA text buffer driver (screen output)
├── src/keyboard.rs          # Polled PS/2 keyboard driver (keyboard input)
├── src/fs.rs                # In-memory file system
├── src/shell.rs             # Interactive menu-driven shell
└── x86_64-xspaceos.json     # Custom bare-metal x86_64 target
```

## Using the Shell

When XSpace OS boots it shows a **Desktop** screen, where the `File Manager`
app can be opened. The interface is driven entirely from the keyboard:

| Key          | Action                                            |
| ------------ | ------------------------------------------------- |
| Up / Down    | Move the highlighted selection in menus.          |
| Enter        | Run the highlighted action.                       |
| Esc          | Return from the file manager to the desktop.      |

Inside the file manager and editor, the current workflow supports these tasks:

1. **Create a file** — make a new, empty file.
2. **Open / view a file** — load a file into the editor.
3. **Edit a file** — insert or delete text at the current cursor position.
4. **Save a file** — write the current editor contents into the file system.
5. **Rename a file** — change a file name from inside the editor.
6. **Delete a file** — remove a file from the file manager.
7. **Inspect allocation** — open the `Allocation Table` app from the desktop.

The file manager always shows the current list of files and their sizes.

### Editor controls

The editor is not append-only. It supports a movable insertion cursor:

| Key         | Action                                              |
| ----------- | --------------------------------------------------- |
| Left/Right  | Move the cursor one character backward or forward.  |
| Up/Down     | Move the cursor vertically through wrapped lines.   |
| Backspace   | Delete the character before the cursor.             |
| Delete      | Delete the character at the cursor.                 |
| Enter       | Insert a newline at the cursor.                     |
| Ctrl+S      | Save the file.                                      |
| Ctrl+R      | Rename the file.                                    |
| Ctrl+X      | Close the editor.                                   |

### Keyboard input details

The keyboard layer now supports these text-entry behaviors:

- `Shift` capitalizes letters
- `Caps Lock` toggles alphabetic capitalization
- `Shift` + number row produces symbols such as `!`, `@`, `#`, and `(`
- `Shift` + punctuation produces symbols such as `_`, `+`, `{`, `}`, and `?`

### Allocation table

The desktop now includes an `Allocation Table` screen that displays the
simulated disk blocks. Each row shows:

- the block number
- whether the block is `USED` or `FREE`
- the block type: `IDX` for an index block or `DAT` for a data block
- the owning file name when the block is allocated

Use `Left` and `Right` on that screen to move between pages of block entries.

### How input works

XSpace OS does not set up an interrupt descriptor table, so the keyboard is
read by **polling** the PS/2 controller (`src/keyboard.rs`) rather than via
interrupts. Hardware interrupts are disabled at boot (`cli`) for this reason.

## File System

XSpace OS has no disk driver yet, so the file system (`src/fs.rs`) keeps every
file in kernel RAM. It is **not persistent** — all files are lost on reboot.

The file system is a fixed-size table of fixed-size files, so it needs no heap
and stays fully `no_std`. Because of this it has hard capacity limits:

| Limit              | Value     |
| ------------------ | --------- |
| Maximum files      | 8         |
| Maximum name size  | 32 bytes  |
| Maximum file size  | 512 bytes |
| Block size         | 128 bytes |
| Total blocks       | 40        |

### Operations

The `FileSystem` type exposes the core text-file operations and simulates
indexed block allocation underneath them:

| Operation | Method                       | Description                                  |
| --------- | ---------------------------- | -------------------------------------------- |
| Create    | `create(name)`               | Register a new, empty named file.            |
| Save      | `save(name, data)`           | Replace the full contents of a file.         |
| Edit      | `edit(name, extra)`          | Append more text onto an existing file.      |
| Delete    | `delete(name)`               | Remove a file and free its slot.             |
| Rename    | `rename(old_name, new_name)` | Change a file name without changing content. |

Supporting methods: `read(name)` returns a file's contents, `file_count()`
returns how many files exist, and `for_each_file(...)` lists the directory.

Every operation returns a `Result<_, FsError>`; `FsError` covers a full table,
a missing file, a duplicate name, an invalid name, and oversized content.

The shell (`src/shell.rs`) is the front end for these operations — see the
[Using the Shell](#using-the-shell) section above.

## Troubleshooting

If `cargo build` complains about `build-std`, make sure you are using nightly
Rust and have installed `rust-src`:

```sh
rustup override set nightly
rustup component add rust-src
```

If `cargo bootimage` is not found, install it:

```sh
cargo install bootimage
```

If `cargo run` cannot start QEMU, make sure `qemu-system-x86_64` is installed
and available on your `PATH`:

```sh
qemu-system-x86_64 --version
```
