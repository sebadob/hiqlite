<script lang="ts">
    import {API_PREFIX} from "$lib/constants";
    import {postText} from "$lib/fetch";
    import Results from "$lib/components/query/Results.svelte";
    import type {IRow} from "$lib/types/query_results";

    let {query = $bindable()}: { query: string } = $props();
    let rows: IRow[] = $state([]);

    function onKeyDown(ev: KeyboardEvent) {
        if (ev.ctrlKey && ev.code === 'Enter') {
            execute();
        }
    }

    async function execute() {
        rows = [];

        let res = await postText(`${API_PREFIX}/query`, query);
        if (res.status === 200) {
            rows = await res.json();
        } else {
            console.error(await res.json());
        }
    }
</script>

<textarea
        name="query"
        bind:value={query}
        onkeydown={onKeyDown}
>
</textarea>

<Results bind:rows/>

<style>
    textarea {
        width: calc(100% - 20px);
        height: 20rem;
        padding: 10px;
        border: 1px solid var(--col-gmid);
        border-radius: 3px;
        outline: none;
        resize: none;
        font-size: 1.1rem;
        color: var(--col-text);
        background: var(--col-bg);
        border-bottom: 1px solid var(--col-mid-a);
    }

    textarea:focus {
        resize: none;
    }
</style>