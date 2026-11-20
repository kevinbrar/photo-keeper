// What the library window says. The verdict is the part that matters: it is
// the one place that tells someone their photos can leave the phone.
import assert from "node:assert/strict"
import { describe, test } from "node:test"
import {
    byMonth,
    drawable,
    isHeic,
    mirrored,
    nameOf,
    percent,
    verdict,
    years,
    type Checked,
    type Item,
} from "./library.ts"

const clean = (matched: number, more: Partial<Checked> = {}): Checked => ({
    matched,
    fromPhone: matched,
    adopted: 0,
    problems: [],
    stopped: false,
    ...more,
})

const at = (year: number, month: number, day = 15): number =>
    new Date(year, month - 1, day, 12).getTime() / 1000

const item = (path: string, taken: number): Item => ({
    path,
    size: 1,
    taken,
    video: false,
})

describe("safe to delete", () => {
    test("is said when both copies match and hold the same", () => {
        const said = verdict({
            library: clean(200),
            second: clean(200),
            onlyHere: 0,
            sameDisk: false,
        })

        assert.equal(said.level, "safe")
        assert.equal(said.mayDelete, true)
        assert.equal(said.title, "200 photos and videos safe in two places")
    })

    test("is not said when both copies are on one disk", () => {
        const said = verdict({
            library: clean(200),
            second: clean(200),
            onlyHere: 0,
            sameDisk: true,
        })

        assert.equal(said.level, "samedisk")
        assert.equal(said.mayDelete, false)
    })

    test("is not said with one copy", () => {
        const said = verdict({
            library: clean(200),
            second: null,
            onlyHere: 0,
            sameDisk: false,
        })

        assert.equal(said.level, "single")
        assert.equal(said.mayDelete, false)
    })

    test("is not said while the second copy is behind", () => {
        const said = verdict({
            library: clean(200),
            second: clean(190),
            onlyHere: 10,
            sameDisk: false,
        })

        assert.equal(said.level, "behind")
        assert.equal(said.mayDelete, false)
        assert.match(said.title, /^10 photos and videos not in the second/)
    })

    test("is not said when a file in the library is wrong", () => {
        const said = verdict({
            library: clean(199, {
                problems: [{ path: "a.jpg", reason: "changed" }],
            }),
            second: clean(200),
            onlyHere: 0,
            sameDisk: false,
        })

        assert.equal(said.level, "problem")
        assert.equal(said.mayDelete, false)
        assert.equal(said.title, "1 file needs a look")
    })

    test("is not said when a file in the second copy is wrong", () => {
        const said = verdict({
            library: clean(200),
            second: clean(198, {
                problems: [
                    { path: "a.jpg", reason: "changed" },
                    { path: "b.jpg", reason: "missing" },
                ],
            }),
            onlyHere: 0,
            sameDisk: false,
        })

        assert.equal(said.mayDelete, false)
        assert.equal(said.title, "2 files need a look")
    })

    test("is not said when the check was stopped", () => {
        for (const which of ["library", "second"] as const) {
            const said = verdict({
                library: clean(200, { stopped: which === "library" }),
                second: clean(200, { stopped: which === "second" }),
                onlyHere: 0,
                sameDisk: false,
            })

            assert.equal(said.level, "stopped")
            assert.equal(said.mayDelete, false)
        }
    })

    test("is not said of an empty library", () => {
        const said = verdict({
            library: clean(0),
            second: clean(0),
            onlyHere: 0,
            sameDisk: false,
        })

        assert.equal(said.level, "empty")
        assert.equal(said.mayDelete, false)
    })

    test("counts files first seen on disk", () => {
        const said = verdict({
            library: clean(3, { adopted: 2 }),
            second: clean(5),
            onlyHere: 0,
            sameDisk: false,
        })

        assert.equal(said.title, "5 photos and videos safe in two places")
    })
})

describe("the grid", () => {
    test("groups by month, in the order given", () => {
        const groups = byMonth([
            item("c.jpg", at(2025, 1)),
            item("b.jpg", at(2024, 5, 20)),
            item("a.jpg", at(2024, 5, 2)),
        ])

        assert.deepEqual(
            groups.map((group) => [group.label, group.items.length]),
            [
                ["January 2025", 1],
                ["May 2024", 2],
            ],
        )
        assert.deepEqual(years(groups), [2025, 2024])
    })

    test("has nothing to show for nothing", () => {
        assert.deepEqual(byMonth([]), [])
    })

    test("knows a file by its name and kind", () => {
        assert.equal(nameOf("2024/05/IMG_0001.HEIC"), "IMG_0001.HEIC")
        assert.equal(nameOf("IMG_0001.HEIC"), "IMG_0001.HEIC")
        assert.equal(isHeic("2024/05/IMG_0001.HEIC"), true)
        assert.equal(isHeic("IMG_0001.jpg"), false)
        assert.equal(drawable("IMG_0001.DNG"), false)
        assert.equal(drawable("IMG_0001.mov"), true)
    })
})

describe("progress", () => {
    test("is by bytes", () => {
        const half = {
            done: 1,
            total: 10,
            bytes: 50,
            totalBytes: 100,
            name: "",
        }

        assert.equal(percent(half), 50)
        assert.equal(percent(null), 0)
        assert.equal(percent({ ...half, total: 0, totalBytes: 0 }), 100)
    })

    test("of a second copy is one sentence", () => {
        const none = {
            copied: 0,
            already: 5,
            bytes: 0,
            problems: [],
            stopped: false,
            full: false,
        }

        assert.equal(mirrored(none), "Nothing new to copy.")
        assert.equal(
            mirrored({ ...none, copied: 2, full: true }),
            "2 photos and videos copied; the second disk is full.",
        )
    })
})
