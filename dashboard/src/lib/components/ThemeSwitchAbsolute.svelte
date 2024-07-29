<script>
    import ThemeSwitch from "$lib/components/ThemeSwitch.svelte";
</script>

<div class="theme-switch">
    <ThemeSwitch/>
</div>

<style>
    .theme-switch {
        position: absolute;
        top: .5rem;
        right: .5rem;
        z-index: 1;
    }
</style>