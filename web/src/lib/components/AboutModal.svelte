<script lang="ts">
	import { fade, fly } from 'svelte/transition';
	import { uiState } from '$lib/state.svelte';
	// @ts-ignore
	import Content from '$lib/assets/about.md';
	import { SocialLink } from '$lib/components';

	function close() {
		uiState.isAboutOpen = false;
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') {
			close();
		}
	}
</script>

<svelte:window onkeydown={handleKeydown} />

<div
	class="backdrop"
	transition:fade={{ duration: 200 }}
	onclick={close}
	onkeydown={(e) => {
		if (e.key === 'Escape') close();
	}}
	role="button"
	tabindex="0"
>
	<div
		class="modal"
		transition:fly={{ y: 20, duration: 300 }}
		onclick={(e) => e.stopPropagation()}
		onkeydown={(e) => e.stopPropagation()}
		role="dialog"
		aria-modal="true"
		tabindex="-1"
	>
		<button class="close-btn" onclick={close} aria-label="Close">×</button>

		<header>
			<div class="logo">🌲</div>
			<div class="header-text">
				<h2>Midpen Tracker</h2>
				<span class="badge">Unofficial Project</span>
			</div>
		</header>

		<div class="content">
			<Content />
		</div>

		<footer>
			<p>Created by Roland</p>
			<div class="social-links">
				<SocialLink platform="github" href="https://github.com/rolandd" username="rolandd" />
				<SocialLink
					platform="bluesky"
					href="https://bsky.app/profile/rbd.bsky.social"
					username="@rbd.bsky.social"
				/>
			</div>
		</footer>
	</div>
</div>

<style>
	.backdrop {
		position: fixed;
		inset: 0;
		background: rgba(0, 0, 0, 0.4);
		backdrop-filter: blur(4px);
		z-index: 100;
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 1rem;
	}

	.modal {
		background: var(--color-surface);
		width: 100%;
		max-width: 640px;
		border-radius: var(--radius);
		border: 1px solid var(--color-border);
		box-shadow:
			0 20px 25px -5px rgba(0, 0, 0, 0.1),
			0 10px 10px -5px rgba(0, 0, 0, 0.04);
		position: relative;
		overflow: hidden;
		max-height: 90vh;
		display: flex;
		flex-direction: column;
	}

	.close-btn {
		position: absolute;
		top: 0.75rem;
		right: 1rem;
		background: none;
		border: none;
		color: var(--color-text-muted);
		font-size: 1.5rem;
		line-height: 1;
		padding: 0.25rem;
		cursor: pointer;
		z-index: 10;
	}

	.close-btn:hover {
		color: var(--color-text);
	}

	header {
		padding: 1.5rem 1.5rem 1rem;
		display: flex;
		align-items: center;
		gap: 0.75rem;
		border-bottom: 1px solid var(--color-border);
		flex-shrink: 0;
	}

	.logo {
		font-size: 2rem;
	}

	.header-text h2 {
		font-size: 1.25rem;
		font-weight: 700;
		margin: 0;
		line-height: 1.2;
	}

	.badge {
		font-size: 0.65rem;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		background: #fef3c7;
		color: #92400e;
		padding: 0.1rem 0.4rem;
		border-radius: 999px;
		font-weight: 600;
		display: inline-block;
		margin-top: 0.25rem;
	}

	.content {
		padding: 1.5rem;
		overflow-y: auto;
		flex-grow: 1;
	}

	footer {
		text-align: center;
		border-top: 1px solid var(--color-border);
		padding: 1.5rem;
		background: var(--color-surface);
		flex-shrink: 0;
		z-index: 10;
	}

	footer p {
		font-size: 0.875rem;
		margin-bottom: 1rem;
		color: var(--color-text-muted);
	}

	.social-links {
		display: flex;
		justify-content: center;
		gap: 0.75rem;
		flex-wrap: wrap;
	}

	/* Markdown styles */
	:global(.content p) {
		color: var(--color-text-muted);
		font-size: 0.95rem;
		line-height: 1.6;
		margin-bottom: 1rem;
	}
	:global(.content p:first-of-type) {
		color: var(--color-text);
	}

	:global(.content a) {
		color: var(--color-primary);
		text-decoration: underline;
		text-decoration-thickness: 1px;
		text-underline-offset: 2px;
	}

	:global(.content h3) {
		font-size: 0.875rem;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		color: var(--color-text);
		margin: 1.5rem 0 0.75rem;
		font-weight: 700;
	}
	:global(.content h3:first-of-type) {
		margin-top: 0.5rem;
	}

	:global(.content li) {
		color: var(--color-text-muted);
		margin-bottom: 0.5rem;
		font-size: 0.95rem;
		line-height: 1.6;
	}

	:global(.content strong) {
		color: var(--color-text);
		font-weight: 600;
	}

	:global(.steps) {
		display: flex;
		flex-direction: column;
		gap: 0;
		margin-bottom: 1.5rem;
	}

	:global(.step) {
		display: flex;
		align-items: flex-start;
		gap: 1rem;
		position: relative;
		z-index: 1;
	}

	:global(.step-icon) {
		font-size: 1.25rem;
		background: var(--color-surface);
		border: 1px solid var(--color-border);
		width: 32px;
		height: 32px;
		display: flex;
		align-items: center;
		justify-content: center;
		border-radius: 50%;
		flex-shrink: 0;
	}

	:global(.step-text) {
		padding-bottom: 1.5rem;
	}

	:global(.step-text strong) {
		display: block;
		color: var(--color-text);
		font-size: 0.95rem;
		margin-bottom: 0.1rem;
	}

	:global(.step-text span) {
		font-size: 0.85rem;
		color: var(--color-text-muted);
	}

	:global(.step-line) {
		width: 1px;
		background: var(--color-border);
		height: 1rem;
		margin-left: 16px;
		margin-top: -0.5rem;
		margin-bottom: -0.5rem;
	}

	:global(.footer) {
		text-align: center;
		border-top: 1px solid var(--color-border);
		padding-top: 1.5rem;
		margin-top: 1.5rem;
	}

	:global(.footer p) {
		font-size: 0.875rem;
		margin-bottom: 1rem;
		color: var(--color-text-muted);
	}

	:global(.social-links) {
		display: flex;
		justify-content: center;
		gap: 0.75rem;
		flex-wrap: wrap;
	}
</style>
