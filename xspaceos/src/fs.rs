//! In-memory file system for XSpace OS.
//!
//! XSpace OS has no disk driver yet, so this file system is *not* persistent:
//! every file lives in kernel RAM and is lost on reboot. It is a teaching
//! model of the four core file operations required by CMSC125:
//!
//!   * create — register a new, empty named file
//!   * save   — write (replace) the full contents of a file
//!   * edit   — append more text onto an existing file
//!   * delete — remove a file and free its slot
//!
//! To stay `no_std` and allocator-free, the file system is a fixed-size table
//! of fixed-size files. All storage is statically sized — there is no heap.
//! That keeps the implementation simple and the memory layout predictable,
//! at the cost of hard capacity limits (see the constants below).

/// Maximum number of files the file system can hold at once.
const MAX_FILES: usize = 8;
/// Maximum length, in bytes, of a file name.
const MAX_NAME_LEN: usize = 32;
/// Maximum length, in bytes, of a file's contents.
const MAX_CONTENT_LEN: usize = 512;

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

/// A single file: a name and a body, each stored in a fixed-size buffer with
/// an accompanying length so we know how much of the buffer is in use.
struct File {
    /// Whether this table slot currently holds a live file.
    used: bool,
    name: [u8; MAX_NAME_LEN],
    name_len: usize,
    content: [u8; MAX_CONTENT_LEN],
    content_len: usize,
}

impl File {
    /// An empty, unused slot. `const` so a whole `[File; MAX_FILES]` array can
    /// be built in a `const fn` without a heap or runtime initialization.
    const EMPTY: File = File {
        used: false,
        name: [0; MAX_NAME_LEN],
        name_len: 0,
        content: [0; MAX_CONTENT_LEN],
        content_len: 0,
    };

    /// Does this live file have the given name?
    fn matches(&self, name: &str) -> bool {
        self.used
            && self.name_len == name.len()
            && self.name[..self.name_len] == *name.as_bytes()
    }
}

/// The file system: a fixed table of file slots living in kernel memory.
pub struct FileSystem {
    files: [File; MAX_FILES],
}

impl FileSystem {
    /// Create an empty file system. `const` so it can initialize a `static`.
    pub const fn new() -> FileSystem {
        FileSystem {
            files: [File::EMPTY; MAX_FILES],
        }
    }

    /// Find the index of the live file with `name`, if any.
    fn find(&self, name: &str) -> Option<usize> {
        self.files.iter().position(|f| f.matches(name))
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

        let file = &mut self.files[idx];
        file.used = true;
        file.name[..name.len()].copy_from_slice(name.as_bytes());
        file.name_len = name.len();
        file.content_len = 0;
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
        let file = &mut self.files[idx];
        file.content[..data.len()].copy_from_slice(data.as_bytes());
        file.content_len = data.len();
        Ok(())
    }

    /// EDIT: append text to the end of an existing file's contents.
    ///
    /// Fails if the file does not exist or the result would be too large.
    pub fn edit(&mut self, name: &str, extra: &str) -> Result<(), FsError> {
        let idx = self.find(name).ok_or(FsError::NotFound)?;
        let file = &mut self.files[idx];
        let new_len = file.content_len + extra.len();
        if new_len > MAX_CONTENT_LEN {
            return Err(FsError::ContentTooLong);
        }
        file.content[file.content_len..new_len].copy_from_slice(extra.as_bytes());
        file.content_len = new_len;
        Ok(())
    }

    /// DELETE: remove a file and free its table slot.
    pub fn delete(&mut self, name: &str) -> Result<(), FsError> {
        let idx = self.find(name).ok_or(FsError::NotFound)?;
        self.files[idx] = File::EMPTY;
        Ok(())
    }

    /// Read back the contents of a file as a string slice.
    pub fn read(&self, name: &str) -> Result<&str, FsError> {
        let idx = self.find(name).ok_or(FsError::NotFound)?;
        let file = &self.files[idx];
        // Contents only ever come from `&str` writes, so this is valid UTF-8.
        Ok(core::str::from_utf8(&file.content[..file.content_len]).unwrap_or("<non-utf8>"))
    }

    /// Number of files currently stored.
    pub fn file_count(&self) -> usize {
        self.files.iter().filter(|f| f.used).count()
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
}
