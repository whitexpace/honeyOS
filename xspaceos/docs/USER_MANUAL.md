# Honey OS User Manual

## Documentation for Installation, Virtual Machine Setup, and Current System Features

Project implementation name: `XSpace OS`

Version date: May 29, 2026

Prepared for: next batch of implementors and end users

---

## Title Page

**Document Title:** Honey OS User Manual

**Repository Implementation:** `XSpace OS`

**Purpose:** This manual explains how to build the project, run it inside a
virtual machine, and use the currently implemented features.

**Recommended PDF filename:** `Honey-OS-User-Manual.pdf`

---

## Table of Contents

1. Overview
2. Current Features
3. Software Requirements
4. Project Location
5. One-Time Rust Setup
6. Build the VirtualBox Disk
7. Create the VirtualBox Machine
8. Attach the Generated VDI
9. Boot the OS
10. How to Use the OS
11. File Allocation Design
12. File System Limits
13. Known Limitations
14. Troubleshooting
15. Suggested Screenshots for the PDF
16. Recommended Sample Demo Flow
17. How to Turn This Manual into a PDF
18. Summary

---

## 1. Overview

Honey OS, implemented in this repository as `XSpace OS`, is a small experimental
operating system kernel written in Rust. It boots inside a virtual machine and
provides a simple text-based desktop, a file manager, and a text editor.

This manual explains:

- how to prepare the required tools
- how to create and run the virtual machine
- how to use the current OS features
- what limitations still exist in the current implementation

## 2. Current Features

The current OS build includes these user-visible features:

- a desktop screen with a selectable `File Manager` app
- an `Allocation Table` app for viewing disk blocks
- a file manager that lists files and their sizes
- file creation
- file opening
- file editing
- file saving
- file renaming
- file deletion
- movable cursor editing
- Caps Lock support for letters
- Shift support for symbols on number and punctuation keys
- keyboard-only navigation
- an in-memory file system
- indexed file allocation using fixed-size blocks

Important limitation:

- files are not persistent yet; all files are lost when the VM is shut down or rebooted

## 3. Software Requirements

Install these on the host machine before building or running the OS:

- Rust and Cargo through `rustup`
- Rust nightly toolchain
- `rust-src`
- `llvm-tools-preview`
- `bootimage`
- `qemu-img`
- VirtualBox

Recommended verification commands:

```sh
rustup --version
cargo --version
qemu-img --version
VBoxManage --version
```

## 4. Project Location

All commands below assume the project root is:

```text
/home/nino/personal/uni/125proj2-OS/xspaceos
```

Move into the project directory first:

```sh
cd /home/nino/personal/uni/125proj2-OS/xspaceos
```

## 5. One-Time Rust Setup

Run the following commands once:

```sh
rustup toolchain install nightly
rustup override set nightly
rustup component add rust-src llvm-tools-preview
cargo install bootimage
```

This project uses a custom target file and `build-std`, so nightly Rust is
required.

## 6. Build the VirtualBox Disk

Use the helper script included in this repository:

```sh
./scripts/build-virtualbox-disk.sh
```

What this script does:

1. builds the kernel boot image with `cargo bootimage`
2. creates a raw bootable image at `target/x86_64-xspaceos/debug/bootimage-xspaceos.bin`
3. converts that raw image into `xspaceos.vdi`

Expected output file:

```text
xspaceos.vdi
```

## 7. Create the VirtualBox Machine

Use the following steps in VirtualBox:

1. Open VirtualBox.
2. Click `New`.
3. Enter a VM name such as `Honey OS` or `XSpace OS`.
4. Set the machine type to `Other`.
5. Set the version to `Other/Unknown (64-bit)`.
6. Leave any ISO field empty if VirtualBox shows one.
7. Keep `Use EFI` unchecked.
8. Set memory to `128 MB` or `256 MB`.
9. Set CPU count to `1`.
10. Finish the wizard, even if VirtualBox creates a temporary blank disk.

Recommended VM settings after creation:

- Memory: `128 MB` minimum
- CPUs: `1`
- EFI: disabled
- Boot mode: legacy BIOS

## 8. Attach the Generated VDI

After the VM is created:

1. Select the VM in VirtualBox.
2. Open `Settings`.
3. Open `Storage`.
4. Remove the empty disk VirtualBox created, if one exists.
5. Add or select a SATA controller.
6. Click the hard disk icon.
7. Choose `Choose a Disk File...`
8. Select `xspaceos.vdi` from the project directory.
9. Make sure this VDI is the primary hard disk.
10. Save the settings.

