# For AI assistants working in this repository

Read this first, then MISSION.md and DECISIONS.md.

## Who you are helping

The person using this app is not a programmer. She plugs in her iPhone, copies
her photos to a drive, and wants to feel sure they are safe before she deletes
them from the phone. She will ask for things like "make it blue", "bigger
photos", "where is the button for the second copy", or "walk me through
importing". Answer in plain words, without jargon, and make the change for her.

Kevin (@kevinbrar) set this up and reviews anything that touches how photos are
copied or checked.

## Walking her through an import

1. Plug the iPhone into the computer with a cable, unlock it, tap **Trust**.
2. Open Photo Keeper. The small window shows how many photos are new. Press
   **Import** and wait for it to finish.
3. Press **Photos & safety**, open the **Safety check** tab.
4. If the second copy's drive is plugged in, press **Update second copy**.
5. Press **Check now** and read the result to her exactly as the app words it.
6. Only if it says **safe in two places** may she delete from the iPhone: in the
   Photos app, then in Recently Deleted. Never tell her it is safe on your own
   judgement; the app's check is the only thing that decides.

If the phone is not found: unlock it, tap Trust, try another cable or port. On
Windows the "Apple Devices" app from the Microsoft Store must be installed.

## What you may change freely

These files decide how the app looks and reads. Change them when she asks.

| What she wants                         | Where                                                         |
| -------------------------------------- | ------------------------------------------------------------- |
| Colours, photo size, spacing, corners  | `src/theme.css`                                               |
| Layout of the library window, the tabs | `src/routes/library/+page.svelte`                             |
| One photo in the grid                  | `src/lib/components/Tile.svelte`                              |
| Layout and wording of the safety page  | `src/lib/components/Safe.svelte` (not the verdict; see below) |
| Grouping (by month, by year…), sorting | `byMonth` in `src/lib/library.ts`, with a test                |
| App-wide colours and buttons           | `src/design.css`                                              |

A new view such as "birthday pictures" is a filter over the list the window
already has (path, size, date, photo or video). Build it in the window. Do not
add a database or change the Rust side for it.

## What you must not change without Kevin

- Anything in `src-tauri/` (the Rust side: talking to the iPhone, copying,
  checksums, the second copy, the check).
- `verdict()` in `src/lib/library.ts` and its tests: the one place that decides
  whether the app says photos are safe to delete.
- `.github/`, `lefthook.yml`, this file, MISSION.md and DECISIONS.md.
- Never add anything that deletes, moves, renames or overwrites a file in the
  library or the second copy, or that deletes from the iPhone.
- Never weaken, skip or delete a test or check to make a change pass.
- A test counts only if it fails when the feature is broken. For any change to
  the copy, check or verdict code, break the code on purpose and show the test
  failing (see "Do the tests catch bugs?" in STATUS.md).

If a request needs one of these, stop, explain to her in one or two sentences
why, and suggest she ask Kevin. These paths are listed in `.github/CODEOWNERS`,
so a pull request touching them needs his approval anyway.

## How to make a change

1. Work on a branch and open a pull request; do not push to `main`.
2. Before pushing, all of these must pass (the git hooks run them too):
   `pnpm format:check`, `pnpm check`, `pnpm test`, and in `src-tauri`:
   `cargo clippy --all-targets -- -D warnings` and `cargo test`.
3. Look at the change in the running app (`pnpm tauri dev`) before saying it
   is done, and show her a screenshot.
4. Keep changes small: one request, one pull request.
5. If you make a decision that will matter later, add a line to DECISIONS.md
   (new decisions only; do not edit Kevin's). Update STATUS.md when what works
   or what is left changes.
