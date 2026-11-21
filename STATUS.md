# Status

## Works (checked by tests and by hand on Linux, with sample files)

- Import over USB (Rollport's, with its tests) now hashes each file and reads
  it back before it counts as imported.
- Checksums database, safety check, second copy: 16 Rust tests covering
  bit flips, missing files, damaged originals, conflicting files and stopping.
- The "safe to delete" wording: 8 tests.
- Library window: photos by month, year jump, full-size view, HEIC decoding.

## Not yet checked

- A real iPhone on a real Mac and a real Windows PC (see below).
- The Mac and Windows installers (built by pushing a version tag).
- Video thumbnails and HEIC display on Windows and macOS webviews.
- A library of 100–200 GB: speed of the check and of scrolling.

## First run with the real iPhone

1. Format the external drive exFAT. Install the app on the Mac.
2. Import with a date range of the last 7 days first (Options). Confirm the
   count matches the phone and open a few photos and a video from the folder.
3. Import everything. Note the time, and any files listed as not copied.
4. Choose a second-copy folder on another disk, Update second copy, Check now.
   Expect "safe in two places" and the file count to equal the import's.
5. Unplug the drive, plug it back in, Check now again (this read is from the
   disk for certain).
6. Independent check: in a terminal, `shasum -a 256` (Mac) or
   `Get-FileHash` (Windows) on three files, compared with their rows in
   `photo-keeper-checksums.db`.
7. Delete a handful of already-imported photos on the phone, plug in again,
   and confirm the app does not offer or lose anything.
8. Only then delete in bulk on the phone.

## Next

- Mirror `rollport-library.db` to the second copy, so the second copy can
  become the library on its own if the first drive dies. (Today the second
  copy holds every photo and its checksums, but a fresh import into it would
  not know what was already imported.)
- Albums or favourites, if she asks.

## Do the tests catch bugs?

Checked by breaking the code on purpose and running the tests against each
break. Run it again after any change under `src-tauri/`:

```sh
cd src-tauri
cargo mutants -f src/keeper.rs
cargo mutants -f src/phone.rs -F landed_whole
```

Last run: 113 breaks of the checksum, check, second-copy and read-back code,
104 caught, 3 would not compile, 6 not caught. Of `verdict()`, 17 hand-made
breaks (each way of wrongly saying "safe"), 17 caught.

The 6 not caught, and why:

- The disk-full test in the second copy (2): nothing fills a disk in a test.
- A second copy stopped in the middle of one file being reported as a failed
  file instead of "stopped" (1). Stopping between files is tested.
- The byte count shown while one file is half read (2): corrected when the
  file ends, so only the progress bar would be off for a moment.
- Same-disk detection on Windows (1): that code is not built on Linux. Check
  it by hand on the PC.

The first run caught 73 of 104. The 28 misses were closed with real tests:
progress counts, a stopped check, an unreadable file, a second copy whose
record differs, two real filesystems, and the "Open in viewer" path guard.
