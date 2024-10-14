<script>
    let {checked = $bindable()} = $props();

    const width = 20;
    const opacity = 0.9;
</script>

{#if checked}
    <svg
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
            stroke-width={2}
            color="hsl(var(--action))"
            {width}
            {opacity}
    >
        <path stroke-linecap="round" stroke-linejoin="round" d="M4.5 12.75l6 6 9-13.5"/>
    </svg>
{:else}
    <svg
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
            stroke-width={2}
            color="hsl(var(--error))"
            {width}
            {opacity}
    >
        <path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12"/>
    </svg>
{/if}
