<script lang="ts">
    import Query from "$lib/components/query/Query.svelte";
    import Tab from "$lib/components/query/Tab.svelte";
    import IconPlus from "$lib/components/icons/IconPlus.svelte";
    import {randomKey} from "$lib/utils/randomKey";
    import {derived} from "svelte/store";
    import {AUTO_QUERY, DEFAULT_QUERY, DEFAULT_QUERY_FULL, QUERIES} from "$lib/stores/query.svelte.js";

    let tabSelected = $state(QUERIES[0].id);
    let querySelected = $derived(QUERIES.filter(q => q.id === tabSelected)[0]);

    $effect(() => {
        let last = QUERIES[QUERIES.length - 1];
        console.log(last.query);
        if (last?.query.startsWith(AUTO_QUERY)) {
            tabSelected = last.id;
        }
    });

    function addNew() {
        QUERIES.push({
            id: randomKey(6),
            query: DEFAULT_QUERY,
        });
    }

    function onClose(id: string) {
        let ids = QUERIES.map(q => q.id);
        let idx = ids.indexOf(id);
        if (tabSelected === id) {

            if (QUERIES.length === 1) {
                QUERIES.push(DEFAULT_QUERY_FULL);
                QUERIES.shift();
                tabSelected = QUERIES[0].id;
            } else if (idx === 0) {
                QUERIES.shift();
                tabSelected = QUERIES[0].id;
            } else {
                QUERIES.splice(idx, 1);
                tabSelected = QUERIES[idx - 1].id;
            }
        } else {
            QUERIES.splice(idx, 1);
        }
    }
</script>

<div id="tabs">
    {#each QUERIES as query (query.id)}
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
        display: flex;
        align-items: center;
        flex-wrap: wrap;
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
