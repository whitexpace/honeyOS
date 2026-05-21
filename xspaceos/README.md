# XSpace OS

XSpace OS is a small experimental operating system kernel written in Rust. It
uses a custom `x86_64` target, the `bootloader` crate, and QEMU to boot the
kernel in a virtual machine.

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
you should see the kernel's VGA output in the QEMU window.

You can also run the image manually:

```sh
qemu-system-x86_64 -drive format=raw,file=target/x86_64-xspaceos/debug/bootimage-xspaceos.bin
```

## Project Layout

```text
.
├── .cargo/config.toml       # Cargo target and runner configuration
├── Cargo.toml               # Rust package metadata and dependencies
├── src/main.rs              # Kernel entry point
├── src/vga_buffer.rs        # VGA text buffer output code
└── x86_64-xspaceos.json     # Custom bare-metal x86_64 target
```

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
