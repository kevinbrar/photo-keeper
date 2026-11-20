<script lang="ts">
    import { invoke } from "@tauri-apps/api/core"
    import { listen } from "@tauri-apps/api/event"
    import ChevronLeft from "@lucide/svelte/icons/chevron-left"
    import ChevronRight from "@lucide/svelte/icons/chevron-right"
    import X from "@lucide/svelte/icons/x"
    import "../../theme.css"
    import Safe from "$lib/components/Safe.svelte"
    import Tile from "$lib/components/Tile.svelte"
    import { size } from "$lib/flow"
    import {
        byMonth,
        drawable,
        isHeic,
        nameOf,
        years,
        type Item,
        type Places,
    } from "$lib/library"
    import { address, heic, useFolder } from "$lib/thumbs"

    // The library window: the photos by month, and the safety check. How it
    // looks is in src/theme.css; what it says about safety is decided in
    // src/lib/library.ts.
    let tab = $state<"photos" | "safety">("photos")
    let places = $state<Places | null>(null)
    let items = $state<Item[]>([])
    let failed = $state("")
    let loading = $state(true)

    const groups = $derived(byMonth(items))
    const total = $derived(items.reduce((sum, item) => sum + item.size, 0))

    /// The photo shown large, by its place in `items`.
    let open = $state<number | null>(null)
    const shown = $derived(open === null ? null : (items[open] ?? null))
    /// A HEIC made drawable for this webview, when it cannot draw it itself.
    let converted = $state<string | null>(null)
    let undrawable = $state(false)

    async function load() {
        loading = true
        failed = ""
        try {
            await useFolder()
            places = await invoke<Places>("library_places")
            items = await invoke<Item[]>("library_items")
        } catch (e) {
            failed = String(e)
        }
        loading = false
    }

    function show(index: number | null) {
        if (converted) URL.revokeObjectURL(converted)
        converted = null
        undrawable = false
        open = index
    }

    function step(by: number) {
        if (open === null) return
        const next = open + by
        if (next >= 0 && next < items.length) show(next)
    }

    /// The webview could not draw the photo. A HEIC is converted; anything
    /// else is offered to the system's viewer.
    async function cannotDraw(item: Item) {
        if (!isHeic(item.path) || converted) {
            undrawable = true
            return
        }
        try {
            const made = URL.createObjectURL(await heic(address(item)))
            if (shown === item) converted = made
            else URL.revokeObjectURL(made)
        } catch {
            undrawable = true
        }
    }

    function keys(event: KeyboardEvent) {
        if (open === null) return
        if (event.key === "Escape") show(null)
        if (event.key === "ArrowLeft") step(-1)
        if (event.key === "ArrowRight") step(1)
    }

    function jump(year: string) {
        document
            .querySelector(`[data-year="${year}"]`)
            ?.scrollIntoView({ block: "start" })
    }

    const day = (taken: number) =>
        new Date(taken * 1000).toLocaleString(undefined, {
            dateStyle: "long",
            timeStyle: "short",
        })

    $effect(() => {
        void load()

        // An import that ends has put new photos in the folder.
        let was = false
        const stop = listen<{ run: { run: string } | null }>(
            "flow",
            (event) => {
                const copying = event.payload.run?.run === "copying"
                if (was && !copying) void load()
                was = copying
            },
        )

        return () => void stop.then((off) => off())
    })
</script>

<svelte:window onkeydown={keys} />

