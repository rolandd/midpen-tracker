<!-- SPDX-License-Identifier: MIT -->
<!-- Copyright 2026 Roland Dreier <roland@rolandd.dev> -->

<script lang="ts">
	import '../app.css';
	import { onMount } from 'svelte';
	import { uiState } from '$lib/state.svelte';
	import { AboutModal } from '$lib/components';
	import { PUBLIC_BASE_URL } from '$env/static/public';
	import { fetchHealth } from '$lib/api';

	let { children } = $props();

	onMount(() => {
		// Remove the auth-pending class if it exists (in case the blocking script added it)
		// This ensures content becomes visible if the user stays on the page (e.g. redirect fails)
		document.documentElement.classList.remove('auth-pending');

		fetchHealth()
			.then((h) => {
				uiState.backendBuildId = h.build_id;
			})
			.catch((e) => console.debug('Failed to fetch backend health:', e));
	});
</script>

<svelte:head>
	<title>Midpen Tracker</title>

	<script>
		// Blocking script to prevent Flash of Unauthenticated Content (FOUC)
		// If the user is logged in (cookie present), hide the page body immediately
		// before it paints. The auth check in +page.svelte will then redirect.
		if (document.cookie.includes('midpen_logged_in=1')) {
			document.documentElement.classList.add('auth-pending');
		}
	</script>
	<style>
		:global(html.auth-pending body) {
			visibility: hidden;
			opacity: 0;
		}
	</style>

	<meta name="description" content="Track your Strava activities in Midpen Open Space Preserves" />

	<!-- Open Graph / Facebook -->
	<meta property="og:type" content="website" />
	<meta property="og:url" content="{PUBLIC_BASE_URL}/" />
	<meta property="og:title" content="Midpen Tracker" />
	<meta
		property="og:description"
		content="Track your Strava activities in Midpen Open Space Preserves"
	/>
	<meta property="og:image" content="{PUBLIC_BASE_URL}/card.png" />

	<!-- Twitter -->
	<meta property="twitter:card" content="summary_large_image" />
	<meta property="twitter:url" content="{PUBLIC_BASE_URL}/" />
	<meta property="twitter:title" content="Midpen Tracker" />
	<meta
		property="twitter:description"
		content="Track your Strava activities in Midpen Open Space Preserves"
	/>
	<meta property="twitter:image" content="{PUBLIC_BASE_URL}/card.png" />

	<link rel="preconnect" href="https://fonts.googleapis.com" />
	<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin="anonymous" />
	<link
		href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&display=swap"
		rel="stylesheet"
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
