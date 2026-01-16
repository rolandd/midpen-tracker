<script lang="ts">
	import '../app.css';

	let { children } = $props();
</script>

<svelte:head>
	<meta charset="utf-8" />
	<meta name="viewport" content="width=device-width, initial-scale=1" />
	<title>Midpen Strava Tracker</title>
	<meta name="description" content="Track your Strava activities in Midpen Open Space Preserves" />
	<link rel="preconnect" href="https://fonts.googleapis.com" />
	<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin="anonymous" />
	<link
		href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&display=swap"
		rel="stylesheet"
	/>

	<!-- Content Security Policy -->
	<!-- Added unsafe-inline for SvelteKit hydration and localhost:8080 for local API -->
	<meta
		http-equiv="content-security-policy"
		content="default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline' https://fonts.googleapis.com; font-src 'self' https://fonts.gstatic.com; img-src 'self' data: https:; connect-src 'self' https://*.run.app http://localhost:8080; frame-ancestors 'none';"
	/>
</svelte:head>

<div class="app-wrapper">
	<main class="app-content">
		{@render children()}
	</main>

	<footer class="site-footer">
		made with ❤️ in mountain view, ca by <a
			href="https://github.com/rolandd"
			target="_blank"
			rel="noopener">roland</a
		>
	</footer>
</div>

<style>
	.app-wrapper {
		min-height: 100vh;
		min-height: 100dvh;
		display: flex;
		flex-direction: column;
	}

	.app-content {
		flex: 1;
		display: flex;
		flex-direction: column;
	}

	.site-footer {
		text-align: center;
		padding: 1.5rem 1rem;
		font-size: 0.8rem;
		color: #6b7280;
		letter-spacing: 0.02em;
	}

	.site-footer a {
		color: #e8856c;
		text-decoration: none;
		border-bottom: 1px dotted #e8856c;
		transition:
			color 0.2s,
			border-color 0.2s;
	}

	.site-footer a:hover {
		color: #fc5200;
		border-bottom-color: #fc5200;
	}
</style>
