import {defineConfig} from '@playwright/test';

export default defineConfig({
    testDir: './tests',
    fullyParallel: true,
    reporter: 'list',
    use: {
        baseURL: 'http://localhost:4173/dashboard',
        headless: true,
    },
    webServer: {
        command: 'npm run build && npm run preview -- --port 4173 --strictPort',
        port: 4173,
        reuseExistingServer: !process.env.CI,
        timeout: 120_000,
    },
});
