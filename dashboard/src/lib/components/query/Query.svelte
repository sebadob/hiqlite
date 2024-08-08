<script lang="ts">
    import {fetchPostText} from "$lib/utils/fetch";
    import Results from "$lib/components/query/Results.svelte";
    import type {IRow} from "$lib/types/query_results";
    import type {IQuery} from "$lib/types/query";
    import {onMount} from "svelte";
    import {AUTO_QUERY} from "$lib/stores/query.svelte.js";

    // let {query, onUpdate}: {
    //     query: IQuery,
    //     onUpdate: (id: string, query: string) => void,
    // } = $props();
    let {query}: {
        query: IQuery,
    } = $props();
    let rows: IRow[] = $state([]);

    let error = $state('');

    $effect(() => {
        if (query.query.startsWith(AUTO_QUERY)) {
            query.query = query.query.replace(`${AUTO_QUERY}\n`, '');
            execute();
        }
    });

    function onKeyDown(ev: KeyboardEvent) {
        if (ev.ctrlKey) {
            if (ev.code === 'Enter') {
                execute();
            }
        }
    }

    async function execute() {
        rows = [];
        error = '';

        let q = [];
        for (let line of query.query.split(/\r?\n/)) {
            if (!line.startsWith('--')) {
                q.push(line);
            }
        }
        let qry = q.join('\n');

        let res = await fetchPostText('/query', qry);
        if (res.status === 200) {
            rows = await res.json();
        } else {
            let json = await res.json();
            error = JSON.stringify(json);
        }
    }
</script>

<textarea
        name="query"
        bind:value={query.query}
        onkeydown={onKeyDown}
>
</textarea>

{#if error}
    <div class="err">
        {error}
    </div>
{/if}

<Results bind:rows/>

<style>
    textarea {
        /*width: calc(100% - 20px);*/
        height: 20rem;
        padding: 10px;
        border: 1px solid var(--col-gmid);
        border-radius: 3px;
        outline: none;
        /*resize: none;*/
        resize: vertical;
        font-size: 1.1rem;
        color: var(--col-text);
        background: var(--col-bg);
        border-bottom: 1px solid var(--col-mid-a);
    }
</style>