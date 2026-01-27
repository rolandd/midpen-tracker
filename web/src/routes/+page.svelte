<!-- SPDX-License-Identifier: MIT -->
<!-- Copyright 2026 Roland Dreier <roland@rolandd.dev> -->

<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { isLoggedIn, API_BASE_URL } from '$lib/api';

	onMount(async () => {
		if (isLoggedIn()) {
			await goto('/dashboard');
		}
	});

	function connectStrava() {
		const redirectUri = encodeURIComponent(window.location.origin);
		window.location.href = `${API_BASE_URL}/auth/strava?redirect_uri=${redirectUri}`;
	}
</script>

<div class="landing">
	<div class="hero">
		<div class="logo">
			<span class="logo-icon">🌲</span>
		</div>

		<h1>Midpen Tracker</h1>
		<p class="subtitle">
			Track your activities across all 25 Midpen Open Space Preserves. Connect your Strava account
			to automatically detect which preserves you've visited.
		</p>

		<button class="strava-connect-btn" onclick={connectStrava}>
			<img src="/btn_strava_connect_with_orange.svg" alt="Connect with Strava" height="48" />
		</button>

		<p class="legal-disclaimer">
			By connecting your Strava account, you agree to our <a href="/legal">Privacy Policy & Terms</a
			>.
		</p>

		<div class="features">
			<div class="feature">
				<span class="feature-icon">📍</span>
				<h3>Auto-detect</h3>
				<p>We automatically check your activities against preserve boundaries</p>
			</div>
			<div class="feature">
				<span class="feature-icon">🏆</span>
				<h3>Track Progress</h3>
				<p>See which preserves you've visited and catch 'em all</p>
			</div>
			<div class="feature">
				<span class="feature-icon">📝</span>
				<h3>Annotate</h3>
				<p>New activities get preserve names added to descriptions</p>
			</div>
		</div>

		<p class="privacy">
			<small>
				We only read your activity data to detect preserves.<br />
				Your data is stored securely and never shared.
			</small>
		</p>
	</div>
</div>

<style>
	.landing {
		flex: 1;
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 1.5rem 1rem;
	}

	.hero {
		max-width: 600px;
		text-align: center;
	}

	.logo {
		margin-bottom: 0.25rem;
	}

	.logo-icon {
		font-size: 2rem;
	}

	h1 {
		font-size: 2.25rem;
		font-weight: 700;
		margin-bottom: 0.25rem;
		background: linear-gradient(135deg, #fff 0%, #8b95a7 100%);
		-webkit-background-clip: text;
		-webkit-text-fill-color: transparent;
		background-clip: text;
		text-wrap: balance;
	}

	.subtitle {
		color: var(--color-text-muted);
		font-size: 1rem;
		margin-bottom: 0.75rem;
		line-height: 1.4;
		text-wrap: pretty;
	}

	.strava-connect-btn {
		background: transparent;
		border: none;
		cursor: pointer;
		padding: 0;
		transition:
			transform 0.2s ease,
			filter 0.2s ease;
	}

	.strava-connect-btn:hover {
		transform: translateY(-2px);
		filter: brightness(1.1);
	}

	.strava-connect-btn img {
		display: block;
		height: 48px;
		width: auto;
	}

	.features {
		display: grid;
		grid-template-columns: repeat(3, 1fr);
		gap: 1rem;
		margin-top: 1rem;
	}

	.feature {
		padding: 1rem 0.75rem;
		background: rgba(255, 255, 255, 0.02);
		border-radius: var(--radius);
		border: 1px solid rgba(255, 255, 255, 0.05);
		transition: all 0.2s ease;
	}

	.feature:hover {
		background: rgba(255, 255, 255, 0.05);
		transform: translateY(-2px);
		border-color: var(--color-primary);
	}

	.feature-icon {
		font-size: 1.5rem;
		display: block;
		margin-bottom: 0.5rem;
	}

	.feature h3 {
		font-size: 1rem;
		font-weight: 600;
		margin-bottom: 0.5rem;
	}

	.feature p {
		font-size: 0.875rem;
		color: var(--color-text-muted);
	}

	.privacy {
		margin-top: 1rem;
		color: var(--color-text-muted);
		text-wrap: balance;
	}

	.legal-disclaimer {
		margin-top: 0.5rem;
		font-size: 0.75rem;
		color: var(--color-text-muted);
		max-width: 300px;
		margin-left: auto;
		margin-right: auto;
	}

	.legal-disclaimer a {
		color: #e8856c;
		text-decoration: none;
		border-bottom: 1px dotted #e8856c;
		transition:
			color 0.2s,
			border-color 0.2s;
	}

	.legal-disclaimer a:hover {
		color: #fc5200;
		border-bottom-color: #fc5200;
	}

	@media (max-width: 640px) {
		h1 {
			font-size: 1.75rem;
		}

		.features {
			grid-template-columns: 1fr;
		}
	}
</style>
