<script lang="ts">
    export let color = 'currentColor';
    export let opacity = 0.9;
    export let width = "1.5rem";
</script>

<svg
        fill="none"
        viewBox="0 0 24 24"
        stroke="currentColor"
        stroke-width={2}
        {width}
        {color}
        {opacity}
>
    <path
            stroke-linecap="round"
            stroke-linejoin="round"
            d="m19.5 8.25-7.5 7.5-7.5-7.5"
    />
</svg>
