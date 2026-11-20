// Small pictures for the grid, made in the window from the files in the
// library and kept in the webview's own storage so each is made once. Only
// reads: nothing here writes to the library folder.
import { convertFileSrc, invoke } from "@tauri-apps/api/core"
import { drawable, isHeic, type Item } from "./library"

/// The longest side of a small picture, in pixels.
const SIDE = 360

/// How many are made at once. Decoding a HEIC is slow; more than a few and
/// the window stops answering.
const AT_ONCE = 3

let folder: string | null = null

/// The folder the items are in. Asked of Rust once per load of the grid.
export async function useFolder() {
    folder = (await invoke<{ folder: string | null }>("library_places")).folder
}

/// The address the webview loads a library file from.
export function address(item: Item): string {
    const separator = folder?.includes("\\") ? "\\" : "/"
    const path = [folder ?? "", ...item.path.split("/")].join(separator)
    return convertFileSrc(path)
}

const keyOf = (item: Item) => `${item.path}|${item.size}|${item.taken}`

let store: Promise<IDBDatabase | null> | null = null

function open(): Promise<IDBDatabase | null> {
    store ??= new Promise((resolve) => {
        const asked = indexedDB.open("photo-keeper-thumbs", 1)
        asked.onupgradeneeded = () => asked.result.createObjectStore("thumbs")
        asked.onsuccess = () => resolve(asked.result)
        // Without storage every picture is made again each time; it still works.
        asked.onerror = () => resolve(null)
    })
    return store
}

async function kept(key: string): Promise<Blob | null> {
    const db = await open()
    if (!db) return null

    return new Promise((resolve) => {
        const asked = db.transaction("thumbs").objectStore("thumbs").get(key)
        asked.onsuccess = () => resolve((asked.result as Blob) ?? null)
        asked.onerror = () => resolve(null)
    })
}

async function keep(key: string, blob: Blob) {
    const db = await open()
    db?.transaction("thumbs", "readwrite").objectStore("thumbs").put(blob, key)
}

function shrink(
    source: CanvasImageSource,
    width: number,
    height: number,
): Promise<Blob | null> {
    const scale = Math.min(1, SIDE / Math.max(width, height))
    const canvas = document.createElement("canvas")
    canvas.width = Math.max(1, Math.round(width * scale))
    canvas.height = Math.max(1, Math.round(height * scale))
    canvas
        .getContext("2d")
        ?.drawImage(source, 0, 0, canvas.width, canvas.height)

    return new Promise((resolve) => canvas.toBlob(resolve, "image/jpeg", 0.8))
}

function picture(url: string): Promise<HTMLImageElement> {
    return new Promise((resolve, reject) => {
        const image = new Image()
        image.crossOrigin = "anonymous"
        image.onload = () => resolve(image)
        image.onerror = () => reject(new Error("cannot draw the picture"))
        image.src = url
    })
}

function frame(url: string): Promise<HTMLVideoElement> {
    return new Promise((resolve, reject) => {
        const video = document.createElement("video")
        video.crossOrigin = "anonymous"
        video.muted = true
        video.preload = "metadata"
        video.onloadeddata = () => resolve(video)
        video.onerror = () => reject(new Error("cannot play the video"))
        setTimeout(() => reject(new Error("the video took too long")), 15000)
        video.src = `${url}#t=0.1`
    })
}

/// A HEIC as a JPEG the webview can draw. Safari on a Mac draws HEIC itself;
/// the webview on Windows does not.
export async function heic(url: string): Promise<Blob> {
    const { heicTo } = await import("heic-to")
    const blob = await (await fetch(url)).blob()
    return heicTo({ blob, type: "image/jpeg", quality: 0.9 })
}

async function make(item: Item): Promise<Blob | null> {
    const url = address(item)

    if (item.video) {
        const video = await frame(url)
        const made = await shrink(video, video.videoWidth, video.videoHeight)
        video.removeAttribute("src")
        video.load()
        return made
    }

    try {
        const image = await picture(url)
        return await shrink(image, image.naturalWidth, image.naturalHeight)
    } catch (e) {
        if (!isHeic(item.path)) throw e
    }

    const local = URL.createObjectURL(await heic(url))
    try {
        const image = await picture(local)
        return await shrink(image, image.naturalWidth, image.naturalHeight)
    } finally {
        URL.revokeObjectURL(local)
    }
}

let running = 0
const waiting: (() => void)[] = []

async function turn<T>(job: () => Promise<T>): Promise<T> {
    if (running >= AT_ONCE) await new Promise<void>((go) => waiting.push(go))
    running++
    try {
        return await job()
    } finally {
        running--
        waiting.shift()?.()
    }
}

/// The small picture of an item, as an address an <img> can show, or null
/// for a file the webview cannot draw: the grid shows its name instead.
export async function thumb(item: Item): Promise<string | null> {
    if (!drawable(item.path)) return null

    const key = keyOf(item)
    const had = await kept(key)
    if (had) return URL.createObjectURL(had)

    try {
        const made = await turn(() => make(item))
        if (!made) return null
        void keep(key, made)
        return URL.createObjectURL(made)
    } catch {
        return null
    }
}
