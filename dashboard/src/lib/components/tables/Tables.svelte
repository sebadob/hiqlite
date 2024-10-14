<script lang="ts">
    import {type ITable, ITableView} from "$lib/types/table";
    import TableDetails from "$lib/components/tables/TableDetails.svelte";
    import TableView from "$lib/components/tables/TableView.svelte";
    import {fetchGet} from "$lib/utils/fetch";
    import IconDocText from "$lib/components/icons/IconDocText.svelte";
    import {AUTO_QUERY, QUERIES} from "$lib/stores/query.svelte.js";
    import type {IQuery} from "$lib/types/query";
    import {genKey} from "$lib/utils/genKey";
    import Resizable from "$lib/components/Resizable.svelte";

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

    function select(tableName: string) {
        selectedTable = data.filter(t => t.name === tableName)[0];
    }

    function addInfoQuery(tableName: string) {
        let query: IQuery = {
            id: `${tableName}_${genKey(4)}`,
            query: `${AUTO_QUERY}\nPRAGMA table_info(${tableName})`,
        };
        QUERIES.push(query);
        select(tableName);
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

<Resizable
        resizeBottom
        initialHeightPx={window ? window.innerHeight - 400 : 600}
        minHeightPx={120}
>
    {#each data as table (table.name)}
        <div
                role="button"
                tabindex="0"
                class={selectedTable?.name === table.name ? 'entry selected' : 'entry'}
                onclick={() => select(table.name)}
                onkeydown={() => select(table.name)}
        >
            <div>
                {table.name}
            </div>
            {#if table.typ === 'table'}
                <div
                        role="button"
                        tabindex="0"
                        class="btn"
                        onclick={() => addInfoQuery(table.name)}
                        onkeydown={() => addInfoQuery(table.name)}
                >
                    <IconDocText/>
                </div>
            {/if}
        </div>
    {/each}
</Resizable>

{#if selectedTable}
    <TableDetails table={selectedTable}/>
{/if}

<style>
    .btn {
        color: var(--col-mid);
        cursor: pointer;
    }

    .entry {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: .15rem .25rem;
        cursor: pointer;
        transition: all 150ms;
    }

    .entry:hover {
        background: hsl(var(--action));
        color: hsl(var(--bg));
    }

    .selected {
        background: hsl(var(--action));
        color: hsl(var(--bg));
    }

    .selector {
        display: flex;
    }
</style>