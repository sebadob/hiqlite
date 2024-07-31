<script lang="ts">
    import {type ITable, ITableView} from "$lib/types/table";
    import {API_PREFIX} from "$lib/constants";
    import TableDetails from "$lib/components/tables/TableDetails.svelte";
    import {get} from "$lib/fetch";
    import TableView from "$lib/components/tables/TableView.svelte";

    let data: ITable[] = $state([]);
    let selectedTable: undefined | ITable = $state();
    let selectedView = $state(ITableView.Table);
    let error: undefined | Error = $state();

    $effect(() => {
        fetchTables(selectedView);
    })

    async function fetchTables(view: ITableView) {
        let res = await get(`${API_PREFIX}/tables/${view}`);
        if (res.status === 200) {
            data = await res.json();
        } else {
            error = await res.json();
        }
    }

    async function select(tableName: string) {
        selectedTable = data.filter(t => t.name === tableName)[0];
    }
</script>

{#if error}
    <div class="err">
        {error}
    </div>
{/if}

<div class="selector">
    <TableView view={ITableView.Table} bind:selectedView/>
    <TableView view={ITableView.Index} bind:selectedView/>
    <TableView view={ITableView.Trigger} bind:selectedView/>
    <TableView view={ITableView.View} bind:selectedView/>
</div>

<div id="tables">
    <div class="tables">
        {#each data as table (table.name)}
        <span
                role="button"
                tabindex="0"
                class="tbl-name"
                style:background={selectedTable?.name === table.name ? 'var(--col-s)' : ''}
                onclick={() => select(table.name)}
                onkeydown={() => select(table.name)}
        >
            {table.name}
        </span>
        {/each}
    </div>

    {#if selectedTable}
        <TableDetails table={selectedTable}/>
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

    .selector {
        display: flex;
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