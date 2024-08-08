import {sveltekit} from '@sveltejs/kit/vite';
import {defineConfig} from 'vite';

const backend = 'http://127.0.0.1:8200';

export default defineConfig({
    plugins: [sveltekit()],
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
