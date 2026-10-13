//! Safe saving: a file is written to a temporary file beside it, then renamed over it, so a failed
//! or interrupted write never leaves a damaged file behind (the old one stays as it was).
//! [`write_new_with`] creates a file the same way without ever replacing one.
//!
//! On macOS `File::sync_all` is a full flush (`F_FULLFSYNC`), which SMB shares refuse with
//! `ENOTSUP` while a plain `fsync` works: [`sync_file`] falls back to the plain one there (#1620).

use std::fs::{File, OpenOptions};
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// Write `bytes` to `path` atomically (see the module docs).
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    write_atomic_with(path, |f| f.write_all(bytes))
}

/// Write `path` atomically with `fill`, which writes the content into the temporary file. When
/// anything fails (`fill`, flushing, the rename) the temporary file is removed and `path` is left
/// untouched.
pub fn write_atomic_with(path: &Path, fill: impl FnOnce(&mut File) -> io::Result<()>) -> io::Result<()> {
    let target = resolve_link(path)?;
    let tmp = temp_path(&target)?;
    let mut f = OpenOptions::new().write(true).create_new(true).open(&tmp)?;
    let filled = fill(&mut f).and_then(|()| sync_file(&f));
    drop(f);
    filled.and_then(|()| keep_permissions(&target, &tmp)).and_then(|()| std::fs::rename(&tmp, &target)).inspect_err(|_| {
        // Best effort: the temporary file is all there is to clean up.
        let _ = std::fs::remove_file(&tmp);
    })
}

/// Create `path` with the content `fill` writes, never replacing a file or a link of that name. The
/// content goes to a temporary file beside it, which is then linked in under `path`
/// (`std::fs::hard_link`, which fails with `AlreadyExists` when the name is taken, also when it was
/// taken meanwhile). The file appears whole or not at all. Where hard links fail for another
/// reason, `path` is created with `create_new` and the content copied in; a failed copy removes
/// it. The temporary file is always removed.
pub fn write_new_with(path: &Path, fill: impl FnOnce(&mut File) -> io::Result<()>) -> io::Result<()> {
    write_new_linking(path, fill, |tmp, path| std::fs::hard_link(tmp, path))
}

/// [`write_new_with`], with `link` linking the temporary file in under `path`.
fn write_new_linking(
    path: &Path,
    fill: impl FnOnce(&mut File) -> io::Result<()>,
    link: impl FnOnce(&Path, &Path) -> io::Result<()>,
) -> io::Result<()> {
    let tmp = temp_path(path)?;
    let mut f = OpenOptions::new().write(true).create_new(true).open(&tmp)?;
    let filled = fill(&mut f).and_then(|()| sync_file(&f));
    drop(f);
    let placed = filled.and_then(|()| match link(&tmp, path) {
        Err(e) if e.kind() != io::ErrorKind::AlreadyExists => copy_new(&tmp, path),
        linked => linked,
    });
    // Best effort: the temporary file is all there is to clean up.
    let _ = std::fs::remove_file(&tmp);
    placed
}

/// Create `path` (never replacing anything) with the content of the file `from`; a failed copy
/// removes it again.
fn copy_new(from: &Path, path: &Path) -> io::Result<()> {
    let mut to = OpenOptions::new().write(true).create_new(true).open(path)?;
    let copied = File::open(from).and_then(|mut f| io::copy(&mut f, &mut to)).and_then(|_| sync_file(&to));
    drop(to);
    if copied.is_err() {
        // Best effort: the part copied is all there is to clean up.
        let _ = std::fs::remove_file(path);
    }
    copied
}

/// Flush `file` to stable storage (`sync_all`). On Apple platforms that is a full flush
/// (`F_FULLFSYNC`); where the volume refuses it with `ENOTSUP` (SMB shares, #1620) this falls back
/// to a plain `fsync`. Any other error, and a failed `fsync`, is returned as it is.
pub fn sync_file(file: &File) -> io::Result<()> {
    match full_sync(file) {
        Err(e) if full_flush_unsupported(&e) => plain_fsync(file),
        r => r,
    }
}

/// Whether `e` is a volume refusing a full flush. Never off Apple platforms: there `sync_all` is
/// already a plain `fsync`. Checked by its code because std maps `ENOTSUP` to `Uncategorized`.
fn full_flush_unsupported(e: &io::Error) -> bool {
    cfg!(target_vendor = "apple") && e.raw_os_error() == Some(ENOTSUP)
}

