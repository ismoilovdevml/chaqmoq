//! Filesystem ops backing the `Chaqmoq.*File*` / directory APIs.
//!
//! Async variants run on Tokio's blocking pool; `*Sync` variants block the
//! isolate on purpose, mirroring how Node and Deno expose the same trade-off.

use std::cell::RefCell;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use deno_core::op2;
use deno_core::JsBuffer;
use deno_core::OpState;
use deno_error::JsErrorBox;
use serde::Serialize;

use super::io_error;
use crate::permissions::Permissions;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileInfo {
    pub is_file: bool,
    pub is_directory: bool,
    pub is_symlink: bool,
    pub size: u64,
    /// Milliseconds since the Unix epoch, or `null` when the platform does not
    /// report the field.
    pub mtime: Option<f64>,
    pub atime: Option<f64>,
    pub birthtime: Option<f64>,
    pub mode: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirEntry {
    pub name: String,
    pub is_file: bool,
    pub is_directory: bool,
    pub is_symlink: bool,
}

fn to_ms(time: std::io::Result<SystemTime>) -> Option<f64> {
    time.ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as f64)
}

fn file_info(meta: std::fs::Metadata) -> FileInfo {
    #[cfg(unix)]
    let mode = {
        use std::os::unix::fs::MetadataExt;
        meta.mode()
    };
    #[cfg(not(unix))]
    let mode = 0u32;

    FileInfo {
        is_file: meta.is_file(),
        is_directory: meta.is_dir(),
        is_symlink: meta.file_type().is_symlink(),
        size: meta.len(),
        mtime: to_ms(meta.modified()),
        atime: to_ms(meta.accessed()),
        birthtime: to_ms(meta.created()),
        mode,
    }
}

fn dir_entries(read_dir: std::fs::ReadDir, path: &Path) -> Result<Vec<DirEntry>, JsErrorBox> {
    let mut out = Vec::new();
    for entry in read_dir {
        let entry = entry.map_err(|e| io_error(e, &path.display().to_string()))?;
        let file_type = entry
            .file_type()
            .map_err(|e| io_error(e, &path.display().to_string()))?;
        out.push(DirEntry {
            name: entry.file_name().to_string_lossy().into_owned(),
            is_file: file_type.is_file(),
            is_directory: file_type.is_dir(),
            is_symlink: file_type.is_symlink(),
        });
    }
    // Stable ordering makes script output deterministic across platforms.
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// Borrows permissions out of the shared op state without holding the borrow
/// across an `await` point.
fn check_read(state: &Rc<RefCell<OpState>>, path: &Path) -> Result<(), JsErrorBox> {
    state.borrow().borrow::<Permissions>().check_read(path)
}

fn check_write(state: &Rc<RefCell<OpState>>, path: &Path) -> Result<(), JsErrorBox> {
    state.borrow().borrow::<Permissions>().check_write(path)
}

// ---------------------------------------------------------------------------
// Reading
// ---------------------------------------------------------------------------

#[op2]
#[buffer]
pub async fn op_chq_read_file(
    state: Rc<RefCell<OpState>>,
    #[string] path: String,
) -> Result<Vec<u8>, JsErrorBox> {
    let path = PathBuf::from(&path);
    check_read(&state, &path)?;
    tokio::fs::read(&path)
        .await
        .map_err(|e| io_error(e, &path.display().to_string()))
}

#[op2]
#[buffer]
pub fn op_chq_read_file_sync(
    state: &mut OpState,
    #[string] path: String,
) -> Result<Vec<u8>, JsErrorBox> {
    let path = PathBuf::from(&path);
    state.borrow::<Permissions>().check_read(&path)?;
    std::fs::read(&path).map_err(|e| io_error(e, &path.display().to_string()))
}

#[op2]
#[string]
pub async fn op_chq_read_text_file(
    state: Rc<RefCell<OpState>>,
    #[string] path: String,
) -> Result<String, JsErrorBox> {
    let path = PathBuf::from(&path);
    check_read(&state, &path)?;
    tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| io_error(e, &path.display().to_string()))
}

#[op2]
#[string]
pub fn op_chq_read_text_file_sync(
    state: &mut OpState,
    #[string] path: String,
) -> Result<String, JsErrorBox> {
    let path = PathBuf::from(&path);
    state.borrow::<Permissions>().check_read(&path)?;
    std::fs::read_to_string(&path).map_err(|e| io_error(e, &path.display().to_string()))
}

