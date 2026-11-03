// See https://kit.svelte.dev/docs/types#app
// for information about these interfaces
declare global {
	namespace App {
		// interface Error {}
		// interface Locals {}
		// interface PageData {}
		// interface PageState {}
		// interface Platform {}
	}

	// SvelteKit 3 type-checks `vite.config.ts` and `playwright.config.ts`, which read
	// `process.env`. Instead of adding `@types/node` we declare only what they use.
	const process: {
		env: Record<string, string | undefined>;
	};
}

export {};
