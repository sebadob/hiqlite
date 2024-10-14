<script lang="ts">
    import type {Snippet} from "svelte";

    let {label, children}: {
        label: string,
        children: Snippet,
    } = $props();
</script>

<div class="metric">
    <div class="label font-label">{label}</div>
    <div class="font-mono">
        {@render children()}
    </div>
</div>

<style>
    .label {
        font-size: .9rem;
    }

    .metric {
        margin: .25rem .1rem;
    }
</style>