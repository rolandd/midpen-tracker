import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig, loadEnv } from 'vite';

export default defineConfig(({ mode }) => {
	const env = loadEnv(mode, process.cwd(), '');
	return {
		plugins: [sveltekit()],
		define: {
			'import.meta.env.PUBLIC_API_URL': JSON.stringify(env.PUBLIC_API_URL),
			'import.meta.env.PUBLIC_DEMO_MODE': JSON.stringify(env.PUBLIC_DEMO_MODE)
		}
	};
});
