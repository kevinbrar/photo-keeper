//! What Photo Keeper adds to the importer: a checksum for every file, a check
//! that the files on disk still match those checksums, and a second copy of
//! the library on another disk.
//!
//! Nothing here talks to the phone and nothing here deletes or overwrites a
//! photo. The rules this module must keep are in DECISIONS.md.

use std::{
    collections::HashMap,
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::UNIX_EPOCH,
};

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{Context, library::room, phone::MEDIA};

/// The checksums of a folder, kept in the folder so they travel with the disk.
pub const MANIFEST: &str = "photo-keeper-checksums.db";

const CHUNK: usize = 1 << 20;

pub fn hex(digest: impl AsRef<[u8]>) -> String {
    digest
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// The SHA-256 of a file as the disk gives it back now.
pub fn hash_file(
    path: &Path,
    cancel: &AtomicBool,
    mut read: impl FnMut(u64),
) -> io::Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut chunk = vec![0; CHUNK];

    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(io::Error::new(io::ErrorKind::Interrupted, "stopped"));
        }

        let n = file.read(&mut chunk)?;
        if n == 0 {
            break;
        }

        hasher.update(&chunk[..n]);
        read(n as u64);
    }

    Ok(hex(hasher.finalize()))
}

/// A path inside the folder, written with `/` on every system: the same disk
/// is read on a Mac and on Windows.
pub fn relative(folder: &Path, file: &Path) -> Option<String> {
    let inside = file.strip_prefix(folder).ok()?;
    let parts: Option<Vec<&str>> = inside
        .components()
        .map(|part| match part {
            std::path::Component::Normal(name) => name.to_str(),
            _ => None,
        })
        .collect();

    Some(parts?.join("/"))
}

fn absolute(folder: &Path, relative: &str) -> PathBuf {
    relative
        .split('/')
        .fold(folder.to_path_buf(), |path, part| path.join(part))
}

/// Where a checksum came from.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Source {
    /// The bytes as the phone sent them, hashed during the import.
    Phone,
    /// The file as it was on disk when first seen: it was imported before
    /// checksums were kept, or put in the folder by hand.
    Disk,
}

impl Source {
    fn stored(self) -> &'static str {
        match self {
            Source::Phone => "phone",
            Source::Disk => "disk",
        }
    }
}

pub struct Manifest(rusqlite::Connection);

impl Manifest {
    pub fn open(folder: &Path) -> Result<Self, String> {
        let db = rusqlite::Connection::open(folder.join(MANIFEST))
            .doing("cannot open photo-keeper-checksums.db")?;

        db.busy_timeout(std::time::Duration::from_secs(10))
            .doing("cannot prepare photo-keeper-checksums.db")?;

        db.execute_batch(
            "CREATE TABLE IF NOT EXISTS checksums (
                 path     TEXT PRIMARY KEY,
                 size     INTEGER NOT NULL,
                 sha256   TEXT NOT NULL,
                 source   TEXT NOT NULL,
                 added_at DATETIME DEFAULT CURRENT_TIMESTAMP
             );",
        )
        .doing("cannot prepare photo-keeper-checksums.db")?;

        Ok(Manifest(db))
    }

    /// Write a checksum down. One already there for this path is kept unless
    /// this one comes from the phone: a checksum is what the file is compared
    /// with, so the file itself must never quietly replace it.
    pub fn record(
        &self,
        path: &str,
        size: u64,
        sha256: &str,
        source: Source,
    ) -> Result<(), String> {
        let sql = match source {
            Source::Phone => "INSERT OR REPLACE",
            Source::Disk => "INSERT OR IGNORE",
        };

        self.0
            .execute(
                &format!(
                    "{sql} INTO checksums (path, size, sha256, source) VALUES (?1, ?2, ?3, ?4)"
                ),
                rusqlite::params![path, size as i64, sha256, source.stored()],
            )
            .map(|_| ())
            .doing("cannot record the checksum")
    }

    pub fn all(&self) -> Result<HashMap<String, (u64, String)>, String> {
        self.0
            .prepare("SELECT path, size, sha256 FROM checksums")
            .and_then(|mut sql| {
                sql.query_map([], |row| {
                    Ok((row.get(0)?, (row.get::<_, i64>(1)? as u64, row.get(2)?)))
                })?
                .collect()
            })
            .doing("cannot read photo-keeper-checksums.db")
    }
}

/// A photo or video in the folder.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    /// Inside the folder, with `/`.
    pub path: String,
    pub size: u64,
    /// Seconds since 1970. The import sets it to when the photo was taken.
    pub taken: i64,
    pub video: bool,
}

