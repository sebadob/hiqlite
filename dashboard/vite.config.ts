import adapter from "@sveltejs/adapter-static";
import { sveltekit } from "@sveltejs/kit/vite";
import { defineConfig } from "vite";
import wasm from "vite-plugin-wasm";

const backend = "http://127.0.0.1:8200";
const isDev = process.env.DEV_MODE === "true";

export default defineConfig({
  build: { target: "esnext" },
  plugins: [
    wasm(),
    sveltekit({
      paths: { base: "/dashboard" },
      adapter: adapter({
        fallback: undefined,
        pages: "../hiqlite/static",
        assets: "../hiqlite/static",
        precompress: true,
        strict: true,
      }),

      csp: isDev
        ? {}
        : {
            directives: {
              "default-src": ["none"],
              "connect-src": ["self"],
              "script-src": ["self", "wasm-unsafe-eval"],
              "style-src": ["self", "unsafe-inline"],
              "img-src": ["self"],
            },
          },
    }),
  ],
  server: {
    // https: {
    //     key: fs.readFileSync(`${__dirname}/../tls/key.pem`),
    //     cert: fs.readFileSync(`${__dirname}/../tls/cert-chain.pem`)
    // },
    proxy: {
      "/dashboard/api": backend,
    },
  },
});
