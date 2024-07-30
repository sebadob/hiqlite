<script lang="ts">
    import type {ITable} from "$lib/types/table";
    import {derived} from "svelte/store";

    let {table}: {
        table: ITable,
    } = $props();

    let sql = $derived(table.sql?.split('\n'));

    let isExpanded = $state(false);
</script>

<section>
    <h5 class="header">
        {table.typ} {table.tbl_name}<br/>
        {table.name}
    </h5>

    <div class="sql font-mono">
        {#each sql as s}
            {s}<br/>
        {/each}
    </div>
</section>

<style>
    section {
        padding: .15rem .25rem;
        border-top: 1px solid var(--col-mid-a);
    }

    .sql {
        display: flex;
        word-wrap: break-word;
        flex-wrap: wrap;
    }
</style>