import {expect, test} from '@playwright/test';

// Smoke test for the embedded dashboard login page: the page must build, hydrate,
// and render without uncaught JavaScript errors. The preview server has no Rust
// backend, so API calls fail with 502 - only page errors (hydration / WASM
// instantiation failures) are fatal here.
test('login page loads, hydrates, and renders without JS errors', async ({page}) => {
    const pageErrors: string[] = [];
    page.on('pageerror', (err) => pageErrors.push(err.message));

    const resp = await page.goto('/');
    expect(resp?.status()).toBe(200);

    await expect(page).toHaveTitle('Login');
    await expect(page.locator('input[type="password"]')).toBeVisible();
    await expect(page.locator('button[type="submit"]')).toHaveText('Login');

    expect(pageErrors).toEqual([]);
});
