# Decisions

Newest at the bottom. Each one says what was decided and why, so nobody has to
guess later. Add to it; do not rewrite history.

1. **Built on Rollport, not from scratch.** Rollport (MIT) already imports from
   an iPhone over USB on Mac and Windows, copies to a scratch file, syncs,
   checks the size and renames into place, and has its own tests. The upstream
   history is kept (remote `upstream`) so its fixes can be merged.

2. **Cable only, nothing always on.** She wants to see the photos move. No
   server, no Wi-Fi backup.

3. **The app never deletes from the iPhone.** Removing files from the phone's
   DCIM folder over USB leaves the Photos database pointing at files that are
   gone. Deleting is done by her, in the Photos app.

4. **Every imported file is hashed twice.** SHA-256 of the bytes as the iPhone
   sends them, then SHA-256 of the file read back from the disk before it gets
   its name. A mismatch fails that file and it is tried again. The hash is kept
   in `photo-keeper-checksums.db` in the library folder, marked `phone`.
   Limit: the read-back right after writing can be served from the computer's
   memory rather than the disk. The Safety check, run later, reads from the
   disk itself; that is why it exists as a separate step.

5. **A checksum is never replaced by the file it describes.** A file found
   with no checksum is given one marked `disk` (the app says it cannot vouch
   that these match the iPhone). After that a file that differs from its
   checksum is a problem to report, never a reason to update the checksum.

6. **The second copy only adds.** It copies what is missing, verifies each file
   by reading it back, and never replaces or removes a file already there. A
   file that differs is reported and left alone. It must be a different folder,
   not inside the library.

7. **"Safe to delete" is decided in one function**, `verdict()` in
   `src/lib/library.ts`, and only when both copies matched in full and the
   second copy holds everything. One copy is never "safe".

8. **Separate checksum database.** Rollport's `rollport-library.db` is left
   exactly as upstream writes it, so upstream changes merge cleanly.

9. **The library window reads the folder, not a database.** The gallery lists
   the media files on disk and groups them by the file date (which the import
   sets to when the photo was taken). Nothing to get out of sync, and it works
   on either computer the drive is plugged into.

10. **Thumbnails are made in the window and cached in the webview's storage**,
    never written to the library. HEIC photos are decoded with `heic-to`
    (libheif, LGPL-3.0, loaded as its own module) where the webview cannot
    draw them (Windows).

11. **Guard rails for the assistant are in the repository**: AGENTS.md says
    what may change; `.github/CODEOWNERS` makes changes to the copy and check
    code need Kevin's review; CI runs every test on every pull request.

12. **Real-iPhone testing is manual.** CI has no iPhone. Before the first real
    use, and after any change under `src-tauri/`, run the checklist in
    STATUS.md with a real phone.

13. **Installers are unsigned.** Signing needs Apple's and Microsoft's paid
    certificates. For one household the first-open warning is acceptable.