// ---------------------------------------------------------------------------
// Writing
// ---------------------------------------------------------------------------

#[op2]
pub async fn op_chq_write_file(
    state: Rc<RefCell<OpState>>,
    #[string] path: String,
    #[buffer] data: JsBuffer,
    append: bool,
) -> Result<(), JsErrorBox> {
    let path = PathBuf::from(&path);
    check_write(&state, &path)?;
    let bytes = data.to_vec();
    let path_for_error = path.clone();
    tokio::task::spawn_blocking(move || write_bytes(&path, &bytes, append))
        .await
        .map_err(|e| JsErrorBox::generic(e.to_string()))?
        .map_err(|e| io_error(e, &path_for_error.display().to_string()))
}

#[op2(fast)]
pub fn op_chq_write_file_sync(
    state: &mut OpState,
    #[string] path: String,
    #[buffer] data: &[u8],
    append: bool,
) -> Result<(), JsErrorBox> {
    let path = PathBuf::from(&path);
    state.borrow::<Permissions>().check_write(&path)?;
    write_bytes(&path, data, append).map_err(|e| io_error(e, &path.display().to_string()))
}

fn write_bytes(path: &Path, data: &[u8], append: bool) -> std::io::Result<()> {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .append(append)
        .truncate(!append)
        .open(path)?;
    file.write_all(data)
}

// ---------------------------------------------------------------------------
// Directory & metadata
// ---------------------------------------------------------------------------

#[op2]
#[serde]
pub async fn op_chq_read_dir(
    state: Rc<RefCell<OpState>>,
    #[string] path: String,
) -> Result<Vec<DirEntry>, JsErrorBox> {
    let path = PathBuf::from(&path);
    check_read(&state, &path)?;
    tokio::task::spawn_blocking(move || {
        let rd = std::fs::read_dir(&path).map_err(|e| io_error(e, &path.display().to_string()))?;
        dir_entries(rd, &path)
    })
    .await
    .map_err(|e| JsErrorBox::generic(e.to_string()))?
}

#[op2]
#[serde]
pub fn op_chq_read_dir_sync(
    state: &mut OpState,
    #[string] path: String,
) -> Result<Vec<DirEntry>, JsErrorBox> {
    let path = PathBuf::from(&path);
    state.borrow::<Permissions>().check_read(&path)?;
    let rd = std::fs::read_dir(&path).map_err(|e| io_error(e, &path.display().to_string()))?;
    dir_entries(rd, &path)
}

#[op2]
pub async fn op_chq_mkdir(
    state: Rc<RefCell<OpState>>,
    #[string] path: String,
    recursive: bool,
) -> Result<(), JsErrorBox> {
    let path = PathBuf::from(&path);
    check_write(&state, &path)?;
    let result = if recursive {
        tokio::fs::create_dir_all(&path).await
    } else {
        tokio::fs::create_dir(&path).await
    };
    result.map_err(|e| io_error(e, &path.display().to_string()))
}

#[op2(fast)]
pub fn op_chq_mkdir_sync(
    state: &mut OpState,
    #[string] path: String,
    recursive: bool,
) -> Result<(), JsErrorBox> {
    let path = PathBuf::from(&path);
    state.borrow::<Permissions>().check_write(&path)?;
    let result = if recursive {
        std::fs::create_dir_all(&path)
    } else {
        std::fs::create_dir(&path)
    };
    result.map_err(|e| io_error(e, &path.display().to_string()))
}

#[op2]
pub async fn op_chq_remove(
    state: Rc<RefCell<OpState>>,
    #[string] path: String,
    recursive: bool,
) -> Result<(), JsErrorBox> {
    let path = PathBuf::from(&path);
    check_write(&state, &path)?;
    let path_for_error = path.clone();
    tokio::task::spawn_blocking(move || remove_path(&path, recursive))
        .await
        .map_err(|e| JsErrorBox::generic(e.to_string()))?
        .map_err(|e| io_error(e, &path_for_error.display().to_string()))
}

