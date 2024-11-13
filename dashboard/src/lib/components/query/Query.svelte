<script lang="ts">
    import {fetchPostText} from "$lib/utils/fetch";
    import type {IRow} from "$lib/types/query_results";
    import type {IQuery} from "$lib/types/query";
    import {AUTO_QUERY} from "$lib/stores/query.svelte.js";
    import ResultsDataTable from "$lib/components/query/ResultsDataTable.svelte";
    import Resizable from "$lib/components/Resizable.svelte";

    let {query}: {
        query: IQuery,
    } = $props();
    let rows: IRow[] = $state([]);

    let error = $state('');

    let innerHeight: undefined | number = $state();
    let bottomQueryInput: undefined | number = $state();
    let heightResults = $derived.by(() => {
        if (innerHeight && bottomQueryInput) {
            return `${innerHeight - bottomQueryInput}px`;
        }
        return '100%';
    });

    $effect(() => {
        if (query.query.startsWith(AUTO_QUERY)) {
            query.query = query.query.replace(`${AUTO_QUERY}\n`, '');
            execute();
        }
    });

    function onkeydown(ev: KeyboardEvent) {
        if (ev.ctrlKey && ev.code === 'Enter') {
            execute();
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
            error = Object.values(json)[0] as string;
        }
    }

    async function onResizeBottom(bottom: number) {
        bottomQueryInput = bottom;
    }
</script>

<svelte:window bind:innerHeight/>

<Resizable
        resizeBottom
        minHeightPx={100}
        initialHeightPx={300}
        {onResizeBottom}
>
    <div
            role="textbox"
            tabindex="0"
            class="query"
            bind:innerText={query.query}
            contenteditable
            {onkeydown}
    ></div>
</Resizable>

{#if error}
    <div class="err">
        {error}
    </div>
{/if}

<div
        id="query-results"
        style:height={heightResults}
        style:max-height={heightResults}
>
    <ResultsDataTable bind:rows/>
</div>

<style>
    #query-results {
        border-top: 1px solid hsla(var(--bg-high), .66);
    }

    .query {
        padding: .25rem .5rem;
        height: 100%;
        background: hsla(var(--bg-high), .2);
        overflow: auto;
    }
</style>