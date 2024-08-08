import {sveltekit} from '@sveltejs/kit/vite';
import {defineConfig} from 'vite';
import topLevelAwait from "vite-plugin-top-level-await";
import wasm from "vite-plugin-wasm";

const backend = 'http://127.0.0.1:8200';

export default defineConfig({
    plugins: [wasm(), topLevelAwait(), sveltekit()],
    server: {
        // https: {
        //     key: fs.readFileSync(`${__dirname}/../tls/key.pem`),
        //     cert: fs.readFileSync(`${__dirname}/../tls/cert-chain.pem`)
        // },
        proxy: {
            '/dashboard/api': backend,
        }
    }
});
