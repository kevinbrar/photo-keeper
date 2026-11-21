<script lang="ts">
    import Film from "@lucide/svelte/icons/film"
    import { nameOf, type Item } from "$lib/library"
    import { thumb } from "$lib/thumbs"

    // One photo or video in the grid. Its small picture is made only once it
    // scrolls near the window.
    let { item, onopen }: { item: Item; onopen: () => void } = $props()

    let url = $state<string | null>(null)
    let tried = $state(false)

    function near(node: HTMLElement) {
        const watcher = new IntersectionObserver(
            (seen) => {
                if (!seen.some((entry) => entry.isIntersecting)) return
                watcher.disconnect()
                void thumb(item).then((made) => {
                    url = made
                    tried = true
                })
            },
            { rootMargin: "600px" },
        )
        watcher.observe(node)

        return () => {
            watcher.disconnect()
            if (url) URL.revokeObjectURL(url)
        }
    }
</script>

<button
    type="button"
    {@attach near}
    onclick={onopen}
    title={nameOf(item.path)}
    aria-label={nameOf(item.path)}
    class="tile relative overflow-hidden bg-element"
>
    {#if url}
        <img
            src={url}
            alt=""
            draggable="false"
            class="size-full object-cover"
        />
    {:else if tried}
        <span
            class="grid size-full place-items-center p-2 text-center text-xs break-all text-muted"
        >
            {nameOf(item.path)}
        </span>
    {/if}
    {#if item.video}
        <span
            class="absolute right-1.5 bottom-1.5 rounded-full bg-black/55 p-1 text-white"
        >
            <Film size={12} aria-hidden="true" />
        </span>
    {/if}
</button>

<style>
    .tile {
        aspect-ratio: 1;
        border-radius: var(--keeper-radius);
    }

    .tile:hover {
        outline: 2px solid var(--keeper-accent);
        outline-offset: -2px;
    }
</style>
