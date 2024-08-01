<script lang="ts">
    import {ITableView} from "$lib/types/table";

    let {view, viewSelected = $bindable()}: {
        view: ITableView,
        viewSelected: ITableView
    } = $props();

    function onClick() {
        viewSelected = view;
    }
</script>

<div
        class={viewSelected === view ? 'selected' : ''}
        role="button"
        tabindex="0"
        onclick={onClick}
        onkeydown={onClick}
>
    {view}
</div>

<style>
    div {
        flex: 1;
        /*color: var(--col-tabs);*/
        color: var(--col-s-a);
        background: var(--col-tabs-bg);
        text-align: center;
        cursor: pointer;
    }

    div:hover {
        border-bottom: 2px solid var(--col-s);
    }

    .selected {
        background: var(--col-s);
        color: var(--col-tabs-sel);
        border-bottom: 2px solid var(--col-s);
    }
</style>