<script lang="ts">
    import type {ITable} from "$lib/types/table";
    import {API_PREFIX} from "$lib/constants";
    import {onMount} from "svelte";
    import TableDetails from "$lib/components/tables/TableDetails.svelte";

    let tables: ITable[] = $state([]);
    let selected: undefined | ITable = $state();
    // let indexes: Table[] = $state([]);
    // let views: Table[] = $state([]);
    // let triggers: Table[] = $state([]);

    let error: undefined | Error = $state();

    onMount(() => {
        fetchTables();
    })

    async function fetchTables() {
        let res = await fetch(`${API_PREFIX}/tables`);
        if (res.status === 200) {
            tables = await res.json();
        } else {
            error = await res.json();
        }
    }

    async function select(tableName: string) {
        selected = tables.filter(t => t.name === tableName)[0];
    }
</script>

{#if error}
    <div class="err">
        {error}
    </div>
{/if}

<h3>TABLES</h3>

<div id="tables">
    <div class="tables">
        {#each tables as table (table.name)}
        <span
                role="button"
                tabindex="0"
                class="tbl-name"
                style:background={selected?.name === table.name ? 'var(--col-s)' : ''}
                onclick={() => select(table.name)}
                onkeydown={() => select(table.name)}
        >
            {table.name}
        </span>
        {/each}
    </div>

    {#if selected}
        <TableDetails table={selected}/>
    {/if}
</div>

<style>
    #tables {
        height: 100%;
        width: 18rem;
        display: flex;
        flex-direction: column;
        justify-content: space-between;
    }

    .tables {
        display: flex;
        flex-direction: column;
        overflow-y: auto;
    }

    .tbl-name {
        padding: .15rem .25rem;
        cursor: pointer;
    }

    .tbl-name:hover {
        background: var(--col-a);
    }
</style>