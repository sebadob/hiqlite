<script lang="ts">
    import {crossfade} from "svelte/transition";
    import {cubicInOut} from 'svelte/easing';

    const storageIdx = 'darkMode';
    const [send, receive] = crossfade({
        duration: 250,
        easing: cubicInOut
    });

    let mode = $state({
        isInitialized: false,
        darkMode: false,
    });

    $effect(() => {
        if (!mode.isInitialized) {
            mode.darkMode = isDarkMode();
            mode.isInitialized = true;
        } else {
            toggleColorScheme(mode.darkMode);
        }
    });

    function isDarkMode() {
        const darkMode = window?.localStorage?.getItem(storageIdx);
        if (darkMode) {
            return "true" === darkMode;
        }

        return window?.matchMedia("(prefers-color-scheme: dark)")?.matches;
    }

    function toggleColorScheme(darkMode: boolean) {
        if (darkMode) {
            document.body.classList.remove("light-theme");
            document.body.classList.add("dark-theme");
        } else {
            document.body.classList.remove("dark-theme");
            document.body.classList.add("light-theme");
        }

        localStorage.setItem(storageIdx, darkMode.toString());
    }

    function toggle() {
        mode.darkMode = !mode.darkMode
    }
</script>

<div
        role="button"
        tabindex="0"
        class="icon"
        onclick={toggle}
        onkeydown={toggle}
>
    {#if mode.darkMode}
        <div class="moon" in:receive={{key: "dark", delay: 200}} out:send={{key: "light"}}>
            <svg
                    xmlns="http://www.w3.org/2000/svg"
                    fill="none"
                    viewBox="0 0 24 24"
                    stroke-width="2"
                    stroke="currentColor"
                    class="w-6 h-6"
            >
                <path
                        stroke-linecap="round"
                        stroke-linejoin="round"
                        d="M21.752 15.002A9.718 9.718 0 0118 15.75c-5.385 0-9.75-4.365-9.75-9.75
                        0-1.33.266-2.597.748-3.752A9.753 9.753 0 003 11.25C3 16.635 7.365 21 12.75
                        21a9.753 9.753 0 009.002-5.998z"
                />
            </svg>
        </div>
    {:else}
        <div class="sun" in:receive={{key: "light", delay: 200}} out:send={{key: "dark"}}>
            <svg
                    xmlns="http://www.w3.org/2000/svg"
                    fill="none"
                    viewBox="0 0 24 24"
                    stroke-width="2"
                    stroke="currentColor"
                    class="w-6 h-6"
            >
                <path
                        stroke-linecap="round"
                        stroke-linejoin="round"
                        d="M12 3v2.25m6.364.386l-1.591 1.591M21 12h-2.25m-.386 6.364l-1.591-1.591M12
                        18.75V21m-4.773-4.227l-1.591 1.591M5.25 12H3m4.227-4.773L5.636 5.636M15.75 12a3.75
                        3.75 0 11-7.5 0 3.75 3.75 0 017.5 0z"
                />
            </svg>
        </div>
    {/if}
</div>

<style>
    .icon {
        margin-bottom: -.35rem;
        width: 1.5rem;
        aspect-ratio: 1;
        cursor: pointer;
    }

    .sun {
        color: #969605;
    }

    .moon {
        color: #6f6fdb;
    }
</style>
