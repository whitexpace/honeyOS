# Honey OS

Honey OS is a small experimental operating system kernel written in Rust. It
currently boots with the `bootloader` crate and writes `Honey OS` directly to
the VGA text buffer.

## Requirements

Install these once before building the project:

- Rust with `rustup`
- Nightly Rust toolchain
- `rust-src` and `llvm-tools-preview` Rust components
- `bootimage`
- QEMU, if you want to run the OS locally

On macOS, QEMU can be installed with Homebrew:

```sh
brew install qemu
```

On Ubuntu or Debian:

```sh
sudo apt install qemu-system-x86
```

## Setup

From the project directory, install the Rust toolchain requirements:

```sh
rustup toolchain install nightly
rustup override set nightly
rustup component add rust-src llvm-tools-preview
cargo install bootimage
```

This project uses `.cargo/config.toml` to build with the custom
`x86_64-Honeyos.json` target by default.

## Build

```sh
cargo build
```

Cargo will download the Rust crate dependencies listed in `Cargo.toml`, similar
to how `npm install` downloads dependencies for a web app.

## Run

```sh
cargo run
```

The configured runner uses `bootimage runner`, which launches the kernel in
QEMU. If this command fails, make sure both `bootimage` and QEMU are installed.

## Project Structure

```text
.
├── .cargo/config.toml        # Cargo build settings for the custom target
├── Cargo.toml                # Rust package manifest and dependencies
├── src/main.rs               # Kernel entry point
└── x86_64-Honeyos.json      # Custom bare-metal x86_64 target
```

## Notes

- The kernel is `#![no_std]` and `#![no_main]`, so it does not use Rust's
  standard library or normal `main` function.
- Panics currently enter an infinite loop.
- The VGA text buffer starts at memory address `0xb8000`, which is where the
  current boot message is written.
