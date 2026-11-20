// What the library window shows, worked out from what Rust says. Kept apart
// from the page so it can be tested without a window: `pnpm test`.

export type Item = {
    /// Inside the library folder, with `/`.
    path: string
    size: number
    /// Seconds since 1970.
    taken: number
    video: boolean
}

export type Places = {
    folder: string | null
    second: string | null
    secondThere: boolean
    importing: boolean
}

export type Progress = {
    done: number
    total: number
    bytes: number
    totalBytes: number
    name: string
}

export type Problem = { path: string; reason: string }

export type Checked = {
    matched: number
    fromPhone: number
    adopted: number
    problems: Problem[]
    stopped: boolean
}

export type Safety = {
    library: Checked
    second: Checked | null
    onlyHere: number
}

export type Mirrored = {
    copied: number
    already: number
    bytes: number
    problems: Problem[]
    stopped: boolean
    full: boolean
}

export type Group = { key: string; label: string; year: number; items: Item[] }

const MONTHS = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
]

/// The photos by the month they were taken, in the order given (Rust sends
/// newest first). Months are this computer's, as the folder names are.
export function byMonth(items: Item[]): Group[] {
    const groups: Group[] = []
    let last: Group | undefined

    for (const item of items) {
        const when = new Date(item.taken * 1000)
        const key = `${when.getFullYear()}-${String(when.getMonth() + 1).padStart(2, "0")}`

        if (last?.key !== key) {
            last = {
                key,
                label: `${MONTHS[when.getMonth()]} ${when.getFullYear()}`,
                year: when.getFullYear(),
                items: [],
            }
            groups.push(last)
        }

        last.items.push(item)
    }

    return groups
}

/// The years that have photos, newest first, for the jump menu.
export function years(groups: Group[]): number[] {
    return [...new Set(groups.map((group) => group.year))]
}

/// The file's own name, without its folders.
export function nameOf(path: string): string {
    return path.slice(path.lastIndexOf("/") + 1)
}

export function isHeic(path: string): boolean {
    return /\.(heic|heif)$/i.test(path)
}

/// Files a webview cannot draw at all: camera RAW and TIFF. They are in the
/// library and are checked like the rest; the grid shows their name.
export function drawable(path: string): boolean {
    return !/\.(dng|cr2|cr3|nef|arw|raf|orf|rw2|tif|tiff)$/i.test(path)
}

const plural = (n: number, one: string, many: string) =>
    `${n.toLocaleString("en-US")} ${n === 1 ? one : many}`

const files = (n: number) => plural(n, "photo or video", "photos and videos")

export type Verdict = {
    /// safe: two matching copies. single: one matching copy. behind: the
    /// second copy lacks some. problem: something did not match. stopped:
    /// the check did not finish. empty: nothing in the library.
    level: "safe" | "single" | "behind" | "problem" | "stopped" | "empty"
    title: string
    detail: string
    /// Whether the window may say photos can be deleted from the iPhone.
    mayDelete: boolean
}

/// What a finished check means, in the words the window shows. This is the
/// only place that decides whether "safe to delete" is said: it is said only
/// when every file matched in the library and in the second copy.
export function verdict(safety: Safety): Verdict {
    const { library, second, onlyHere } = safety
    const wrong = library.problems.length + (second?.problems.length ?? 0)
    const total = library.matched + library.adopted

    if (library.stopped || second?.stopped) {
        return {
            level: "stopped",
            title: "The check was stopped",
            detail: "Nothing was changed. Run it again to finish; do not delete from the iPhone until it has finished.",
            mayDelete: false,
        }
    }

    if (wrong > 0) {
        return {
            level: "problem",
            title: `${plural(wrong, "file needs", "files need")} a look`,
            detail: "They are different from when they were copied, or missing. Do not delete anything from the iPhone yet.",
            mayDelete: false,
        }
    }

    if (total === 0) {
        return {
            level: "empty",
            title: "No photos here yet",
            detail: "Import from the iPhone first, then run the check.",
            mayDelete: false,
        }
    }

    if (!second) {
        return {
            level: "single",
            title: `${files(total)} checked, in one place`,
            detail: "Every file matches. But this is the only copy: if this disk fails, they are gone. Make the second copy before deleting from the iPhone.",
            mayDelete: false,
        }
    }

    if (onlyHere > 0) {
        return {
            level: "behind",
            title: `${files(onlyHere)} not in the second copy yet`,
            detail: "Press Update second copy, then check again.",
            mayDelete: false,
        }
    }

    return {
        level: "safe",
        title: `${files(total)} safe in two places`,
        detail: "Every file was read back from both disks and matches. Photos the import window shows as imported can be deleted from the iPhone.",
        mayDelete: true,
    }
}

/// What the second copy did, in a sentence.
export function mirrored(done: Mirrored): string {
    const parts = [
        done.copied > 0
            ? `${files(done.copied)} copied`
            : "Nothing new to copy",
    ]

    if (done.full) parts.push("the second disk is full")
    if (done.stopped) parts.push("stopped before the end")
    if (done.problems.length > 0)
        parts.push(
            `${plural(done.problems.length, "file", "files")} could not be copied`,
        )

    return `${parts.join("; ")}.`
}

/// How far along, 0 to 100, by bytes: one long video is not one file's worth.
export function percent(at: Progress | null): number {
    if (!at) return 0
    if (at.totalBytes === 0) return at.total === 0 ? 100 : 0
    return Math.min(100, Math.floor((at.bytes / at.totalBytes) * 100))
}
