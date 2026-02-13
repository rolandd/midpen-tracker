<!-- SPDX-License-Identifier: MIT -->
<!-- Copyright 2026 Roland Dreier <roland@rolandd.dev> -->

<script lang="ts">
	import { goto } from '$app/navigation';
	import { fetchMe, logout, ApiError } from '$lib/api';

	$effect(() => {
		const controller = new AbortController();
		let timeoutId: ReturnType<typeof setTimeout>;

		const poll = async () => {
			try {
				const user = await fetchMe(controller.signal);
				if (controller.signal.aborted) return;

				if (!user.deletion_requested_at) {
					// Deletion aborted/cleared -> back to dashboard
					goto('/dashboard');
					return;
				}
				// Still present and marked for deletion -> wait
				if (!controller.signal.aborted) timeoutId = setTimeout(poll, 2000);
			} catch (e: unknown) {
				if (controller.signal.aborted) return;

				if (e instanceof ApiError && (e.status === 401 || e.status === 404)) {
					// Session gone -> deletion complete
					await logout(controller.signal).catch(() => {}); // Ensure cookie is cleared, ignore errors
					if (controller.signal.aborted) return;
					goto('/');
				} else {
					// Transient error -> retry
					console.debug('Transient error during deletion poll, retrying...', e);
					if (!controller.signal.aborted) timeoutId = setTimeout(poll, 5000);
				}
			}
		};

		poll();

		return () => {
			controller.abort();
			clearTimeout(timeoutId);
		};
	});
</script>

<div class="container">
	<div class="content">
		<div class="spinner"></div>
		<h1>Deleting Account</h1>
		<p>We are securely removing your data.</p>
		<p class="sub">You will be redirected automatically when finished.</p>
	</div>
</div>

<style>
	.container {
		display: flex;
		align-items: center;
		justify-content: center;
		min-height: 60vh;
		text-align: center;
	}

	.content {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 1.5rem;
	}

	h1 {
		font-size: 1.5rem;
		font-weight: 600;
		color: var(--color-text);
		margin: 0;
	}

	p {
		color: var(--color-text-muted);
		margin: 0;
	}

	.sub {
		font-size: 0.875rem;
		opacity: 0.8;
	}

	.spinner {
		width: 48px;
		height: 48px;
		border: 4px solid var(--color-border);
		border-top-color: var(--color-danger, #ef4444);
		border-radius: 50%;
		animation: spin 1s linear infinite;
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}
</style>
