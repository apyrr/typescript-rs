use std::{io, path::Path, process::Command};

pub fn mklink(target: &Path, link: &Path, is_dir: bool) -> io::Result<()> {
    if cfg!(windows) && is_dir {
        // Don't use os.Symlink on Windows, as it creates a "real" symlink, not a junction.
        let status = Command::new("cmd")
            .args(["/c", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .status()?;
        if status.success() {
            return Ok(());
        }
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!("mklink exited with {status}"),
        ));
    }

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link)
    }

    #[cfg(windows)]
    {
        let err = if is_dir {
            std::os::windows::fs::symlink_dir(target, link)
        } else {
            std::os::windows::fs::symlink_file(target, link)
        };
        if let Err(err) = &err {
            if !is_dir
                && err
                    .to_string()
                    .contains("A required privilege is not held by the client")
            {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    err.to_string(),
                ));
            }
        }
        err
    }

    #[cfg(not(any(unix, windows)))]
    {
        let _ = (target, link, is_dir);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "symlinks are not supported on this platform",
        ))
    }
}