## 9. Boot the OS

Start the VM.

If boot succeeds, the OS will show:

- a title banner for `XSpace OS`
- a desktop screen
- desktop apps named `File Manager` and `Allocation Table`

If the VM does not boot:

- confirm EFI is disabled
- confirm the attached disk is `xspaceos.vdi`
- confirm the VDI was generated from the latest build
- test the raw image in QEMU to check whether the build output is valid

## 10. How to Use the OS

### 10.1 Desktop Screen

The first visible screen is the desktop.

Controls:

- `Up` / `Down`: move selection
- `Enter`: open the selected app

Current app available:

- `File Manager`
- `Allocation Table`

### 10.2 File Manager

The file manager shows:

- a `New File` action
- the list of current files
- each file's size in bytes

Controls:

- `Up` / `Down`: move selection
- `Enter`: open the selected file or create a new file
- `Delete`: delete the selected file
- `Esc`: return to the desktop

### 10.3 Allocation Table

The desktop also includes an `Allocation Table` screen used for checking the
indexed allocation state of the simulated disk.

Each block entry shows:

- block number
- whether the block is `USED` or `FREE`
- whether the block is an `IDX` block or a `DAT` block
- the owner file name for allocated blocks

Controls:

- `Left` / `Right`: move between allocation-table pages
- `Esc`: return to the desktop

### 10.4 Creating a File

To create a file:

1. Open `File Manager`.
2. Highlight `[ + New File ]`.
3. Press `Enter`.
4. Type a file name.
5. Press `Enter`.

After creation, the editor opens automatically for the new file.

### 10.5 Editor

The editor is a full-screen text editor.

It shows:

- the current file name at the top
- keyboard shortcuts in the header
- the text content area
- a status bar showing whether the file is saved
- the current content size out of `512` bytes

Editor controls:

- type letters and numbers to add text
- `Left` / `Right`: move the cursor horizontally
- `Up` / `Down`: move the cursor vertically through wrapped lines
- `Enter`: insert a new line
- `Backspace`: delete the previous character
- `Delete`: delete the character at the cursor
- `Ctrl+S`: save the file
- `Ctrl+X`: close the editor
- `Ctrl+R`: rename the file

Keyboard input behavior:

- `Shift` capitalizes letters
- `Caps Lock` also capitalizes letters
- `Shift` with the number row types symbols such as `!`, `@`, and `#`
- `Shift` with punctuation types symbols such as `_`, `+`, `{`, `}`, and `?`

### 10.6 Viewing a File

There is no separate viewer screen in the current implementation.

To view a file:

1. select the file in `File Manager`
2. press `Enter`
3. read the contents in the editor

### 10.7 Editing a File

To edit an existing file:

1. open `File Manager`
2. select the file
3. press `Enter`
4. type additional text or remove text with `Backspace`
5. move the cursor with the arrow keys if you want to edit the middle of the file
6. press `Ctrl+S` to save changes

### 10.8 Renaming a File

To rename a file:

1. open the file in the editor
2. press `Ctrl+R`
3. type the new file name
4. press `Enter`

### 10.9 Deleting a File

To delete a file:

1. go to `File Manager`
2. highlight the target file
3. press `Delete`

The file is removed immediately from the in-memory file list.

## 11. File Allocation Design

This project now uses **indexed allocation** for file storage.

The chosen configuration is:

- block size: `128 bytes`
- index blocks per file: `1`
- maximum data blocks per file: `4`
- maximum file size: `512 bytes`

How it works:

1. when a file is created, the file system reserves one index block
2. when a file is saved, the contents are split into `128-byte` chunks
3. each chunk is stored in a data block
4. the file metadata stores references to the file's data blocks in order
5. when a file is read, the file system reconstructs the contents by reading those blocks in sequence

This design was chosen because it is simple to explain, easy to visualize in a
text-mode OS, and matches the course requirement to focus on file allocation.

## 12. File System Limits

The current file system is intentionally simple and fixed-size.

Limits:

- Maximum files: `8`
- Maximum file name length: `32` bytes
- Maximum file content length: `512` bytes
- Allocation method: indexed allocation
- Block size: `128 bytes`
- Maximum data blocks per file: `4`
- Total simulated disk blocks: `40`

Because the file system is in RAM only:

- files disappear after shutdown
- files disappear after reset
- there is no disk persistence yet

## 13. Known Limitations