#[op2(fast)]
pub fn op_chq_remove_sync(
    state: &mut OpState,
    #[string] path: String,
    recursive: bool,
) -> Result<(), JsErrorBox> {
    let path = PathBuf::from(&path);
    state.borrow::<Permissions>().check_write(&path)?;
    remove_path(&path, recursive).map_err(|e| io_error(e, &path.display().to_string()))
}

fn remove_path(path: &Path, recursive: bool) -> std::io::Result<()> {
    let meta = std::fs::symlink_metadata(path)?;
    if meta.is_dir() {
        if recursive {
            std::fs::remove_dir_all(path)
        } else {
            std::fs::remove_dir(path)
        }
    } else {
        std::fs::remove_file(path)
    }
}

#[op2]
#[serde]
pub async fn op_chq_stat(
    state: Rc<RefCell<OpState>>,
    #[string] path: String,
    lstat: bool,
) -> Result<FileInfo, JsErrorBox> {
    let path = PathBuf::from(&path);
    check_read(&state, &path)?;
    let meta = if lstat {
        tokio::fs::symlink_metadata(&path).await
    } else {
        tokio::fs::metadata(&path).await
    };
    meta.map(file_info)
        .map_err(|e| io_error(e, &path.display().to_string()))
}

#[op2]
#[serde]
pub fn op_chq_stat_sync(
    state: &mut OpState,
    #[string] path: String,
    lstat: bool,
) -> Result<FileInfo, JsErrorBox> {
    let path = PathBuf::from(&path);
    state.borrow::<Permissions>().check_read(&path)?;
    let meta = if lstat {
        std::fs::symlink_metadata(&path)
    } else {
        std::fs::metadata(&path)
    };
    meta.map(file_info)
        .map_err(|e| io_error(e, &path.display().to_string()))
}

/// `exists` intentionally reports `false` (rather than throwing) for a missing
/// path, but still throws when the permission check fails.
#[op2(fast)]
pub fn op_chq_exists_sync(state: &mut OpState, #[string] path: String) -> Result<bool, JsErrorBox> {
    let path = PathBuf::from(&path);
    state.borrow::<Permissions>().check_read(&path)?;
    Ok(path.exists())
}

#[op2]
pub async fn op_chq_copy_file(
    state: Rc<RefCell<OpState>>,
    #[string] from: String,
    #[string] to: String,
) -> Result<(), JsErrorBox> {
    let from = PathBuf::from(&from);
    let to = PathBuf::from(&to);
    check_read(&state, &from)?;
    check_write(&state, &to)?;
    tokio::fs::copy(&from, &to)
        .await
        .map(|_| ())
        .map_err(|e| io_error(e, &format!("{} -> {}", from.display(), to.display())))
}

#[op2]
pub async fn op_chq_rename(
    state: Rc<RefCell<OpState>>,
    #[string] from: String,
    #[string] to: String,
) -> Result<(), JsErrorBox> {
    let from = PathBuf::from(&from);
    let to = PathBuf::from(&to);
    check_write(&state, &from)?;
    check_write(&state, &to)?;
    tokio::fs::rename(&from, &to)
        .await
        .map_err(|e| io_error(e, &format!("{} -> {}", from.display(), to.display())))
}

#[op2]
#[string]
pub fn op_chq_real_path_sync(
    state: &mut OpState,
    #[string] path: String,
) -> Result<String, JsErrorBox> {
    let path = PathBuf::from(&path);
    state.borrow::<Permissions>().check_read(&path)?;
    std::fs::canonicalize(&path)
        .map(|p| p.to_string_lossy().into_owned())
        .map_err(|e| io_error(e, &path.display().to_string()))
}

#[op2]
#[string]
pub fn op_chq_make_temp_dir(
    state: &mut OpState,
    #[string] prefix: String,
) -> Result<String, JsErrorBox> {
    let base = std::env::temp_dir();
    state.borrow::<Permissions>().check_write(&base)?;

    // 8 hex characters of OS randomness is plenty to avoid collisions, and
    // retrying on `AlreadyExists` closes the remaining race.
    for _ in 0..16 {
        let suffix: u64 = rand::random();
        let candidate = base.join(format!("{prefix}{suffix:016x}"));
        match std::fs::create_dir(&candidate) {
            Ok(()) => return Ok(candidate.to_string_lossy().into_owned()),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(io_error(e, &candidate.display().to_string())),
        }
    }
    Err(JsErrorBox::generic(
        "could not create a unique temporary directory",
    ))
}