const VIDEO: [&str; 5] = ["mov", "mp4", "m4v", "avi", "3gp"];

/// Every photo and video in the folder, newest first. Hidden files and
/// folders are passed over: the scratch files of a copy start with a dot.
/// Whether the file is a photo or video, by its name.
pub fn is_media(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| MEDIA.contains(&e.to_lowercase().as_str()))
}

pub fn items(folder: &Path) -> Result<Vec<Item>, String> {
    if !folder.is_dir() {
        return Err("cannot find the folder".into());
    }

    let mut found = Vec::new();
    let mut waiting = vec![folder.to_path_buf()];

    while let Some(dir) = waiting.pop() {
        for entry in fs::read_dir(&dir).doing("cannot read the folder")? {
            let entry = entry.doing("cannot read the folder")?;
            let path = entry.path();
            let name = entry.file_name();

            if name.to_string_lossy().starts_with('.') {
                continue;
            }

            let Ok(meta) = entry.metadata() else { continue };

            if meta.is_dir() {
                waiting.push(path);
                continue;
            }

            let Some(extension) = path
                .extension()
                .and_then(|e| e.to_str())
                .map(str::to_lowercase)
            else {
                continue;
            };

            if !MEDIA.contains(&extension.as_str()) {
                continue;
            }

            let Some(relative) = relative(folder, &path) else {
                continue;
            };

            found.push(Item {
                path: relative,
                size: meta.len(),
                taken: meta
                    .modified()
                    .ok()
                    .and_then(|when| when.duration_since(UNIX_EPOCH).ok())
                    .map_or(0, |since| since.as_secs() as i64),
                video: VIDEO.contains(&extension.as_str()),
            });
        }
    }

    found.sort_by(|a, b| b.taken.cmp(&a.taken).then_with(|| a.path.cmp(&b.path)));
    Ok(found)
}

/// How far a check or a second copy has got.
#[derive(Serialize, Clone, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub done: usize,
    pub total: usize,
    pub bytes: u64,
    pub total_bytes: u64,
    pub name: String,
}

/// One file that is not as it should be, and what is wrong with it.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Problem {
    pub path: String,
    pub reason: String,
}

/// What checking a folder against its checksums found.
#[derive(Serialize, Clone, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Checked {
    /// Read back from the disk and equal to their checksum.
    pub matched: usize,
    /// Of `matched`, those whose checksum is of the bytes the phone sent.
    pub from_phone: usize,
    /// In the folder with no checksum until now. They are given one, of the
    /// file as it is: this check cannot say they match the phone.
    pub adopted: usize,
    /// Different from their checksum, or gone.
    pub problems: Vec<Problem>,
    pub stopped: bool,
}

/// Read every photo and video in the folder back from the disk and compare it
/// with its checksum.
pub fn check(
    folder: &Path,
    cancel: &AtomicBool,
    mut progress: impl FnMut(&Progress),
) -> Result<Checked, String> {
    let on_disk = items(folder)?;
    let manifest = Manifest::open(folder)?;
    let known = manifest.all()?;

    let phone: std::collections::HashSet<String> = manifest
        .0
        .prepare("SELECT path FROM checksums WHERE source = 'phone'")
        .and_then(|mut sql| sql.query_map([], |row| row.get(0))?.collect())
        .doing("cannot read photo-keeper-checksums.db")?;

    let mut checked = Checked::default();
    let mut at = Progress {
        total: on_disk.len(),
        total_bytes: on_disk.iter().map(|item| item.size).sum(),
        ..Progress::default()
    };

    let present: std::collections::HashSet<&str> =
        on_disk.iter().map(|item| item.path.as_str()).collect();

    for (path, _) in known
        .iter()
        .filter(|(path, _)| !present.contains(path.as_str()))
    {
        checked.problems.push(Problem {
            path: path.clone(),
            reason: "missing from the folder".into(),
        });
    }

    for item in &on_disk {
        at.name = item.path.clone();
        progress(&at);

        let before = at.bytes;
        let hashed = hash_file(&absolute(folder, &item.path), cancel, |n| {
            at.bytes += n;
        });
        at.bytes = before + item.size;
        at.done += 1;

        match (hashed, known.get(&item.path)) {
            (Err(e), _) if e.kind() == io::ErrorKind::Interrupted => {
                checked.stopped = true;
                break;
            }
            (Err(e), _) => checked.problems.push(Problem {
                path: item.path.clone(),
                reason: format!("cannot be read: {e}"),
            }),
            (Ok(sha256), Some((_, expected))) if sha256 == *expected => {
                checked.matched += 1;
                if phone.contains(&item.path) {
                    checked.from_phone += 1;
                }
            }
            (Ok(_), Some(_)) => checked.problems.push(Problem {
                path: item.path.clone(),
                reason: "changed since it was copied".into(),
            }),
            (Ok(sha256), None) => {
                manifest.record(&item.path, item.size, &sha256, Source::Disk)?;
                checked.adopted += 1;
            }
        }
    }

    checked.problems.sort_by(|a, b| a.path.cmp(&b.path));
    progress(&at);
    Ok(checked)
}

