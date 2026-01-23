<script lang="ts">
	import '../app.css';
	import { uiState } from '$lib/state.svelte';
	import { AboutModal } from '$lib/components';

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
		<div class="footer-text">
			<button class="footer-link" onclick={() => (uiState.isAboutOpen = true)}>about</button>
			<span class="separator">•</span>
			<span>
				made with ❤️ in mountain view, ca by <a
					href="https://github.com/rolandd"
					target="_blank"
					rel="noopener">roland</a
				>
			</span>
			<span class="separator">•</span>
			<a href="/legal" class="footer-link">privacy & terms</a>
		</div>
		<img
			src="/api_logo_pwrdBy_strava_horiz_white.svg"
			alt="Powered by Strava"
			class="strava-logo"
		/>
	</footer>
</div>

{#if uiState.isAboutOpen}
	<AboutModal />
{/if}

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
		padding-bottom: 10rem; /* Space for fixed footer */
	}

	.site-footer {
		text-align: center;
		padding: 1rem;
		font-size: 0.8rem;
		color: #6b7280;
		letter-spacing: 0.02em;

		/* Fixed positioning */
		position: fixed;
		bottom: 0;
		left: 0;
		right: 0;
		z-index: 5;

		/* Visuals */
		background: rgba(var(--color-bg-rgb, 10, 15, 26), 0.8);
		backdrop-filter: blur(8px);
		border-top: 1px solid var(--color-border);

		/* Layout */
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 1.5rem; /* 24px */
	}

	.strava-logo {
		height: 24px;
		width: auto;
		opacity: 0.8;
		transition: opacity 0.2s;
	}

	.strava-logo:hover {
		opacity: 1;
	}

	.site-footer a,
	.footer-link {
		color: #e8856c;
		text-decoration: none;
		border-bottom: 1px dotted #e8856c;
		transition:
			color 0.2s,
			border-color 0.2s;
		background: none;
		border-top: none;
		border-left: none;
		border-right: none;
		padding: 0;
		font: inherit;
		cursor: pointer;
	}

	.site-footer a:hover,
	.footer-link:hover {
		color: #fc5200;
		border-bottom-color: #fc5200;
	}

	.separator {
		margin: 0 0.5rem;
		color: var(--color-border);
	}
</style>
