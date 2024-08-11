import adapter from '@sveltejs/adapter-static';

const isDev = process.env.DEV_MODE === 'true';

/** @type {import('@sveltejs/kit').Config} */
const config = {
    kit: {
        paths: {
            base: '/dashboard',
        },
        adapter: adapter({
            fallback: null,
            pages: '../hiqlite/static',
            assets: '../hiqlite/static',
            precompress: true,
            strict: true,
        }),
        csp: isDev ? {} : {
            directives: {
                'default-src': ['none'],
                'connect-src': ['self'],
                'script-src': ['self', 'wasm-unsafe-eval'],
                'style-src': ['self', 'unsafe-inline'],
                'img-src': ['self'],
            },
        },
    }
};

export default config;