/// What making the second copy did.
#[derive(Serialize, Clone, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Mirrored {
    pub copied: usize,
    /// Already there with the same checksum on record.
    pub already: usize,
    pub bytes: u64,
    pub problems: Vec<Problem>,
    pub stopped: bool,
    /// The second disk ran out of room.
    pub full: bool,
}

/// Copy every photo and video of `from` that `to` does not hold yet. A file is
/// written beside its place, read back and compared, and only then given its
/// name: `to` never holds a file under its name that did not match.
///
/// Nothing in `to` is replaced or removed. A file there that differs from the
/// one in `from` is reported and left alone.
/// The second copy is no copy if it is the library, or lies inside it.
fn apart(folder: &Path, second: &Path) -> Result<(), String> {
    let (Ok(a), Ok(b)) = (folder.canonicalize(), second.canonicalize()) else {
        return Err("cannot read the folders".into());
    };

    if a.starts_with(&b) || b.starts_with(&a) {
        return Err("the second copy must be in a different folder, on a different disk".into());
    }

    Ok(())
}

/// Whether both folders are on one disk, where one failure takes both copies.
/// Unix compares the devices. Windows compares the drive letters, so two
/// partitions of one physical disk still pass as two.
fn same_disk(a: &Path, b: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        matches!((fs::metadata(a), fs::metadata(b)), (Ok(a), Ok(b)) if a.dev() == b.dev())
    }
    #[cfg(not(unix))]
    {
        let drive = |path: &Path| {
            path.canonicalize()
                .ok()
                .and_then(|path| match path.components().next() {
                    Some(std::path::Component::Prefix(prefix)) => {
                        Some(prefix.as_os_str().to_owned())
                    }
                    _ => None,
                })
        };
        matches!((drive(a), drive(b)), (Some(a), Some(b)) if a == b)
    }
}

pub fn mirror(
    from: &Path,
    to: &Path,
    cancel: &AtomicBool,
    mut progress: impl FnMut(&Progress),
) -> Result<Mirrored, String> {
    if !to.is_dir() {
        return Err("cannot find the folder for the second copy".into());
    }

    apart(from, to)?;

    let source = items(from)?;
    let ours = Manifest::open(from)?;
    let theirs = Manifest::open(to)?;
    let known = ours.all()?;
    let held = theirs.all()?;

    let mut mirrored = Mirrored::default();
    let mut at = Progress {
        total: source.len(),
        total_bytes: source.iter().map(|item| item.size).sum(),
        ..Progress::default()
    };

    for item in &source {
        if cancel.load(Ordering::Relaxed) {
            mirrored.stopped = true;
            break;
        }

        at.name = item.path.clone();
        progress(&at);

        let target = absolute(to, &item.path);
        let expected = known.get(&item.path).map(|(_, sha256)| sha256.as_str());
        let before = at.bytes;

        let outcome = if target.exists() {
            match (held.get(&item.path), expected) {
                (Some((size, theirs)), Some(ours)) if theirs == ours && *size == item.size => {
                    Ok(false)
                }
                (Some(_), Some(_)) => Err("the second copy holds a different file".to_string()),
                // One side has no checksum yet: compare the files themselves.
                _ => same(&absolute(from, &item.path), &target, cancel).and_then(|sha256| {
                    ours.record(&item.path, item.size, &sha256, Source::Disk)?;
                    theirs.record(&item.path, item.size, &sha256, Source::Disk)?;
                    Ok(false)
                }),
            }
        } else if room(to) < item.size {
            mirrored.full = true;
            break;
        } else {
            send(
                &absolute(from, &item.path),
                &target,
                expected,
                item,
                cancel,
                |n| {
                    at.bytes += n;
                },
            )
            .and_then(|sha256| {
                if expected.is_none() {
                    ours.record(&item.path, item.size, &sha256, Source::Disk)?;
                }
                theirs.record(&item.path, item.size, &sha256, Source::Disk)?;
                Ok(true)
            })
        };

        at.bytes = before + item.size;
        at.done += 1;

        match outcome {
            Ok(true) => {
                mirrored.copied += 1;
                mirrored.bytes += item.size;
            }
            Ok(false) => mirrored.already += 1,
            Err(reason) if reason == "stopped" => {
                mirrored.stopped = true;
                break;
            }
            Err(reason) => mirrored.problems.push(Problem {
                path: item.path.clone(),
                reason,
            }),
        }
    }

    progress(&at);
    Ok(mirrored)
}

