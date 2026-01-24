<script lang="ts">
	let {
		platform,
		href,
		username
	}: { platform: 'github' | 'bluesky'; href: string; username: string } = $props();
</script>

<a {href} target="_blank" rel="noopener" class="social-btn {platform}">
	{#if platform === 'github'}
		<svg
			height="20"
			viewBox="0 0 16 16"
			version="1.1"
			width="20"
			aria-hidden="true"
			fill="currentColor"
		>
			<path
				d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0016 8c0-4.42-3.58-8-8-8z"
			></path>
		</svg>
	{:else if platform === 'bluesky'}
		<svg width="20" height="20" viewBox="0 0 600 530" fill="currentColor">
			<path
				d="m135.72 44.03c66.496 49.921 138.02 151.14 164.28 205.46 26.262-54.316 97.782-155.54 164.28-205.46 47.98-36.021 125.72-63.892 125.72 24.795 0 17.712-10.155 148.79-16.111 170.07-20.703 73.984-96.144 92.854-163.25 81.433 117.3 19.964 147.14 86.092 82.697 152.05-53.594 54.816-119.2 55.592-192.63 7.4375-73.431 48.155-139.04 47.379-192.63-7.4375-64.445-65.957-34.605-132.08 82.697-152.05-67.108 11.421-142.55-7.4491-163.25-81.433-5.9562-21.282-16.111-152.36-16.111-170.07 0-88.687 77.742-60.816 125.72-24.795z"
			/>
		</svg>
	{/if}
	<span>{username}</span>
</a>

<style>
	.social-btn {
		display: inline-flex;
		align-items: center;
		gap: 0.5rem;
		padding: 0.5rem 1rem;
		border-radius: var(--radius-sm, 8px);
		font-size: 0.875rem;
		font-weight: 500;
		text-decoration: none;
		transition: all 0.2s;
	}

	.social-btn.github {
		background: #24292e;
		color: white;
	}

	.social-btn.bluesky {
		background: #0085ff;
		color: white;
	}

	.social-btn:hover {
		transform: translateY(-2px);
		filter: brightness(1.1);
	}
</style>
