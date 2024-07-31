<script lang="ts">
    import Query from "$lib/components/query/Query.svelte";
    import type {IQuery} from "$lib/types/query";
    import Tab from "$lib/components/query/Tab.svelte";
    import IconPlus from "$lib/components/icons/IconPlus.svelte";
    import {randomKey} from "$lib/utils/randomKey";
    import {derived} from "svelte/store";

    const defaultQuery = '-- comments will be ignored but only a single query is allowed\n' +
        '-- press CTRL + Enter to execute\n' +
        'SELECT 1';

    let queries: IQuery[] = $state([{
        id: 'SELECT 1',
        query: defaultQuery,
    }]);
    let tabSelected = $state(queries[0].id);
    let querySelected = $derived(queries.filter(q => q.id === tabSelected)[0]);

    $effect(() => {
        console.log('querySelected changed to: ' + querySelected.id);
    })

    function addNew() {
        queries.push({
            id: randomKey(6),
            query: defaultQuery,
        });
    }

    // function onUpdate(id: string, query: string) {
    //     queries = queries.map(q => {
    //         if (q.id === id) {
    //             q.query = query;
    //         }
    //         return q;
    //     });
    // }

    function onClose(id: string) {
        if (tabSelected === id) {
            let ids = queries.map(q => q.id);
            let idx = ids.indexOf(id);

            if (queries.length === 1) {
                queries = [{
                    id: 'SELECT 1',
                    query: defaultQuery,
                }];
                tabSelected = queries[0].id;
            } else if (idx === 0) {
                queries.shift();
                tabSelected = queries[0].id;
            } else {
                queries = queries.filter(q => q.id !== id);
                tabSelected = queries[idx - 1].id;
            }
        } else {
            queries = queries.filter(q => q.id !== id);
        }
    }
</script>

<div id="tabs">
    {#each queries as query (query.id)}
        <Tab bind:tab={query.id} bind:tabSelected onClose={onClose}>
            {query.id}
        </Tab>
    {/each}
    <div
            role="button"
            tabindex="0"
            title="Add New Tab"
            class="ctrl add-new"
            onclick={addNew}
            onkeydown={addNew}
    >
        <IconPlus/>
    </div>
</div>

<Query query={querySelected}/>

<style>
    #tabs {
        width: 100%;
        display: flex;
        align-items: center;
        background: var(--col-text);
    }

    .ctrl {
        cursor: pointer;
    }

    .add-new {
        margin-bottom: -4px;
        color: var(--col-ok);
    }
</style>
