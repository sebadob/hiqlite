<script lang="ts">
    import type {Snippet} from "svelte";

    let {
        maxHeight = "20rem",
        children,
    }: {
        maxHeight?: string;
        children: Snippet,
    } = $props();
</script>

<section style:max-height={maxHeight}>
    {@render children()}
</section>

<style>
    section {
        overflow-y: scroll;
    }
</style>