fn told(e: io::Error) -> String {
    if e.kind() == io::ErrorKind::Interrupted {
        "stopped".into()
    } else {
        e.to_string()
    }
}

/// The checksum two files share, or why they do not.
fn same(a: &Path, b: &Path, cancel: &AtomicBool) -> Result<String, String> {
    let first = hash_file(a, cancel, |_| {}).map_err(told)?;
    let second = hash_file(b, cancel, |_| {}).map_err(told)?;

    if first == second {
        Ok(first)
    } else {
        Err("the second copy holds a different file".into())
    }
}

/// Copy one file and prove the copy. Answers its checksum.
fn send(
    from: &Path,
    target: &Path,
    expected: Option<&str>,
    item: &Item,
    cancel: &AtomicBool,
    mut wrote: impl FnMut(u64),
) -> Result<String, String> {
    let Some(parent) = target.parent() else {
        return Err("has no folder".into());
    };
    fs::create_dir_all(parent).doing("cannot make the folder")?;

    let name = target.file_name().unwrap_or_default().to_string_lossy();
    let partial = parent.join(format!(".{name}.keeper-partial"));

    struct Partial<'a>(Option<&'a Path>);
    impl Drop for Partial<'_> {
        fn drop(&mut self) {
            if let Some(path) = self.0 {
                let _ = fs::remove_file(path);
            }
        }
    }
    let mut clean = Partial(Some(&partial));

    let mut source = fs::File::open(from).doing("cannot read the original")?;
    let mut sink = fs::File::create(&partial).doing("cannot write the copy")?;
    let mut hasher = Sha256::new();
    let mut chunk = vec![0; CHUNK];

    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err("stopped".into());
        }

        let n = source.read(&mut chunk).doing("cannot read the original")?;
        if n == 0 {
            break;
        }

        hasher.update(&chunk[..n]);
        sink.write_all(&chunk[..n]).doing("cannot write the copy")?;
        wrote(n as u64);
    }

    sink.sync_all().doing("cannot write the copy")?;
    drop(sink);

    let read = hex(hasher.finalize());

    if expected.is_some_and(|expected| expected != read) {
        return Err("the original has changed since it was copied from the phone".into());
    }

    let landed = hash_file(&partial, cancel, |_| {}).map_err(told)?;
    if landed != read {
        return Err("the copy did not match the original".into());
    }

    // Never over a file that appeared meanwhile.
    if target.exists() {
        return Err("the second copy holds a different file".into());
    }

    fs::rename(&partial, target).doing("cannot name the copy")?;
    clean.0 = None;

    let _ = fs::File::options()
        .write(true)
        .open(target)
        .and_then(|done| {
            done.set_modified(UNIX_EPOCH + std::time::Duration::from_secs(item.taken.max(0) as u64))
        });

    Ok(read)
}

/// What the window says about whether the phone's photos can go.
#[derive(Serialize, Clone, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Safety {
    pub library: Checked,
    /// None when no second copy is set up, or its disk is not plugged in.
    pub second: Option<Checked>,
    /// Photos in the library that the second copy does not hold.
    pub only_here: usize,
    /// Both copies are on one disk: one failure would take both.
    pub same_disk: bool,
}

