<script lang="ts">
    import type {ITable} from "$lib/types/table";
    import {derived} from "svelte/store";

    let {table}: {
        table: ITable,
    } = $props();

    let sql = $derived(table.sql?.split('\n'));
</script>

<section>
    <h5 class="header">
        {table.name}<br/>
        {table.typ}: {table.tbl_name}
    </h5>

    <div class="sql font-mono">
        {#each sql as s}
            {s}<br/>
        {/each}
    </div>
</section>

<style>
    section {
        flex: 1;
        padding: .15rem .25rem;
        border-top: 1px solid hsl(var(--bg-high));
        overflow-y: auto;
    }

    .sql {
        display: flex;
        word-wrap: break-word;
        flex-wrap: wrap;
    }
</style>