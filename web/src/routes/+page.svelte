<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { isLoggedIn, API_BASE_URL } from '$lib/api';
	
	onMount(async () => {
		// If already logged in, go to dashboard
		if (isLoggedIn()) {
			await goto('/dashboard');
		}
	});
	
	function connectStrava() {
		// Redirect to backend OAuth start, passing our origin so backend knows where to redirect back
		const redirectUri = encodeURIComponent(window.location.origin);
		window.location.href = `${API_BASE_URL}/auth/strava?redirect_uri=${redirectUri}`;
	}
</script>

<div class="landing">
	<div class="hero">
		<div class="logo">
			<span class="logo-icon">🌲</span>
		</div>
		
		<h1>Midpen Strava Tracker</h1>
		<p class="subtitle">
			Track your activities across all 26 Midpen Open Space Preserves.
			Connect your Strava account to automatically detect which preserves you've visited.
		</p>
		
		<button class="strava-connect-btn" onclick={connectStrava}>
			<img 
				src="/btn_strava_connect_with_orange.svg" 
				alt="Connect with Strava"
				height="48"
			/>
		</button>
		
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
				We only read your activity data to detect preserves. 
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
		padding: 2rem;
	}
	
	.hero {
		max-width: 600px;
		text-align: center;
	}
	
	.logo {
		margin-bottom: 1.5rem;
	}
	
	.logo-icon {
		font-size: 4rem;
	}
	
	h1 {
		font-size: 2.5rem;
		font-weight: 700;
		margin-bottom: 0.5rem;
		background: linear-gradient(135deg, #fff 0%, #8b95a7 100%);
		-webkit-background-clip: text;
		-webkit-text-fill-color: transparent;
		background-clip: text;
		text-wrap: balance;
	}
	
	.subtitle {
		color: var(--color-text-muted);
		font-size: 1.125rem;
		margin-bottom: 1.25rem;
		line-height: 1.6;
		text-wrap: pretty;
	}
	
	.strava-connect-btn {
		background: transparent;
		border: none;
		cursor: pointer;
		padding: 0;
		transition: transform 0.2s ease, filter 0.2s ease;
	}
	
	.strava-connect-btn:hover {
		transform: translateY(-2px);
		filter: brightness(1.1);
	}
	
	.strava-connect-btn img {
		display: block;
	}
	
	.features {
		display: grid;
		grid-template-columns: repeat(3, 1fr);
		gap: 1.5rem;
		margin-top: 2rem;
	}
	
	.feature {
		padding: 1.5rem 1rem;
		background: rgba(255, 255, 255, 0.02);
		border-radius: var(--radius);
		border: 1px solid rgba(255, 255, 255, 0.05);
	}
	
	.feature-icon {
		font-size: 2rem;
		display: block;
		margin-bottom: 0.75rem;
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
		margin-top: 1.5rem;
		color: var(--color-text-muted);
		text-wrap: balance;
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
