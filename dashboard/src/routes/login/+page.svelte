<script lang="ts">
    import ThemeSwitchAbsolute from "$lib/components/ThemeSwitchAbsolute.svelte";
    import Form from "$lib/components/form/Form.svelte";
    import Button from "$lib/components/Button.svelte";
    import InputPassword from "$lib/components/form/InputPassword.svelte";

    function onResponse(res: Response) {
        if (res.status === 200) {
            window.location.replace('/dashboard');
        }
    }
</script>

<main>
    <div class="login">
        <Form action="/dashboard/api/login" onResponse={onResponse}>
            <InputPassword
                    id="password"
                    name="password"
                    autocomplete="current-password"
                    placeholder="Password"
                    title="Valid Dashboard Password"
                    required
            />
            <Button type="submit">
                Login
            </Button>
        </Form>
    </div>
</main>

<ThemeSwitchAbsolute/>

<style>
    main {
        display: flex;
        height: 100dvh;
        justify-content: center;
        align-items: center;
    }

    .login {
        max-width: 15rem;
    }
</style>
