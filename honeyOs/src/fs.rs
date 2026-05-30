//! In-memory file system for honeyOS.
//!
//! honeyOS has no disk driver yet, so this file system is *not* persistent:
//! every file lives in kernel RAM and is lost on reboot. This module models a
//! small indexed-allocation file system suitable for the course submission:
//!
//!   * each file owns one **index block**
//!   * the index block points at up to four **data blocks**
//!   * each block is 128 bytes
//!   * file contents are reconstructed by reading the referenced data blocks in
//!     index order
//!
//! The whole system stays `no_std` and allocator-free. Every table has fixed
//! capacity, so both file metadata and block storage live in statically sized
//! arrays.

/// Maximum number of files the file system can hold at once.
const MAX_FILES: usize = 8;
/// Maximum length, in bytes, of a file name.
const MAX_NAME_LEN: usize = 32;
/// Number of bytes stored in one allocation block.
pub const BLOCK_SIZE: usize = 128;
/// Maximum number of data blocks that one file may own.
pub const MAX_DATA_BLOCKS_PER_FILE: usize = 4;
/// Maximum length, in bytes, of a file's contents.
pub const MAX_CONTENT_LEN: usize = BLOCK_SIZE * MAX_DATA_BLOCKS_PER_FILE;
/// Total number of blocks in the simulated disk.
pub const TOTAL_BLOCKS: usize = MAX_FILES * (1 + MAX_DATA_BLOCKS_PER_FILE);
/// Sentinel used when a file/block reference slot is empty.
const INVALID_BLOCK: usize = usize::MAX;

/// Errors that a file system operation can return.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsError {
    /// The file table is full; no free slot is available.
    NoSpace,
    /// No file with the requested name exists.
    NotFound,
    /// A file with that name already exists.
    AlreadyExists,
    /// The file name is empty or longer than `MAX_NAME_LEN`.
    BadName,
    /// The contents would exceed `MAX_CONTENT_LEN`.
    ContentTooLong,
}

impl FsError {
    /// A human-readable description, used when reporting results on screen.
    pub fn as_str(&self) -> &'static str {
        match self {
            FsError::NoSpace => "file system is full",
            FsError::NotFound => "file not found",
            FsError::AlreadyExists => "file already exists",
            FsError::BadName => "invalid file name",
            FsError::ContentTooLong => "content exceeds maximum size",
        }
    }
}

/// Block role for the file allocation table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    Index,
    Data,
}

/// Read-only block information exposed to the shell when drawing the file
/// allocation table.
#[derive(Debug, Clone, Copy)]
pub struct BlockInfo<'a> {
    pub used: bool,
    pub kind: Option<BlockKind>,
    pub owner: Option<&'a str>,
}

/// Internal metadata for one simulated disk block.
#[derive(Clone, Copy)]
struct BlockEntry {
    used: bool,
    owner_file: usize,
    kind: BlockKind,
}

impl BlockEntry {
    const FREE: BlockEntry = BlockEntry {
        used: false,
        owner_file: INVALID_BLOCK,
        kind: BlockKind::Data,
    };
}

/// A single file: a name plus the block references used by indexed
/// allocation. Content bytes live in the global block pool, not inside the
/// file metadata itself.
struct File {
    /// Whether this table slot currently holds a live file.
    used: bool,
    name: [u8; MAX_NAME_LEN],
    name_len: usize,
    content_len: usize,
    index_block: usize,
    data_blocks: [usize; MAX_DATA_BLOCKS_PER_FILE],
    data_block_count: usize,
}

impl File {
    /// An empty, unused slot. `const` so a whole `[File; MAX_FILES]` array can
    /// be built in a `const fn` without a heap or runtime initialization.
    const EMPTY: File = File {
        used: false,
        name: [0; MAX_NAME_LEN],
        name_len: 0,
        content_len: 0,
        index_block: INVALID_BLOCK,
        data_blocks: [INVALID_BLOCK; MAX_DATA_BLOCKS_PER_FILE],
        data_block_count: 0,
    };

    /// Does this live file have the given name?
    fn matches(&self, name: &str) -> bool {
        self.used && self.name_len == name.len() && self.name[..self.name_len] == *name.as_bytes()
    }
}

/// The file system: a fixed table of file slots living in kernel memory.
pub struct FileSystem {
    files: [File; MAX_FILES],
    blocks: [[u8; BLOCK_SIZE]; TOTAL_BLOCKS],
    block_entries: [BlockEntry; TOTAL_BLOCKS],
}

