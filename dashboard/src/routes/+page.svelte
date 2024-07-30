<script lang="ts">
    import Query from "$lib/components/query/Query.svelte";
</script>

<svelte:head>
    <meta property="description" content="Hiqlite Dashboard"/>
    <title>Dashboard</title>
</svelte:head>

<Query query="SELECT * FROM _metadata"/>

<style>
</style>