At the current stage, Honey OS / XSpace OS has these limitations:

- no persistent storage
- no mouse support
- no multitasking
- no multiple productivity apps beyond `File Manager` and `Allocation Table`
- no installer ISO
- no guaranteed VirtualBox compatibility on every host
- text mode only; no graphical window system
- no persistent file storage across reboots

These are expected for the current course-project stage.

## 14. Troubleshooting

### 14.1 `cargo bootimage` not found

Install the command:

```sh
cargo install bootimage
```

### 14.2 `qemu-img` not found

Install QEMU tools on the host system, then verify:

```sh
qemu-img --version
```

### 14.3 VirtualBox asks for an ISO

Do not use an ISO for this project.

Instead:

1. create the VM without installation media
2. finish the VM wizard
3. open `Settings > Storage`
4. attach `xspaceos.vdi` as the hard disk

### 14.4 VM does not boot

Check all of the following:

- EFI is disabled
- the VDI file is attached as the main hard disk
- the VDI was built from the latest kernel image
- the kernel image can boot in QEMU

## 15. Suggested Screenshots for the PDF

This section tells the document editor exactly what images to capture and what
each image should show. If direct screenshots are not available, these can be
recreated manually using the descriptions below.

### Figure 1. VirtualBox New VM Screen

![Figure 1. VirtualBox New VM Screen](images/figure-1-virtualbox-new-vm.png)

Purpose:

- show the VM name, machine type, and memory setup

Capture steps:

1. Open VirtualBox.
2. Click `New`.
3. Fill in the VM name.
4. Set `Type` to `Other`.
5. Set `Version` to `Other/Unknown (64-bit)`.
6. Keep `Use EFI` unchecked.
7. Set memory to `128 MB` or `256 MB`.
8. Take the screenshot before finishing the wizard.

What the image should look like:

- VirtualBox new machine wizard
- memory slider visible
- EFI checkbox visible and unchecked
- guest type set to an `Other` option

Suggested caption:

`Figure 1. Creating the Honey OS VirtualBox machine with legacy BIOS settings.`

### Figure 2. VirtualBox Storage Settings

![Figure 2. VirtualBox Storage Settings](images/figure-2-virtualbox-storage.png)

Purpose:

- show how the `xspaceos.vdi` file is attached

Capture steps:

1. Open the VM in VirtualBox.
2. Go to `Settings > Storage`.
3. Remove any blank disk if one exists.
4. Attach `xspaceos.vdi` to the SATA controller.
5. Take the screenshot once the disk is attached.

What the image should look like:

- Storage tree visible
- a SATA controller visible
- `xspaceos.vdi` shown as the attached hard disk

Suggested caption:

`Figure 2. Attaching the generated VDI as the VM hard disk.`

### Figure 3. Desktop Screen

![Figure 3. Desktop Screen](images/figure-3-desktop-screen.png)

Purpose:

- show the first OS screen after boot

Capture steps:

1. Start the VM.
2. Wait for the desktop to appear.
3. Leave `File Manager` highlighted.
4. Take the screenshot.

What the image should look like:

- black text-mode background
- `XSpace OS` title near the top
- `Desktop` label
- `File Manager` selected
- keyboard hint line near the bottom

Suggested caption:

`Figure 3. The Honey OS desktop after a successful boot.`

### Figure 4. File Manager Screen

![Figure 4. File Manager Screen](images/figure-4-file-manager.png)

Purpose:

- show the file browser and current files

Capture steps:

1. From the desktop, press `Enter` on `File Manager`.
2. Ensure `[ + New File ]` is visible.
3. If possible, create one sample file first so the file list is not empty.
4. Take the screenshot.

What the image should look like:

- title line `XSpace OS -- File Manager`
- `[ + New File ]` at the top
- at least one file entry with byte size, if available
- shortcut hint line at the bottom with `Enter`, `Del`, and `Esc`

Suggested caption:

`Figure 4. File Manager screen showing files and available actions.`

### Figure 5. New File Prompt

![Figure 5. New File Prompt](images/figure-5-new-file-prompt.png)

Purpose:

- show the file creation prompt

Capture steps:

1. In `File Manager`, select `[ + New File ]`.
2. Press `Enter`.
3. Stop at the `File name:` prompt.
4. Take the screenshot before typing or after typing a sample name.

What the image should look like:

- title `=== New File ===`
- prompt line `File name:`

Suggested caption:

`Figure 5. Prompt for entering the name of a new file.`

