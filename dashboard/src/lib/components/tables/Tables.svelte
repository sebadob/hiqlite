<script lang="ts">
    import {type ITable, ITableView} from "$lib/types/table";
    import TableDetails from "$lib/components/tables/TableDetails.svelte";
    import TableView from "$lib/components/tables/TableView.svelte";
    import {fetchGet} from "$lib/utils/fetch";

    let data: ITable[] = $state([]);
    let selectedTable: undefined | ITable = $state();
    let viewSelected = $state(ITableView.Table);
    let error: undefined | Error = $state();

    $effect(() => {
        fetchTables(viewSelected);
    })

    async function fetchTables(view: ITableView) {
        let res = await fetchGet(`/tables/${view}`);
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
    <TableView view={ITableView.Table} bind:viewSelected/>
    <TableView view={ITableView.Index} bind:viewSelected/>
    <TableView view={ITableView.Trigger} bind:viewSelected/>
    <TableView view={ITableView.View} bind:viewSelected/>
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