/// Check the library, and the second copy when it is there.
pub fn safety(
    folder: &Path,
    second: Option<&Path>,
    cancel: &AtomicBool,
    mut progress: impl FnMut(&str, &Progress),
) -> Result<Safety, String> {
    let library = check(folder, cancel, |at| progress("library", at))?;

    let second = second.filter(|path| path.is_dir() && !library.stopped);
    let Some(second) = second else {
        return Ok(Safety {
            library,
            ..Safety::default()
        });
    };

    apart(folder, second)?;

    let checked = check(second, cancel, |at| progress("second", at))?;

    let ours = Manifest::open(folder)?.all()?;
    let theirs = Manifest::open(second)?.all()?;
    let only_here = ours
        .iter()
        .filter(|(path, (_, sha256))| theirs.get(*path).is_none_or(|(_, other)| other != sha256))
        .count();

    Ok(Safety {
        library,
        second: Some(checked),
        only_here,
        same_disk: same_disk(folder, second),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    static GO: AtomicBool = AtomicBool::new(false);

    fn folder(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "photo-keeper-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn put(folder: &Path, relative: &str, bytes: &[u8]) {
        let path = absolute(folder, relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }

    /// A library as an import leaves it: files, and the phone's checksums.
    fn library(name: &str) -> PathBuf {
        let path = folder(name);
        let manifest = Manifest::open(&path).unwrap();

        for (relative, bytes) in [
            ("2024/05/IMG_0001.HEIC", &b"a photo"[..]),
            ("2024/05/IMG_0002.MOV", &b"a video, rather longer"[..]),
            ("2025/01/IMG_0003.JPG", &b"another"[..]),
        ] {
            put(&path, relative, bytes);
            manifest
                .record(
                    relative,
                    bytes.len() as u64,
                    &hex(Sha256::digest(bytes)),
                    Source::Phone,
                )
                .unwrap();
        }

        path
    }

    #[test]
    fn the_checksum_is_sha256_as_everyone_else_computes_it() {
        // The published test vector for "abc" (FIPS 180-2).
        let path = folder("vector");
        put(&path, "abc.jpg", b"abc");

        assert_eq!(
            hash_file(&path.join("abc.jpg"), &GO, |_| {}).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn paths_are_written_the_same_on_every_system() {
        let path = folder("paths");
        let file = path.join("2024").join("05").join("IMG_0001.HEIC");

        assert_eq!(relative(&path, &file).unwrap(), "2024/05/IMG_0001.HEIC");
        assert_eq!(absolute(&path, "2024/05/IMG_0001.HEIC"), file);
    }

    #[test]
    fn items_are_photos_and_videos_and_nothing_else() {
        let path = library("items");
        put(&path, "notes.txt", b"not media");
        put(&path, ".IMG_0009.JPG.partial", b"half a photo");
        put(&path, ".hidden/IMG_0010.JPG", b"hidden");

        let found = items(&path).unwrap();

        assert_eq!(found.len(), 3);
        assert_eq!(found.iter().filter(|item| item.video).count(), 1);
    }

    #[test]
    fn an_untouched_library_matches() {
        let path = library("clean");

        let checked = check(&path, &GO, |_| {}).unwrap();

        assert_eq!(
            (checked.matched, checked.from_phone, checked.adopted),
            (3, 3, 0)
        );
        assert!(checked.problems.is_empty());
    }

    #[test]
    fn one_flipped_bit_is_found() {
        let path = library("bitrot");
        put(&path, "2024/05/IMG_0001.HEIC", b"a phoso");

        let checked = check(&path, &GO, |_| {}).unwrap();

        assert_eq!(checked.matched, 2);
        assert_eq!(
            checked.problems,
            [Problem {
                path: "2024/05/IMG_0001.HEIC".into(),
                reason: "changed since it was copied".into()
            }]
        );
    }

    #[test]
    fn a_changed_file_never_replaces_its_checksum() {
        let path = library("stays");
        put(&path, "2024/05/IMG_0001.HEIC", b"a phoso");

        check(&path, &GO, |_| {}).unwrap();
        let again = check(&path, &GO, |_| {}).unwrap();

        assert_eq!(again.problems.len(), 1);
    }

    #[test]
    fn a_missing_file_is_found() {
        let path = library("missing");
        fs::remove_file(absolute(&path, "2025/01/IMG_0003.JPG")).unwrap();

        let checked = check(&path, &GO, |_| {}).unwrap();

        assert_eq!(checked.matched, 2);
        assert_eq!(checked.problems[0].reason, "missing from the folder");
    }

    #[test]
    fn a_file_with_no_checksum_is_adopted_and_not_called_the_phones() {
        let path = library("adopted");
        put(&path, "old/IMG_0100.JPG", b"from before");

        let first = check(&path, &GO, |_| {}).unwrap();
        let second = check(&path, &GO, |_| {}).unwrap();

        assert_eq!((first.matched, first.adopted), (3, 1));
        assert_eq!(
            (second.matched, second.from_phone, second.adopted),
            (4, 3, 0)
        );
    }

    #[test]
    fn the_second_copy_is_the_same_bytes_dates_and_checksums() {
        let from = library("from");
        let to = folder("to");

        let mirrored = mirror(&from, &to, &GO, |_| {}).unwrap();

        assert_eq!((mirrored.copied, mirrored.already), (3, 0));
        assert!(mirrored.problems.is_empty());

        for item in items(&from).unwrap() {
            assert_eq!(
                fs::read(absolute(&from, &item.path)).unwrap(),
                fs::read(absolute(&to, &item.path)).unwrap()
            );
        }

        assert_eq!(items(&from).unwrap(), items(&to).unwrap());
        assert_eq!(
            Manifest::open(&from).unwrap().all().unwrap(),
            Manifest::open(&to).unwrap().all().unwrap()
        );
        assert_eq!(check(&to, &GO, |_| {}).unwrap().matched, 3);
    }

    #[test]
    fn a_second_run_copies_only_what_is_new() {
        let from = library("more");
        let to = folder("more-to");
        mirror(&from, &to, &GO, |_| {}).unwrap();
        put(&from, "2025/02/IMG_0004.JPG", b"new");

        let mirrored = mirror(&from, &to, &GO, |_| {}).unwrap();

        assert_eq!((mirrored.copied, mirrored.already), (1, 3));
    }

    #[test]
    fn a_damaged_original_is_not_copied() {
        let from = library("damaged");
        let to = folder("damaged-to");
        put(&from, "2024/05/IMG_0001.HEIC", b"a phoso");

        let mirrored = mirror(&from, &to, &GO, |_| {}).unwrap();

        assert_eq!(mirrored.copied, 2);
        assert_eq!(mirrored.problems.len(), 1);
        assert!(!absolute(&to, "2024/05/IMG_0001.HEIC").exists());
        assert!(!absolute(&to, "2024/05/.IMG_0001.HEIC.keeper-partial").exists());
    }

    #[test]
    fn a_different_file_in_the_second_copy_is_left_alone() {
        let from = library("differs");
        let to = folder("differs-to");
        put(&to, "2024/05/IMG_0001.HEIC", b"hers, not ours");

        let mirrored = mirror(&from, &to, &GO, |_| {}).unwrap();

        assert_eq!(mirrored.copied, 2);
        assert_eq!(
            mirrored.problems[0].reason,
            "the second copy holds a different file"
        );
        assert_eq!(
            fs::read(absolute(&to, "2024/05/IMG_0001.HEIC")).unwrap(),
            b"hers, not ours"
        );
    }

    #[test]
    fn the_second_copy_cannot_be_inside_the_library() {
        let from = library("inside");
        let to = from.join("copy");
        fs::create_dir_all(&to).unwrap();

        assert!(mirror(&from, &to, &GO, |_| {}).is_err());
        assert!(mirror(&from, &from, &GO, |_| {}).is_err());
    }

    #[test]
    fn a_second_copy_on_the_same_disk_is_said_to_be() {
        let from = library("same-disk");
        let to = folder("same-disk-to");
        mirror(&from, &to, &GO, |_| {}).unwrap();

        // Both are in the temporary folder, so on one disk.
        assert!(safety(&from, Some(&to), &GO, |_, _| {}).unwrap().same_disk);
        // The library is not a second copy of itself.
        assert!(safety(&from, Some(&from), &GO, |_, _| {}).is_err());
    }

    #[test]
    fn stopping_leaves_no_half_file() {
        let from = library("stop");
        let to = folder("stop-to");
        let stop = AtomicBool::new(true);

        let mirrored = mirror(&from, &to, &stop, |_| {}).unwrap();

        assert!(mirrored.stopped);
        assert!(items(&to).unwrap().is_empty());
    }

    #[test]
    fn safety_counts_what_only_the_library_holds() {
        let from = library("safety");
        let to = folder("safety-to");
        mirror(&from, &to, &GO, |_| {}).unwrap();
        put(&from, "2025/02/IMG_0004.JPG", b"new");

        let said = safety(&from, Some(&to), &GO, |_, _| {}).unwrap();

        assert_eq!(said.library.matched, 3);
        assert_eq!(said.library.adopted, 1);
        assert_eq!(said.second.unwrap().matched, 3);
        assert_eq!(said.only_here, 1);
    }

    #[test]
    fn safety_without_a_second_copy_says_so() {
        let from = library("alone");

        let said = safety(&from, None, &GO, |_, _| {}).unwrap();

        assert!(said.second.is_none());
        assert_eq!(said.library.matched, 3);
    }
}