<main class="flex h-full flex-col text-sm">
    <header
        class="flex shrink-0 items-center gap-3 border-b border-line bg-subtle px-4 py-2.5"
    >
        <div role="tablist" class="flex gap-1 rounded-xl bg-element p-1">
            {#each [["photos", "Photos"], ["safety", "Safety check"]] as [id, label] (id)}
                <button
                    type="button"
                    role="tab"
                    aria-selected={tab === id}
                    onclick={() => (tab = id as typeof tab)}
                    class="tab rounded-lg px-4 py-1.5 font-medium"
                >
                    {label}
                </button>
            {/each}
        </div>

        <p class="grow truncate text-muted">
            {#if places?.folder}
                {items.length.toLocaleString("en-US")} photos and videos, {size(
                    total,
                )} · {places.folder}
            {/if}
        </p>

        {#if tab === "photos" && groups.length > 0}
            <select
                aria-label="Jump to a year"
                onchange={(event) => jump(event.currentTarget.value)}
                class="rounded-lg border border-line bg-canvas px-2 py-1.5"
            >
                {#each years(groups) as year (year)}
                    <option value={year}>{year}</option>
                {/each}
            </select>
        {/if}
    </header>

    {#if tab === "safety"}
        <Safe {places} onchange={load} />
    {:else}
        <div class="grow overflow-y-auto px-4 pb-6">
            {#if failed}
                <p class="mt-16 text-center text-muted">
                    The folder could not be read: {failed}
                </p>
            {:else if loading && items.length === 0}
                <p class="mt-16 text-center text-muted">Looking…</p>
            {:else if !places?.folder}
                <p class="mt-16 text-center text-muted">
                    Choose a folder for the photos in the import window first.
                </p>
            {:else if items.length === 0}
                <p class="mt-16 text-center text-muted">
                    No photos here yet. Plug in the iPhone and press Import in
                    the other window.
                </p>
            {/if}

            {#each groups as group, at (group.key)}
                <section
                    class="month"
                    data-year={groups[at - 1]?.year === group.year
                        ? undefined
                        : group.year}
                >
                    <h2
                        class="sticky top-0 z-10 bg-canvas py-3 text-base font-semibold"
                    >
                        {group.label}
                        <span class="ml-1 text-sm font-normal text-muted">
                            {group.items.length.toLocaleString("en-US")}
                        </span>
                    </h2>
                    <div class="grid-of-photos">
                        {#each group.items as item (item.path)}
                            <Tile
                                {item}
                                onopen={() => show(items.indexOf(item))}
                            />
                        {/each}
                    </div>
                </section>
            {/each}
        </div>
    {/if}
</main>

{#if shown}
    <div
        role="dialog"
        aria-modal="true"
        aria-label={nameOf(shown.path)}
        class="fixed inset-0 z-50 flex flex-col bg-black/90 text-white"
    >
        <div class="flex shrink-0 items-center gap-3 px-4 py-3">
            <p class="grow truncate">
                {nameOf(shown.path)}
                <span class="ml-2 text-white/60">
                    {day(shown.taken)} · {size(shown.size)}
                </span>
            </p>
            <button
                type="button"
                class="rounded-lg bg-white/15 px-3 py-1.5 hover:bg-white/25"
                onclick={() => invoke("open_item", { path: shown.path })}
            >
                Open in viewer
            </button>
            <button
                type="button"
                aria-label="Close"
                class="rounded-lg bg-white/15 p-1.5 hover:bg-white/25"
                onclick={() => show(null)}
            >
                <X size={18} aria-hidden="true" />
            </button>
        </div>

        <div class="relative flex min-h-0 grow items-center justify-center">
            {#key shown.path}
                {#if undrawable || !drawable(shown.path)}
                    <p class="text-white/70">
                        This file cannot be shown here. Press Open in viewer.
                    </p>
                {:else if shown.video}
                    <!-- svelte-ignore a11y_media_has_caption -->
                    <video
                        src={address(shown)}
                        controls
                        autoplay
                        class="max-h-full max-w-full"
                        onerror={() => (undrawable = true)}
                    ></video>
                {:else}
                    <img
                        src={converted ?? address(shown)}
                        alt={nameOf(shown.path)}
                        class="max-h-full max-w-full object-contain"
                        onerror={() => cannotDraw(shown)}
                    />
                {/if}
            {/key}

            <button
                type="button"
                aria-label="Newer"
                disabled={open === 0}
                class="arrow left-3"
                onclick={() => step(-1)}
            >
                <ChevronLeft size={24} aria-hidden="true" />
            </button>
            <button
                type="button"
                aria-label="Older"
                disabled={open === items.length - 1}
                class="arrow right-3"
                onclick={() => step(1)}
            >
                <ChevronRight size={24} aria-hidden="true" />
            </button>
        </div>
    </div>
{/if}

<style>
    .tab[aria-selected="true"] {
        background: var(--keeper-accent);
        color: var(--keeper-accent-ink);
    }

    /* Months off the screen are not laid out until they are scrolled to: a
       library of tens of thousands of photos stays quick. */
    .month {
        content-visibility: auto;
        contain-intrinsic-size: auto 600px;
    }

    .grid-of-photos {
        display: grid;
        grid-template-columns: repeat(
            auto-fill,
            minmax(var(--keeper-tile), 1fr)
        );
        gap: var(--keeper-gap);
    }

    .arrow {
        position: absolute;
        top: 50%;
        translate: 0 -50%;
        border-radius: 9999px;
        background: rgb(255 255 255 / 0.15);
        padding: 0.5rem;
    }

    .arrow:hover:not(:disabled) {
        background: rgb(255 255 255 / 0.3);
    }

    .arrow:disabled {
        opacity: 0.25;
    }
</style>
