<script lang="ts">
    import { invoke } from "@tauri-apps/api/core"
    import { listen } from "@tauri-apps/api/event"
    import CircleAlert from "@lucide/svelte/icons/circle-alert"
    import ShieldCheck from "@lucide/svelte/icons/shield-check"
    import { size } from "$lib/flow"
    import {
        mirrored,
        percent,
        verdict,
        type Mirrored,
        type Places,
        type Problem,
        type Progress,
        type Safety,
    } from "$lib/library"

    // The safety check: the second copy, the check of both copies, and what
    // the result means for the iPhone. The words about safety come from
    // verdict() in src/lib/library.ts and must not be written here.
    // `loaded` changes whenever the window reads the library again.
    let {
        places,
        loaded,
        onchange,
    }: { places: Places | null; loaded: number; onchange: () => void } =
        $props()

    let working = $state<"check" | "copy" | null>(null)
    let stage = $state("")
    let at = $state<Progress | null>(null)
    let safety = $state<Safety | null>(null)
    let copied = $state<Mirrored | null>(null)
    let failed = $state("")

    const said = $derived(safety ? verdict(safety) : null)
    /// Files whose checksum is of the file on disk, not of what the phone sent.
    const unproven = $derived(
        safety
            ? safety.library.matched +
                  safety.library.adopted -
                  safety.library.fromPhone
            : 0,
    )
    const within = (where: string, list: Problem[] = []) =>
        list.map((problem) => ({ ...problem, where }))
    const problems = $derived([
        ...within("library", safety?.library.problems),
        ...within("second copy", safety?.second?.problems),
        ...within("second copy", copied?.problems),
    ])

    // A result is about the folders as they were. New photos, another folder
    // or a disk that came or went make it out of date.
    $effect(() => {
        void loaded
        safety = null
        copied = null
        failed = ""
    })

    const STAGE: Record<string, string> = {
        library: "Checking the library",
        second: "Checking the second copy",
        copy: "Copying to the second copy",
    }

    async function run(what: "check" | "copy") {
        working = what
        failed = ""
        // A check that fails must not leave the last one's answer standing.
        if (what === "check") safety = null
        at = null
        stage = what === "copy" ? "copy" : "library"

        try {
            if (what === "copy") {
                copied = await invoke<Mirrored>("make_second_copy")
                // The last check is about the copies as they were.
                safety = null
            } else {
                safety = await invoke<Safety>("check_safety")
            }
        } catch (e) {
            failed = String(e)
        }

        working = null
    }

    async function choose() {
        await invoke("choose_second_copy")
        safety = null
        copied = null
        onchange()
    }

    $effect(() => {
        const stop = listen<[string, Progress]>("keeper", (event) => {
            ;[stage, at] = event.payload
        })
        return () => void stop.then((off) => off())
    })
</script>

