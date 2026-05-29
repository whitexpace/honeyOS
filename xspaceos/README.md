# XSpace OS

XSpace OS is a small experimental operating system kernel written in Rust. It
uses a custom `x86_64` target, the `bootloader` crate, and QEMU to boot the
kernel in a virtual machine.

It includes a simple **in-memory file system** and an **interactive shell**:
on boot you get a keyboard-driven menu for creating, saving, editing, viewing,
and deleting text files.

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

When XSpace OS boots it shows the **File Manager** menu. It is driven entirely
from the keyboard:

| Key          | Action                                            |
| ------------ | ------------------------------------------------- |
| Up / Down    | Move the highlighted selection on the main menu.  |
| Enter        | Run the highlighted action.                       |
| Any key      | Return to the menu from a result screen.          |

The menu offers five actions. Each one opens a prompt screen where you type a
file name (and contents, where relevant) and press Enter:

1. **Create a file** — make a new, empty file.
2. **Write / save a file** — replace a file's contents with what you type.
3. **Edit a file** — append more text to the end of a file.
4. **View a file** — print a file's contents to the screen.
5. **Delete a file** — remove a file.

The main menu always shows the current list of files and their sizes.

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

### Operations

The `FileSystem` type exposes the four required text-file operations:

| Operation | Method                       | Description                                  |
| --------- | ---------------------------- | -------------------------------------------- |
| Create    | `create(name)`               | Register a new, empty named file.            |
| Save      | `save(name, data)`           | Replace the full contents of a file.         |
| Edit      | `edit(name, extra)`          | Append more text onto an existing file.      |
| Delete    | `delete(name)`               | Remove a file and free its slot.             |

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
