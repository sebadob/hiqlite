<script lang="ts">
    let {label}: {
        label: string,
    } = $props();
</script>

<div class="metric">
    <div class="label font-label">{label}</div>
    <div class="font-mono">
        <slot/>
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