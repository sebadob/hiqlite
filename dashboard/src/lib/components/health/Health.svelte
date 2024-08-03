<script lang="ts">
    import Metrics from "$lib/components/health/Metrics.svelte";
</script>

<aside>
    <Metrics/>
</aside>

<style>
    aside {
        width: var(--width-health);
        min-width: 10rem;
        padding: .15rem .5rem 1rem .5rem;
        border-left: 1px solid var(--col-mid-a);
    }
</style>