#[cfg(target_vendor = "apple")]
fn plain_fsync(file: &File) -> io::Result<()> {
    #[cfg(test)]
    {
        PLAIN_FSYNCS.with(|c| c.set(c.get() + 1));
        if let Some(errno) = PLAIN_FSYNC_FAILS.with(std::cell::Cell::get) {
            return Err(io::Error::from_raw_os_error(errno));
        }
    }
    rustix::fs::fsync(file).map_err(io::Error::from)
}

#[cfg(not(target_vendor = "apple"))]
fn plain_fsync(file: &File) -> io::Result<()> {
    file.sync_all()
}

fn full_sync(file: &File) -> io::Result<()> {
    #[cfg(test)]
    if REFUSE_FULL_SYNC.with(std::cell::Cell::get) {
        return Err(io::Error::from_raw_os_error(ENOTSUP));
    }
    file.sync_all()
}

/// What macOS answers when a volume can't make a full flush (SMB shares).
#[cfg(target_vendor = "apple")]
const ENOTSUP: i32 = 45;
#[cfg(not(target_vendor = "apple"))]
const ENOTSUP: i32 = 95;

#[cfg(test)]
thread_local! {
    /// Tests: every full flush on this thread fails as on an SMB share mounted by macOS.
    static REFUSE_FULL_SYNC: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// Tests: the plain `fsync` fallback fails too, with this OS error.
    static PLAIN_FSYNC_FAILS: std::cell::Cell<Option<i32>> = const { std::cell::Cell::new(None) };
    /// Tests: how many plain `fsync` fallbacks were made on this thread.
    static PLAIN_FSYNCS: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// A symbolic link keeps pointing at its file: write the file it names.
fn resolve_link(path: &Path) -> io::Result<PathBuf> {
    match std::fs::symlink_metadata(path) {
        Ok(m) if m.file_type().is_symlink() => std::fs::canonicalize(path),
        _ => Ok(path.to_path_buf()),
    }
}

/// A fresh hidden name beside `target` (the same folder, so the rename stays on one volume).
fn temp_path(target: &Path) -> io::Result<PathBuf> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let name = target.file_name().ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, format!("{}: not a file path", target.display())))?;
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    Ok(target.with_file_name(format!(".{}.{}-{n}.tmp", name.to_string_lossy(), std::process::id())))
}

/// The replacement keeps the replaced file's permissions (Unix; Windows files have none to keep
/// beyond read-only, which makes the save fail as writing in place would).
#[cfg(unix)]
fn keep_permissions(target: &Path, tmp: &Path) -> io::Result<()> {
    match std::fs::metadata(target) {
        Ok(m) => std::fs::set_permissions(tmp, m.permissions()),
        // A new file: the default permissions.
        Err(_) => Ok(()),
    }
}

