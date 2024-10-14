<script lang="ts">
    import Metrics from "$lib/components/health/Metrics.svelte";
</script>

<aside>
    <Metrics/>
</aside>

<style>
    aside {
        width: 12rem;
        min-width: 12rem;
        padding: .15rem .5rem 1rem .5rem;
        border-left: 1px solid hsl(var(--bg-high));
    }
</style>