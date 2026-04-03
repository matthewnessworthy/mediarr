use std::path::Path;

use crate::error::{MediError, Result};

/// Move a file from source to dest, with EXDEV cross-filesystem fallback.
///
/// On same filesystem: uses `std::fs::rename` (atomic).
/// On cross-filesystem (EXDEV): copies file, verifies size, removes source.
///
/// This is the ONLY function in mediarr-core that removes a source file,
/// and only after a verified cross-filesystem copy.
pub fn safe_move(source: &Path, dest: &Path) -> Result<()> {
    match std::fs::rename(source, dest) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::CrossesDevices => {
            // Cross-filesystem: copy, verify size, remove source
            std::fs::copy(source, dest)?;

            let source_len = std::fs::metadata(source)?.len();
            let dest_len = std::fs::metadata(dest)?.len();

            if source_len != dest_len {
                // Clean up the bad copy before returning error
                let _ = std::fs::remove_file(dest);
                return Err(MediError::CopyVerificationFailed {
                    from: source.into(),
                    to: dest.into(),
                });
            }

            std::fs::remove_file(source)?;
            Ok(())
        }
        Err(e) => Err(MediError::RenameFailed {
            from: source.into(),
            to: dest.into(),
            cause: e,
        }),
    }
}

/// Convert a Path to a UTF-8 string, returning `MediError::NonUtf8Path` if invalid.
///
/// Use this instead of `to_string_lossy()` for any path that must round-trip
/// through storage (e.g. SQLite, JSON, TOML) without data loss.
pub fn path_to_utf8(path: &Path) -> Result<&str> {
    path.to_str().ok_or_else(|| MediError::NonUtf8Path {
        path: path.to_path_buf(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_safe_move_same_filesystem() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("source.txt");
        let dst = dir.path().join("dest.txt");

        std::fs::write(&src, "hello world").unwrap();
        safe_move(&src, &dst).unwrap();

        assert!(!src.exists());
        assert!(dst.exists());
        assert_eq!(std::fs::read_to_string(&dst).unwrap(), "hello world");
    }

    #[test]
    fn test_safe_move_source_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("nonexistent.txt");
        let dst = dir.path().join("dest.txt");

        let result = safe_move(&src, &dst);
        assert!(result.is_err());
    }

    #[test]
    fn test_path_to_utf8_valid() {
        let path = PathBuf::from("/valid/utf8/path.txt");
        let result = path_to_utf8(&path);
        assert_eq!(result.unwrap(), "/valid/utf8/path.txt");
    }

    #[cfg(unix)]
    #[test]
    fn test_path_to_utf8_invalid() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;

        // Create a path with invalid UTF-8 bytes
        let invalid_bytes: &[u8] = &[0xff, 0xfe];
        let os_str = OsStr::from_bytes(invalid_bytes);
        let path = PathBuf::from(os_str);

        let result = path_to_utf8(&path);
        assert!(result.is_err());
    }
}
