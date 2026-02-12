<!-- SPDX-License-Identifier: MIT -->
<!-- Copyright 2026 Roland Dreier <roland@rolandd.dev> -->

<script lang="ts">
	import { fade, fly } from 'svelte/transition';
	import Button from './Button.svelte';

	interface Props {
		onConfirm: () => Promise<void>;
		onCancel: () => void;
	}

	let { onConfirm, onCancel }: Props = $props();
	let isDeleting = $state(false);
	let error = $state<string | null>(null);

	async function handleConfirm() {
		isDeleting = true;
		error = null;
		try {
			await onConfirm();
		} catch (e) {
			const rawMessage = e instanceof Error ? e.message : 'Failed to delete account';
			if (rawMessage.includes('500')) {
				error = 'Server error. Please try again later.';
			} else {
				error = rawMessage;
			}
			isDeleting = false;
		}
	}

	let modalRef = $state<HTMLElement>();

	function handleKeydown(e: KeyboardEvent) {
		if (isDeleting) return;

		if (e.key === 'Escape') {
			e.preventDefault();
			onCancel();
			return;
		}

		if (e.key === 'Tab' && modalRef) {
			const focusable = modalRef.querySelectorAll<HTMLElement>(
				'button:not([disabled]), [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
			);

			if (!focusable || focusable.length === 0) return;

			const first = focusable[0];
			const last = focusable[focusable.length - 1];

			if (e.shiftKey) {
				if (document.activeElement === first) {
					e.preventDefault();
					last.focus();
				}
			} else {
				if (document.activeElement === last) {
					e.preventDefault();
					first.focus();
				}
			}
		}
	}
</script>

<svelte:window onkeydown={handleKeydown} />

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
	class="backdrop"
	transition:fade={{ duration: 200 }}
	onclick={(e) => {
		if (e.target === e.currentTarget && !isDeleting) onCancel();
	}}
>
	<div
		bind:this={modalRef}
		class="modal"
		transition:fly={{ y: 20, duration: 300 }}
		role="dialog"
		aria-modal="true"
		aria-labelledby="delete-account-title"
		aria-describedby="delete-account-desc"
		tabindex="-1"
	>
		<div class="icon-container">
			<svg
				xmlns="http://www.w3.org/2000/svg"
				width="32"
				height="32"
				viewBox="0 0 24 24"
				fill="none"
				stroke="currentColor"
				stroke-width="2"
				stroke-linecap="round"
				stroke-linejoin="round"
			>
				<path
					d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"
				/>
				<line x1="12" y1="9" x2="12" y2="13" />
				<line x1="12" y1="17" x2="12.01" y2="17" />
			</svg>
		</div>

		<h2 id="delete-account-title">Delete Your Account?</h2>

		<p class="warning" id="delete-account-desc">
			This action is <strong>permanent</strong> and cannot be undone.
		</p>

		<div class="details">
			<p><strong>All your data will be deleted:</strong></p>
			<ul>
				<li>Your Midpen Tracker profile</li>
				<li>All activity records</li>
				<li>Preserve visit statistics</li>
				<li>Connection to your Strava account</li>
			</ul>
		</div>

		<div class="note">
			<svg
				xmlns="http://www.w3.org/2000/svg"
				width="16"
				height="16"
				viewBox="0 0 24 24"
				fill="none"
				stroke="currentColor"
				stroke-width="2"
			>
				<circle cx="12" cy="12" r="10" />
				<line x1="12" y1="16" x2="12" y2="12" />
				<line x1="12" y1="8" x2="12.01" y2="8" />
			</svg>
			<span>Any preserve annotations added to your Strava activities will remain.</span>
		</div>

		{#if error}
			<div class="error">
				{error}
			</div>
		{/if}

		<div class="actions">
			<Button variant="secondary" onclick={onCancel} disabled={isDeleting} class="flex-1" autofocus>
				Cancel
			</Button>
			<Button
				variant="danger"
				onclick={handleConfirm}
				isLoading={isDeleting}
				disabled={isDeleting}
				class="flex-1"
			>
				Delete Permanently
			</Button>
		</div>
	</div>
</div>

<style>
	.backdrop {
		position: fixed;
		inset: 0;
		background: rgba(0, 0, 0, 0.5);
		backdrop-filter: blur(4px);
		z-index: 200;
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 1rem;
	}

	.modal {
		background: var(--color-surface);
		width: 100%;
		max-width: 420px;
		border-radius: var(--radius);
		border: 1px solid var(--color-border);
		box-shadow: 0 20px 25px -5px rgba(0, 0, 0, 0.2);
		padding: 1.5rem;
		text-align: center;
	}

	.icon-container {
		width: 56px;
		height: 56px;
		background: rgba(239, 68, 68, 0.1);
		border-radius: 50%;
		display: flex;
		align-items: center;
		justify-content: center;
		margin: 0 auto 1rem;
		color: #ef4444;
	}

	h2 {
		font-size: 1.25rem;
		font-weight: 700;
		color: var(--color-text);
		margin: 0 0 0.5rem;
	}

	.warning {
		color: var(--color-text-muted);
		font-size: 0.95rem;
		margin-bottom: 1rem;
	}

	.warning strong {
		color: #ef4444;
	}

	.details {
		text-align: left;
		background: var(--color-surface-hover);
		border-radius: var(--radius-sm);
		padding: 1rem;
		margin-bottom: 1rem;
	}

	.details p {
		font-size: 0.875rem;
		font-weight: 600;
		color: var(--color-text);
		margin: 0 0 0.5rem;
	}

	.details ul {
		margin: 0;
		padding-left: 1.25rem;
	}

	.details li {
		font-size: 0.875rem;
		color: var(--color-text-muted);
		margin-bottom: 0.25rem;
	}

	.note {
		display: flex;
		align-items: flex-start;
		gap: 0.5rem;
		text-align: left;
		background: rgba(59, 130, 246, 0.08);
		border-radius: var(--radius-sm);
		padding: 0.75rem;
		margin-bottom: 1.25rem;
		font-size: 0.8rem;
		color: var(--color-text-muted);
	}

	.note svg {
		flex-shrink: 0;
		margin-top: 0.1rem;
		color: #3b82f6;
	}

	.error {
		background: rgba(239, 68, 68, 0.1);
		color: #ef4444;
		padding: 0.75rem;
		border-radius: var(--radius-sm);
		font-size: 0.875rem;
		margin-bottom: 1rem;
	}

	.actions {
		display: flex;
		gap: 0.75rem;
	}
</style>
