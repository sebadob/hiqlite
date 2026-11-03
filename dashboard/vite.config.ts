import adapter from "@sveltejs/adapter-static";
import { sveltekit } from "@sveltejs/kit/vite";
import { defineConfig, type Plugin } from "vite";
import wasm from "vite-plugin-wasm";

const backend = "https://127.0.0.1:8200";
const isDev = process.env.DEV_MODE === "true";
const base = "/dashboard";

// The server 301s `/dashboard` to `/dashboard/index.html`, which only exists as a
// prerendered file. In dev the app is served at `/dashboard/`, so the production URL
// has to be bounced there, otherwise SvelteKit renders its 404 page.
function devIndexRedirect(): Plugin {
  return {
    name: "hiqlite-dev-index-redirect",
    configureServer(server) {
      server.middlewares.use((req, res, next) => {
        const [path, query] = ((req as { url?: string }).url ?? "").split("?");
        if (path !== `${base}/index.html`) {
          return next();
        }
        res.writeHead(307, { Location: `${base}/${query ? `?${query}` : ""}` });
        res.end();
      });
    },
  };
}

export default defineConfig({
  build: { target: "esnext" },
  plugins: [
    devIndexRedirect(),
    wasm(),
    sveltekit({
      paths: { base },
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
      // The API listens on TLS (`tls_auto_certificates`), and dev certificates are
      // auto-generated and self-signed, hence `secure: false`.
      "/dashboard/api": {
        secure: false,
        target: backend,
      },
    },
  },
});