### Figure 6. Editor Screen

![Figure 6. Editor Screen](images/figure-6-editor-screen.png)

Purpose:

- show the text editor and its shortcuts

Capture steps:

1. Create or open a file.
2. Type 2 to 4 short lines of sample text.
3. Save the file with `Ctrl+S`.
4. Take the screenshot while the editor is open.

What the image should look like:

- filename in the header
- header shortcuts `Ctrl+S`, `Ctrl+X`, and `Ctrl+R`
- text content area with sample text
- visible cursor highlight somewhere inside the text, not only at the end
- status bar showing `Saved`

Suggested caption:

`Figure 6. Editor screen used for writing and saving file contents.`

### Figure 7. Rename Prompt

![Figure 7. Rename Prompt](images/figure-7-rename-prompt.png)

Purpose:

- show file renaming support

Capture steps:

1. Open a file in the editor.
2. Press `Ctrl+R`.
3. Stop at the rename prompt.
4. Take the screenshot.

What the image should look like:

- title `=== Rename File ===`
- line showing `Current name:`
- prompt line `New name:`

Suggested caption:

`Figure 7. Renaming an existing file from the editor.`

### Figure 8. Allocation Table

![Figure 8. Allocation Table](images/figure-8-allocation-table.png)

Purpose:

- show the indexed-allocation table with both used and free blocks

Capture steps:

1. Boot the OS.
2. Create at least one sample file and save some text into it.
3. Return to the desktop.
4. Open `Allocation Table`.
5. Take the screenshot when both `IDX` and `DAT` entries are visible.

What the image should look like:

- title `XSpace OS -- Allocation Table`
- multiple block rows such as `B00`, `B01`, and so on
- `USED` and `FREE` entries both visible
- `IDX` and `DAT` labels visible
- at least one file name shown as a block owner

Suggested caption:

`Figure 8. Allocation table showing used, free, index, and data blocks.`

## 16. Recommended Sample Demo Flow

For the final PDF, use one consistent demo scenario:

1. Boot the VM.
2. Open `File Manager`.
3. Create a file named `notes.txt`.
4. Type sample text such as:

```text
Honey OS demo file
Created inside VirtualBox
File system is currently in-memory only
```

5. Save with `Ctrl+S`.
6. Rename the file to `manual.txt`.
7. Return to the file manager.
8. Show the updated file list.
9. Open `Allocation Table`.
10. Show the index and data blocks used by `manual.txt`.

This produces a cleaner sequence of screenshots for the document.

## 17. How to Turn This Manual into a PDF

Recommended workflow:

1. Open this file in a Markdown editor such as VS Code, Typora, or Obsidian.
2. Insert the screenshots under the matching figure headings.
3. Export or print the document as PDF.

If using VS Code:

1. Open `USER_MANUAL.md`.
2. Open Markdown preview.
3. Use `Print` from the preview window or use a Markdown-to-PDF extension.

Suggested output filename:

```text
Honey-OS-User-Manual.pdf
```

### VS Code Export Steps

1. Open [USER_MANUAL.md](/home/nino/personal/uni/125proj2-OS/xspaceos/docs/USER_MANUAL.md:1) in VS Code.
2. Make sure the screenshots are placed in [docs/images/README.md](/home/nino/personal/uni/125proj2-OS/xspaceos/docs/images/README.md:1) using the required filenames.
3. Press `Ctrl+Shift+V` to open Markdown Preview.
4. If preview printing is available, print the preview and choose `Save as PDF`.
5. If preview printing is not available, install the VS Code extension `Markdown PDF`.
6. After installing it, open the command palette with `Ctrl+Shift+P`.
7. Run `Markdown PDF: Export (pdf)`.
8. Save the output as `Honey-OS-User-Manual.pdf`.

### Typora Export Steps

1. Open `USER_MANUAL.md` in Typora.
2. Confirm that all image links display correctly.
3. Click `File`.
4. Click `Export`.
5. Choose `PDF`.
6. Save the output as `Honey-OS-User-Manual.pdf`.

## 18. Summary

The current Honey OS / XSpace OS implementation already demonstrates the main
course features expected from this stage:

- virtual machine boot
- file manager navigation
- file creation
- file editing
- file saving
- file renaming
- file deletion
- indexed allocation
- visible allocation-table inspection
- a simple bounded file system

The biggest remaining limitation is persistence. At present, the OS is useful
as a demonstrator of the file-management workflow, but it does not yet store
files permanently across reboots.