impl FileSystem {
    /// Create an empty file system. `const` so it can initialize a `static`.
    pub const fn new() -> FileSystem {
        FileSystem {
            files: [File::EMPTY; MAX_FILES],
            blocks: [[0; BLOCK_SIZE]; TOTAL_BLOCKS],
            block_entries: [BlockEntry::FREE; TOTAL_BLOCKS],
        }
    }

    /// Find the index of the live file with `name`, if any.
    fn find(&self, name: &str) -> Option<usize> {
        self.files.iter().position(|f| f.matches(name))
    }

    /// Count currently free blocks in the simulated disk.
    fn free_block_count(&self) -> usize {
        self.block_entries
            .iter()
            .filter(|entry| !entry.used)
            .count()
    }

    /// Reserve one free block for a file and tag it with its purpose.
    fn allocate_block(&mut self, file_idx: usize, kind: BlockKind) -> Option<usize> {
        let block_idx = self.block_entries.iter().position(|entry| !entry.used)?;
        self.block_entries[block_idx] = BlockEntry {
            used: true,
            owner_file: file_idx,
            kind,
        };
        self.blocks[block_idx] = [0; BLOCK_SIZE];
        Some(block_idx)
    }

    /// Release a block back to the free pool and erase its contents.
    fn free_block(&mut self, block_idx: usize) {
        if block_idx >= TOTAL_BLOCKS {
            return;
        }
        self.blocks[block_idx] = [0; BLOCK_SIZE];
        self.block_entries[block_idx] = BlockEntry::FREE;
    }

    /// Free all data blocks for one file while keeping its index block alive.
    fn free_data_blocks(&mut self, file_idx: usize) {
        let data_blocks = self.files[file_idx].data_blocks;
        let data_count = self.files[file_idx].data_block_count;
        for &block_idx in data_blocks.iter().take(data_count) {
            if block_idx != INVALID_BLOCK {
                self.free_block(block_idx);
            }
        }
        self.files[file_idx].data_blocks = [INVALID_BLOCK; MAX_DATA_BLOCKS_PER_FILE];
        self.files[file_idx].data_block_count = 0;
        self.files[file_idx].content_len = 0;
    }

    /// CREATE: register a new, empty file.
    ///
    /// Fails if the name is invalid, a file with that name already exists, or
    /// the file table is full.
    pub fn create(&mut self, name: &str) -> Result<(), FsError> {
        if name.is_empty() || name.len() > MAX_NAME_LEN {
            return Err(FsError::BadName);
        }
        if self.find(name).is_some() {
            return Err(FsError::AlreadyExists);
        }
        let idx = self
            .files
            .iter()
            .position(|f| !f.used)
            .ok_or(FsError::NoSpace)?;
        let index_block = self
            .allocate_block(idx, BlockKind::Index)
            .ok_or(FsError::NoSpace)?;

        let file = &mut self.files[idx];
        file.used = true;
        file.name[..name.len()].copy_from_slice(name.as_bytes());
        file.name_len = name.len();
        file.content_len = 0;
        file.index_block = index_block;
        file.data_blocks = [INVALID_BLOCK; MAX_DATA_BLOCKS_PER_FILE];
        file.data_block_count = 0;
        Ok(())
    }

    /// SAVE: replace the entire contents of an existing file.
    ///
    /// This is the "save" operation — the file must already exist.
    pub fn save(&mut self, name: &str, data: &str) -> Result<(), FsError> {
        if data.len() > MAX_CONTENT_LEN {
            return Err(FsError::ContentTooLong);
        }
        let idx = self.find(name).ok_or(FsError::NotFound)?;
        let needed_blocks = data.len().div_ceil(BLOCK_SIZE);
        let reusable_blocks = self.files[idx].data_block_count;
        if self.free_block_count() + reusable_blocks < needed_blocks {
            return Err(FsError::NoSpace);
        }

        self.free_data_blocks(idx);

        let bytes = data.as_bytes();
        let mut allocated_blocks = [INVALID_BLOCK; MAX_DATA_BLOCKS_PER_FILE];
        for block_no in 0..needed_blocks {
            let block_idx = self
                .allocate_block(idx, BlockKind::Data)
                .ok_or(FsError::NoSpace)?;
            let start = block_no * BLOCK_SIZE;
            let end = (start + BLOCK_SIZE).min(bytes.len());
            self.blocks[block_idx][..end - start].copy_from_slice(&bytes[start..end]);
            allocated_blocks[block_no] = block_idx;
        }

        let file = &mut self.files[idx];
        file.data_blocks = allocated_blocks;
        file.data_block_count = needed_blocks;
        file.content_len = data.len();
        Ok(())
    }

