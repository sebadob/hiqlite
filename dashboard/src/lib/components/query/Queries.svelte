<script lang="ts">
    import Query from "$lib/components/query/Query.svelte";

    let queries = $state(['-- comments will be ignored but only a single query is allowed\nSELECT 1']);

    $effect(() => {
        console.log('queries update: ' + queries);
    })

    function onUpdate(index: number, query: string) {
        queries[index] = query;
    }
</script>

{#each queries as query, index (index)}
    <Query query={query} index={index} onUpdate={onUpdate}/>
{/each}
