//! Explicit saves of reviewed additions/deletions, never model-callable tools.
use std::path::Path;

#[cfg(windows)]
fn lock_parents(path: &Path, create: bool) -> Result<Vec<std::fs::File>, String> {
    use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_READ, FILE_SHARE_WRITE,
    };
    let mut ancestors: Vec<_> = path
        .parent()
        .ok_or("File has no parent.")?
        .ancestors()
        .collect();
    ancestors.reverse();
    let mut guards = Vec::new();
    for directory in ancestors {
        if create
            && std::fs::symlink_metadata(directory)
                .is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound)
        {
            match std::fs::create_dir(directory) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(e) => return Err(e.to_string()),
            }
        }
        // Excluding FILE_SHARE_DELETE keeps each checked ancestor in place.
        let handle = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
            .open(directory)
            .map_err(|e| e.to_string())?;
        let metadata = handle.metadata().map_err(|e| e.to_string())?;
        if !metadata.is_dir() || metadata.file_attributes() & 0x400 != 0 {
            return Err("Reviewed saves cannot follow links or junctions.".into());
        }
        guards.push(handle);
    }
    Ok(guards)
}

#[cfg(windows)]
pub fn create(root: &Path, relative: &str, text: &str) -> Result<(), String> {
    use std::io::Write;
    if text.len() > 128 * 1024 || text.contains('\0') {
        return Err("Reviewed additions require UTF-8 text under 128 KB.".into());
    }
    let context = super::context::Context::new(root, &serde_json::json!([]))?;
    let target = context.target(relative)?;
    let _guards = lock_parents(&target, true)?;
    // Atomic no-clobber publication handles a competing creator at the leaf.
    let mut file =
        tempfile::NamedTempFile::new_in(target.parent().unwrap()).map_err(|e| e.to_string())?;
    file.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
    file.as_file().sync_all().map_err(|e| e.to_string())?;
    file.persist_noclobber(&target).map_err(|e| {
        format!("Could not create reviewed file without overwriting existing data: {e}")
    })?;
    Ok(())
}

#[cfg(windows)]
pub fn delete(root: &Path, relative: &str, expected_revision: &str) -> Result<(), String> {
    use std::{
        io::Read,
        os::windows::{
            fs::{MetadataExt, OpenOptionsExt},
            io::AsRawHandle,
        },
    };
    use windows_sys::Win32::Storage::FileSystem::{
        DELETE, FILE_DISPOSITION_INFO, FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_READ,
        FileDispositionInfo, SetFileInformationByHandle,
    };
    let context = super::context::Context::new(root, &serde_json::json!([]))?;
    let target = context.target(relative)?;
    let _guards = lock_parents(&target, false)?;
    // Verify and delete the same handle; concurrent writers/replacements cannot
    // change the object between the content check and disposition request.
    let file = std::fs::OpenOptions::new()
        .read(true)
        .access_mode(0x8000_0000 | DELETE)
        .share_mode(FILE_SHARE_READ)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(&target)
        .map_err(|e| e.to_string())?;
    let metadata = file.metadata().map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.file_attributes() & 0x400 != 0 {
        return Err("Reviewed deletion requires a regular file without links.".into());
    }
    let mut bytes = Vec::new();
    (&file)
        .take(128 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 128 * 1024 || super::snapshot::revision(&bytes) != expected_revision {
        return Err("The file changed on disk. Its contents were preserved.".into());
    }
    let mut disposition = FILE_DISPOSITION_INFO { DeleteFile: true };
    // SAFETY: the live File owns this handle; the buffer and its size match
    // FileDispositionInfo. Drop closes the marked handle after this call.
    let deleted = unsafe {
        SetFileInformationByHandle(
            file.as_raw_handle(),
            FileDispositionInfo,
            (&mut disposition as *mut FILE_DISPOSITION_INFO).cast(),
            std::mem::size_of::<FILE_DISPOSITION_INFO>() as u32,
        )
    };
    if deleted == 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    drop(file);
    Ok(())
}

#[cfg(not(windows))]
pub fn create(_: &Path, _: &str, _: &str) -> Result<(), String> {
    Err("Reviewed file creation is not available on this platform.".into())
}
#[cfg(not(windows))]
pub fn delete(_: &Path, _: &str, _: &str) -> Result<(), String> {
    Err("Reviewed file deletion is not available on this platform.".into())
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    #[test]
    fn additions_never_clobber_and_deletions_check_the_opened_file() {
        let root = tempfile::tempdir().unwrap();
        create(root.path(), "notes/new.txt", "new\r\n").unwrap();
        assert!(create(root.path(), "notes/new.txt", "overwrite").is_err());
        assert_eq!(
            std::fs::read_to_string(root.path().join("notes/new.txt")).unwrap(),
            "new\r\n"
        );
        assert!(
            delete(
                root.path(),
                "notes/new.txt",
                &super::super::snapshot::revision(b"wrong")
            )
            .is_err()
        );
        assert!(root.path().join("notes/new.txt").exists());
        delete(
            root.path(),
            "notes/new.txt",
            &super::super::snapshot::revision(b"new\r\n"),
        )
        .unwrap();
        assert!(!root.path().join("notes/new.txt").exists());
        assert!(
            delete(
                root.path(),
                "notes/new.txt",
                &super::super::snapshot::revision(b"new\r\n")
            )
            .is_err()
        );
        for path in [
            "../outside.txt",
            ".dado/secret.txt",
            "notes/new.txt:stream",
            "C:/outside.txt",
            "notes/NUL.txt",
            "notes/trailing.",
        ] {
            assert!(create(root.path(), path, "bad").is_err());
        }
    }
    #[test]
    fn ancestor_guards_block_replacement_until_the_save_finishes() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("notes")).unwrap();
        let target = root.path().join("notes/file.txt");
        let guards = lock_parents(&target, false).unwrap();
        assert!(std::fs::rename(root.path().join("notes"), root.path().join("moved")).is_err());
        drop(guards);
        std::fs::rename(root.path().join("notes"), root.path().join("moved")).unwrap();
    }
    #[test]
    fn active_writers_block_deletion_instead_of_losing_concurrent_work() {
        let root = tempfile::tempdir().unwrap();
        let target = root.path().join("file.txt");
        std::fs::write(&target, "baseline").unwrap();
        let writer = std::fs::OpenOptions::new()
            .write(true)
            .open(&target)
            .unwrap();
        assert!(
            delete(
                root.path(),
                "file.txt",
                &super::super::snapshot::revision(b"baseline")
            )
            .is_err()
        );
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "baseline");
        drop(writer);
        delete(
            root.path(),
            "file.txt",
            &super::super::snapshot::revision(b"baseline"),
        )
        .unwrap();
        assert!(!target.exists());
    }
}