#[cfg(not(unix))]
fn keep_permissions(_: &Path, _: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("vc-atomic-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// The other files in `d` besides `keep`.
    fn leftovers(d: &Path, keep: &str) -> Vec<String> {
        std::fs::read_dir(d).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().to_string()).filter(|n| n != keep).collect()
    }

    #[test]
    fn the_original_file_survives_a_failed_write() {
        let d = dir("fail");
        let path = d.join("doc.vectorcraft");
        std::fs::write(&path, b"original").unwrap();
        let r = write_atomic_with(&path, |f| {
            f.write_all(b"half a fi")?;
            Err(io::Error::other("disk full"))
        });
        assert!(r.is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"original");
        assert!(leftovers(&d, "doc.vectorcraft").is_empty(), "the temporary file is removed");
        write_atomic(&path, b"new").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"new");
        assert!(leftovers(&d, "doc.vectorcraft").is_empty());
        // A folder that doesn't exist: an error, nothing written.
        assert!(write_atomic(&d.join("missing").join("x.vectorcraft"), b"x").is_err());
        let _ = std::fs::remove_dir_all(&d);
    }

    /// A save to an SMB share (#1620): macOS refuses the full flush there with `ENOTSUP`, so the
    /// file is flushed with a plain `fsync` and saved. When that fails too the save fails as before,
    /// the old file stays and no temporary file is left. Elsewhere `sync_all` is already a plain
    /// `fsync`: a refusal is an error like any other.
    #[test]
    fn a_volume_that_refuses_the_full_flush_still_saves() {
        let d = dir("no-full-sync");
        let path = d.join("doc.vectorcraft");
        std::fs::write(&path, b"original").unwrap();
        let apple = cfg!(target_vendor = "apple");
        REFUSE_FULL_SYNC.with(|c| c.set(true));
        let before = PLAIN_FSYNCS.with(std::cell::Cell::get);
        let saved = write_atomic(&path, b"new");
        let created = write_new_with(&d.join("new.otf"), |f| f.write_all(b"created"));
        let fsyncs = PLAIN_FSYNCS.with(std::cell::Cell::get) - before;
        PLAIN_FSYNC_FAILS.with(|c| c.set(Some(5)));
        let failed = write_atomic(&path, b"lost");
        PLAIN_FSYNC_FAILS.with(|c| c.set(None));
        REFUSE_FULL_SYNC.with(|c| c.set(false));
        assert_eq!(saved.is_ok(), apple, "{saved:?}");
        assert_eq!(created.is_ok(), apple, "{created:?}");
        assert_eq!(fsyncs, if apple { 2 } else { 0 }, "each refused full flush falls back to fsync");
        assert!(failed.is_err(), "a failed fsync still fails the save");
        assert_eq!(std::fs::read(&path).unwrap(), if apple { &b"new"[..] } else { b"original" });
        let mut left = leftovers(&d, "doc.vectorcraft");
        left.retain(|n| n != "new.otf");
        assert!(left.is_empty(), "no temporary file is left: {left:?}");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn write_new_never_replaces_what_is_there() {
        let d = dir("new");
        let path = d.join("a.otf");
        write_new_with(&path, |f| f.write_all(b"first")).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"first");
        assert!(leftovers(&d, "a.otf").is_empty(), "the temporary file is removed");
        let r = write_new_with(&path, |f| f.write_all(b"second"));
        assert_eq!(r.map_err(|e| e.kind()), Err(io::ErrorKind::AlreadyExists));
        assert_eq!(std::fs::read(&path).unwrap(), b"first");
        assert!(leftovers(&d, "a.otf").is_empty());
        // A failed write leaves nothing.
        let failed = d.join("b.otf");
        assert!(write_new_with(&failed, |f| f.write_all(b"half").and(Err(io::Error::other("disk full")))).is_err());
        assert!(!failed.exists() && leftovers(&d, "a.otf").is_empty());
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Where hard links fail (a file system without them), the content is copied in, and a taken
    /// name stays as it was. A link that fails because the name is taken copies nothing.
    #[test]
    fn write_new_copies_where_hard_links_fail() {
        let d = dir("new-copy");
        let unsupported = |_: &Path, _: &Path| Err(io::Error::from(io::ErrorKind::Unsupported));
        let path = d.join("c.otf");
        write_new_linking(&path, |f| f.write_all(b"copied"), unsupported).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"copied");
        assert!(leftovers(&d, "c.otf").is_empty(), "the temporary file is removed");
        let r = write_new_linking(&path, |f| f.write_all(b"second"), unsupported);
        assert_eq!(r.map_err(|e| e.kind()), Err(io::ErrorKind::AlreadyExists));
        assert_eq!(std::fs::read(&path).unwrap(), b"copied");
        let taken = |_: &Path, _: &Path| Err(io::Error::from(io::ErrorKind::AlreadyExists));
        let other = d.join("d.otf");
        let r = write_new_linking(&other, |f| f.write_all(b"other"), taken);
        assert_eq!(r.map_err(|e| e.kind()), Err(io::ErrorKind::AlreadyExists));
        assert!(!other.exists() && leftovers(&d, "c.otf").is_empty());
        let _ = std::fs::remove_dir_all(&d);
    }

    /// A link of that name, even one to nothing, is never replaced or written through.
    #[cfg(unix)]
    #[test]
    fn write_new_leaves_links_alone() {
        let d = dir("new-links");
        let target = d.join("target.otf");
        std::fs::write(&target, b"target").unwrap();
        std::os::unix::fs::symlink(&target, d.join("link.otf")).unwrap();
        std::os::unix::fs::symlink(d.join("nothing.otf"), d.join("dangling.otf")).unwrap();
        for name in ["link.otf", "dangling.otf"] {
            let r = write_new_with(&d.join(name), |f| f.write_all(b"new"));
            assert_eq!(r.map_err(|e| e.kind()), Err(io::ErrorKind::AlreadyExists), "{name}");
        }
        assert_eq!(std::fs::read(&target).unwrap(), b"target");
        assert!(!d.join("nothing.otf").exists());
        let mut names: Vec<String> = std::fs::read_dir(&d).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().to_string()).collect();
        names.sort();
        assert_eq!(names, ["dangling.otf", "link.otf", "target.otf"], "no temporary file is left");
        let _ = std::fs::remove_dir_all(&d);
    }
}
