<script lang="ts">
    export let color = 'currentColor';
    export let opacity = 0.9;
    export let width = "1.5rem";
</script>

<svg
        fill="none"
        viewBox="0 0 24 24"
        stroke={color}
        stroke-width={2}
        {width}
        {opacity}
>
    <path
            stroke-linecap="round"
            stroke-linejoin="round"
            d="M6.75 12a.75.75 0 1 1-1.5 0 .75.75 0 0 1 1.5 0ZM12.75 12a.75.75 0 1 1-1.5 0 .75.75 0 0 1 1.5
            0ZM18.75 12a.75.75 0 1 1-1.5 0 .75.75 0 0 1 1.5 0Z"
    />
</svg>