    /// EDIT: append text to the end of an existing file's contents.
    ///
    /// Fails if the file does not exist or the result would be too large.
    ///
    /// The interactive editor now supports insertion anywhere in the file, so
    /// this append-only helper is retained mainly as a course-level file
    /// operation and for possible scripted tests.
    #[allow(dead_code)]
    pub fn edit(&mut self, name: &str, extra: &str) -> Result<(), FsError> {
        let mut content = [0u8; MAX_CONTENT_LEN];
        let len = self.read_into(name, &mut content)?;
        let new_len = len + extra.len();
        if new_len > MAX_CONTENT_LEN {
            return Err(FsError::ContentTooLong);
        }
        content[len..new_len].copy_from_slice(extra.as_bytes());
        let new_text = core::str::from_utf8(&content[..new_len]).unwrap_or("");
        self.save(name, new_text)
    }

    /// DELETE: remove a file and free its table slot.
    pub fn delete(&mut self, name: &str) -> Result<(), FsError> {
        let idx = self.find(name).ok_or(FsError::NotFound)?;
        let index_block = self.files[idx].index_block;
        self.free_data_blocks(idx);
        if index_block != INVALID_BLOCK {
            self.free_block(index_block);
        }
        self.files[idx] = File::EMPTY;
        Ok(())
    }

    /// Read back the contents of a file into a caller-provided buffer.
    ///
    /// This avoids heap allocation while still letting higher layers rebuild a
    /// contiguous byte view of block-based file contents.
    pub fn read_into(&self, name: &str, out: &mut [u8; MAX_CONTENT_LEN]) -> Result<usize, FsError> {
        let idx = self.find(name).ok_or(FsError::NotFound)?;
        let file = &self.files[idx];
        let mut copied = 0usize;
        for &block_idx in file.data_blocks.iter().take(file.data_block_count) {
            let remaining = file.content_len - copied;
            let chunk_len = remaining.min(BLOCK_SIZE);
            out[copied..copied + chunk_len].copy_from_slice(&self.blocks[block_idx][..chunk_len]);
            copied += chunk_len;
        }
        Ok(file.content_len)
    }

    /// Number of files currently stored.
    pub fn file_count(&self) -> usize {
        self.files.iter().filter(|f| f.used).count()
    }

    /// RENAME: change a file's name without altering its contents.
    pub fn rename(&mut self, old_name: &str, new_name: &str) -> Result<(), FsError> {
        if new_name.is_empty() || new_name.len() > MAX_NAME_LEN {
            return Err(FsError::BadName);
        }
        if self.find(new_name).is_some() {
            return Err(FsError::AlreadyExists);
        }
        let idx = self.find(old_name).ok_or(FsError::NotFound)?;
        let file = &mut self.files[idx];
        file.name[..new_name.len()].copy_from_slice(new_name.as_bytes());
        // Zero out leftover bytes only when the old name was longer.
        if new_name.len() < file.name_len {
            for b in file.name[new_name.len()..file.name_len].iter_mut() {
                *b = 0;
            }
        }
        file.name_len = new_name.len();
        Ok(())
    }

    /// Visit every live file, passing its name and content length to `visit`.
    /// Used to list the directory without needing a heap-allocated collection.
    pub fn for_each_file(&self, mut visit: impl FnMut(&str, usize)) {
        for f in self.files.iter() {
            if f.used {
                let name = core::str::from_utf8(&f.name[..f.name_len]).unwrap_or("<?>");
                visit(name, f.content_len);
            }
        }
    }

    /// Return one row of file-allocation-table data.
    pub fn block_info(&self, block_idx: usize) -> BlockInfo<'_> {
        if block_idx >= TOTAL_BLOCKS {
            return BlockInfo {
                used: false,
                kind: None,
                owner: None,
            };
        }

        let entry = self.block_entries[block_idx];
        if !entry.used {
            return BlockInfo {
                used: false,
                kind: None,
                owner: None,
            };
        }

        let file = &self.files[entry.owner_file];
        let owner = core::str::from_utf8(&file.name[..file.name_len]).unwrap_or("<?>");
        BlockInfo {
            used: true,
            kind: Some(entry.kind),
            owner: Some(owner),
        }
    }
}
