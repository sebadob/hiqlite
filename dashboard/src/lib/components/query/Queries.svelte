<script lang="ts">
    import Query from "$lib/components/query/Query.svelte";

    let queries = $state(['SELECT 1']);

    function onUpdate(index: number, query: string) {
        console.log(index + query);
    }
</script>

{#each queries as query}
    <Query query={query}/>
{/each}

<!--<Query query="SELECT * FROM _metadata"/>-->