<div class="grow overflow-y-auto">
    <div class="mx-auto flex max-w-2xl flex-col gap-4 px-6 py-6">
        <section class="card">
            <h2>1. The second copy</h2>
            {#if places?.second}
                <p class="text-muted">
                    Kept in <span class="text-ink">{places.second}</span>
                    {#if !places.secondThere}
                        <br />
                        <span class="warn">
                            That folder is not there right now. Plug in its
                            disk, or choose another folder.
                        </span>
                    {/if}
                </p>
            {:else}
                <p class="text-muted">
                    One disk can fail. Choose a folder on a different disk, and
                    every photo is kept there as well.
                </p>
            {/if}
            <div class="flex flex-wrap gap-2">
                <button
                    type="button"
                    class="plain"
                    disabled={working !== null}
                    onclick={choose}
                >
                    {places?.second
                        ? "Choose another folder"
                        : "Choose a folder"}
                </button>
                <button
                    type="button"
                    class="main"
                    disabled={working !== null ||
                        !places?.folder ||
                        !places?.secondThere}
                    onclick={() => run("copy")}
                >
                    Update second copy
                </button>
            </div>
            {#if copied && !working}
                <p>{mirrored(copied)}</p>
            {/if}
        </section>

        <section class="card">
            <h2>2. Check that everything is safe</h2>
            <p class="text-muted">
                Reads every photo and video back from the disk and compares it
                with the record made when it was copied. Nothing is changed or
                deleted. A large library takes a while.
            </p>
            <div>
                <button
                    type="button"
                    class="main"
                    disabled={working !== null || !places?.folder}
                    onclick={() => run("check")}
                >
                    Check now
                </button>
            </div>
        </section>

        {#if working}
            <section class="card" aria-live="polite">
                <h2>{STAGE[stage] ?? "Working"}…</h2>
                <div
                    class="bar"
                    role="progressbar"
                    aria-valuenow={percent(at)}
                    aria-valuemin="0"
                    aria-valuemax="100"
                >
                    <div style:width="{percent(at)}%"></div>
                </div>
                <p class="truncate text-muted">
                    {#if at}
                        {at.done.toLocaleString("en-US")} of {at.total.toLocaleString(
                            "en-US",
                        )} · {size(at.bytes)} of {size(at.totalBytes)} · {at.name}
                    {:else}
                        Starting…
                    {/if}
                </p>
                <div>
                    <button
                        type="button"
                        class="plain"
                        onclick={() => invoke("stop_work")}
                    >
                        Stop
                    </button>
                </div>
            </section>
        {/if}

        {#if failed}
            <section class="card">
                <p class="bad">{failed}</p>
            </section>
        {/if}

        {#if said && !working}
            <section class="card verdict" data-level={said.level}>
                <h2 class="flex items-center gap-2">
                    {#if said.mayDelete}
                        <ShieldCheck size={20} aria-hidden="true" />
                    {:else}
                        <CircleAlert size={20} aria-hidden="true" />
                    {/if}
                    {said.title}
                </h2>
                <p>{said.detail}</p>
                {#if unproven > 0}
                    <p class="text-muted">
                        {unproven.toLocaleString("en-US")} of these have no record
                        from the iPhone (added by hand, or imported before records
                        were kept). They are watched for changes from now on, but
                        this check cannot say they match the iPhone.
                    </p>
                {/if}
                {#if said.mayDelete}
                    <div class="text-muted">
                        <p class="font-medium text-ink">
                            To free space on the iPhone
                        </p>
                        <ol class="ml-5 list-decimal">
                            <li>
                                Check the import window says “Up to date”. If it
                                offers photos to import, import them and run
                                this check again first.
                            </li>
                            <li>
                                On the iPhone open Photos, tap Select, choose
                                the photos, and tap the bin.
                            </li>
                            <li>
                                Then open Recently Deleted (under Utilities) and
                                delete them there too. Space is freed only after
                                this step.
                            </li>
                        </ol>
                        <p>
                            This app never deletes anything from the iPhone
                            itself.
                        </p>
                    </div>
                {/if}
            </section>
        {/if}

        {#if problems.length > 0 && !working}
            <section class="card">
                <h2>Files that need a look</h2>
                <ul class="max-h-64 overflow-y-auto select-text">
                    {#each problems as problem (problem.where + problem.path + problem.reason)}
                        <li class="py-0.5">
                            <span class="text-muted">{problem.where}:</span>
                            {problem.path}
                            <span class="text-muted">— {problem.reason}</span>
                        </li>
                    {/each}
                </ul>
            </section>
        {/if}
    </div>
</div>

<style>
    .card {
        display: flex;
        flex-direction: column;
        gap: 0.6rem;
        border: 1px solid var(--line);
        border-radius: 14px;
        background: var(--subtle);
        padding: 1rem 1.25rem;
    }

    h2 {
        font-size: 1rem;
        font-weight: 600;
    }

    button {
        border-radius: 10px;
        padding: 0.5rem 1rem;
        font-weight: 500;
    }

    button:disabled {
        opacity: 0.45;
    }

    .main {
        background: var(--keeper-accent);
        color: var(--keeper-accent-ink);
    }

    .plain {
        border: 1px solid var(--edge);
        background: var(--canvas);
    }

    .bar {
        height: 8px;
        overflow: hidden;
        border-radius: 9999px;
        background: var(--element);
    }

    .bar div {
        height: 100%;
        background: var(--keeper-accent);
        transition: width 150ms linear;
    }

    .warn {
        color: var(--keeper-warn);
    }

    .bad {
        color: var(--keeper-bad);
    }

    .verdict[data-level="safe"] h2 {
        color: var(--keeper-good);
    }

    .verdict[data-level="single"] h2,
    .verdict[data-level="behind"] h2,
    .verdict[data-level="stopped"] h2 {
        color: var(--keeper-warn);
    }

    .verdict[data-level="problem"] h2 {
        color: var(--keeper-bad);
    }
</style>
