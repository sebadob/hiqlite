<script lang="ts">
    import type {INode, IRaftMetrics} from "$lib/types/raft_metrics";
    import {onMount} from "svelte";
    import Metric from "$lib/components/health/Metric.svelte";
    import {fetchGet} from "$lib/utils/fetch";

    let metrics: undefined | IRaftMetrics = $state();
    let members = $derived(metrics?.membership_config.membership.configs.join(', '));

    setInterval(() => {
        fetchMetrics();
    }, 10000);

    onMount(() => {
        fetchMetrics();
    })

    async function fetchMetrics() {
        let res = await fetchGet('/metrics');
        if (res.status === 200) {
            metrics = await res.json();

        } else {
            console.error(await res.json());
        }
    }
</script>

<b>Metrics</b>

<div class="space"></div>

<Metric label="This Node">
    {metrics?.id}
    {metrics?.state}
</Metric>

<Metric label="Current Leader">
    {metrics?.current_leader}
</Metric>

<Metric label="Vote Leader">
    {metrics?.vote.leader_id.node_id}
</Metric>

<Metric label="Last Log Index">
    {metrics?.last_log_index}
</Metric>

<Metric label="Last Applied Log">
    {metrics?.last_applied?.leader_id.node_id}
    -
    {metrics?.last_applied?.leader_id.term}
    -
    {metrics?.last_applied?.index}
</Metric>

<Metric label="Last Snapshot">
    {metrics?.snapshot?.leader_id}
    -
    {metrics?.snapshot?.index}
</Metric>

<Metric label="Members">
    {members}
</Metric>

<Metric label="Millis Quorum Ack">
    {metrics?.millis_since_quorum_ack}
</Metric>

<style>
    .space {
        height: .5rem;
    }
</style>