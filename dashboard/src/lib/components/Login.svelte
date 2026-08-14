<script lang="ts">
    import Form from "$lib/components/form/Form.svelte";
    import Button from "$lib/components/Button.svelte";
    import InputPassword from "$lib/components/form/InputPassword.svelte";
    import {API_PREFIX, fetchGet} from "$lib/utils/fetch";
    import {storeSession} from "$lib/stores/session";

    const action = `${API_PREFIX}/session`;

    let error = $state('');
    let isLoading = $state(false);
    let lockUntil = $state(0);
    let locked = $derived(Date.now() < lockUntil);

    async function onSubmit(form: HTMLFormElement, params: URLSearchParams) {
        if (locked) {
            // the button is disabled, but Enter can still submit; tell the user why
            const remaining = Math.max(1, Math.ceil((lockUntil - Date.now()) / 1000));
            error = `Too many failed login attempts, try again in ${remaining}s`;
            return;
        }
        error = '';
        isLoading = true;

        // The PoW WASM only runs in a secure context; over plain HTTP the proof is
        // skipped, and the server ignores it there as well (it only validates when
        // serving via TLS). The module is imported dynamically so it is never evaluated
        // on a plain-HTTP page (a static import would try to instantiate the WASM at
        // module load and crash the runtime outside a secure context).
        if (window.isSecureContext) {
            const {pow_work_wasm} = await import("../../spow/spow-wasm");
            let resPow = await fetchGet('/pow');
            if (resPow.status !== 200) {
                let resp = await resPow.json();
                error = Object.values(resp)[0] as string;
                isLoading = false;
                return;
            }

            let challenge = await resPow.text();
            let pow = await pow_work_wasm(challenge);

            if (!pow) {
                error = 'Error calculating pow';
                isLoading = false;
                return;
            }
            params.append('pow', pow);
        }

        const res = await fetch(action, {
            method: 'POST',
            headers: {
                'Content-type': 'application/x-www-form-urlencoded',
            },
            body: params,
        });

        let resp = await res.json();
        if (res.status === 200) {
            storeSession.set(resp);
        } else {
            error = Object.values(resp)[0] as string;
            // Lock the form for the global login cooldown after a 429; the duration
            // comes from the `Retry-After` header so the UI never hardcodes it.
            if (res.status === 429) {
                const retryAfter = Number(res.headers.get('Retry-After')) || 5;
                lockUntil = Date.now() + retryAfter * 1000;
                setTimeout(() => {
                    lockUntil = 0;
                }, retryAfter * 1000);
            }
        }

        isLoading = false;
    }

    // async function onResponse(res: Response) {
    //     let resp = await res.json();
    //     if (res.status === 200) {
    //         storeSession.set(resp);
    //     } else {
    //         error = Object.values(resp)[0] as string;
    //     }
    // }
</script>

<svelte:head>
    <meta property="description" content="Hiqlite Login"/>
    <title>Login</title>
</svelte:head>

<div class="container">
    <div class="login">
        <Form action={action} {onSubmit}>
            <!--        <Form action={action} onResponse={onResponse}>-->
            <InputPassword
                    id="password"
                    name="password"
                    autocomplete="current-password"
                    placeholder="Password"
                    title="Valid Dashboard Password"
                    required
            />
            <Button type="submit" level={1} {isLoading} isDisabled={locked}>
                Login
            </Button>

            {#if error}
                <div class="err">
                    {error}
                </div>
            {/if}
        </Form>
    </div>
</div>

<style>
    .container {
        width: 100dvw;
        flex: 1;
        display: flex;
        justify-content: center;
        align-items: center;
    }

    .login {
        max-width: 15rem;
        display: flex;
        height: 100dvh;
        justify-content: center;
        align-items: center;
    }
</style>
