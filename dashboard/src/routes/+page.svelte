<script lang="ts">
    import {onMount} from "svelte";
    import Loading from "$lib/components/Loading.svelte";
    import ThemeSwitchAbsolute from "$lib/components/ThemeSwitchAbsolute.svelte";

    let isLoggedIn = $state(false);

    onMount(async () => {
        let res = await fetch("/dashboard/api/login");
        if (res.status === 401) {
            window.location.replace('/dashboard/login');
        } else {
            isLoggedIn = true;
        }
    });

</script>

<svelte:head>
    <meta name="robots" content="noindex nofollow"/>
    <meta property="description" content="Hiqlite Dashboard"/>
    <title>Dashboard</title>
</svelte:head>

<main>
    {#if isLoggedIn}
        <div class="content">
            <h1>Dashboard</h1>
        </div>
    {:else}
        <Loading/>
    {/if}
</main>

<ThemeSwitchAbsolute/>

<style>
    main {
        display: flex;
        height: 100dvh;
        justify-content: center;
        align-items: center;
    }

    .content {
        justify-content: flex-start;
    }
</style>
