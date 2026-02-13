<!-- SPDX-License-Identifier: MIT -->
<!-- Copyright 2026 Roland Dreier <roland@rolandd.dev> -->

<script lang="ts">
	import { tick } from 'svelte';
	import { goto } from '$app/navigation';
	import type { UserResponse } from '../generated';
	import { DeleteAccountModal } from '$lib/components';
	import { deleteAccount } from '$lib/api';
	import { uiState } from '$lib/state.svelte';

	interface Props {
		user: UserResponse | null;
		onLogout: () => Promise<void>;
		isLoggingOut: boolean;
	}

	let { user, onLogout, isLoggingOut }: Props = $props();
	let isOpen = $state(false);
	let showDeleteConfirmation = $state(false);
	let dropdownRef = $state<HTMLDivElement>();
	let triggerRef = $state<HTMLButtonElement>();

	const menuId = 'user-profile-menu';

	async function toggleDropdown() {
		isOpen = !isOpen;
		if (isOpen) {
			await tick();
			const firstItem = dropdownRef?.querySelector<HTMLElement>(
				'[role="menuitem"]:not([disabled])'
			);
			firstItem?.focus();
		}
	}

	function closeDropdown() {
		isOpen = false;
	}

	function handleClickOutside(event: MouseEvent) {
		if (
			isOpen &&
			dropdownRef &&
			!dropdownRef.contains(event.target as Node) &&
			triggerRef &&
			!triggerRef.contains(event.target as Node)
		) {
			closeDropdown();
		}
	}

	function handleKeydown(event: KeyboardEvent) {
		if (!isOpen) return;

		if (event.key === 'Escape') {
			event.preventDefault();
			closeDropdown();
			triggerRef?.focus();
			return;
		}

		const items = Array.from(
			dropdownRef?.querySelectorAll<HTMLElement>('[role="menuitem"]:not([disabled])') || []
		);
		if (items.length === 0) return;

		if (['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key)) {
			event.preventDefault();
			const currentIndex = items.indexOf(document.activeElement as HTMLElement);
			let nextIndex;

			if (event.key === 'Home') {
				nextIndex = 0;
			} else if (event.key === 'End') {
				nextIndex = items.length - 1;
			} else if (currentIndex === -1) {
				// If no item is focused, start from the first one
				nextIndex = event.key === 'ArrowDown' ? 0 : items.length - 1;
			} else if (event.key === 'ArrowDown') {
				nextIndex = (currentIndex + 1) % items.length;
			} else {
				nextIndex = (currentIndex - 1 + items.length) % items.length;
			}

			items[nextIndex].focus();
		}
	}

	function handleTriggerKeydown(event: KeyboardEvent) {
		if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
			event.preventDefault();
			event.stopPropagation();
			if (!isOpen) toggleDropdown();
		}
	}

	$effect(() => {
		if (isOpen) {
			window.addEventListener('keydown', handleKeydown);
			return () => {
				window.removeEventListener('keydown', handleKeydown);
			};
		}
	});

	$effect(() => {
		if (isOpen) {
			document.addEventListener('click', handleClickOutside);
			return () => {
				document.removeEventListener('click', handleClickOutside);
			};
		}
	});

	// Helper to get initials
	function getInitials(firstname: string, lastname: string): string {
		return ((firstname.charAt(0) || '') + (lastname.charAt(0) || '')).toUpperCase();
	}
</script>

<div class="relative flex items-center">
	<!-- Trigger Button -->
	<button
		class="bg-transparent border border-[var(--color-border)] p-0 w-9 h-9 cursor-pointer rounded-[var(--radius-sm)] transition-all hover:bg-[var(--color-surface-hover)] hover:border-[var(--color-primary)] focus-visible:outline-2 focus-visible:outline-[var(--color-primary)] focus-visible:outline-offset-2 flex items-center justify-center overflow-hidden"
		onclick={toggleDropdown}
		onkeydown={handleTriggerKeydown}
		bind:this={triggerRef}
		aria-expanded={isOpen}
		aria-haspopup="menu"
		aria-controls={menuId}
		aria-label="User menu"
	>
		{#if user?.profile_picture}
			<img
				src={user.profile_picture}
				alt="{user.firstname} {user.lastname}"
				class="w-full h-full object-cover"
			/>
		{:else if user}
			<div
				class="w-full h-full bg-linear-to-br from-[var(--color-primary)] to-[var(--color-primary-hover)] text-white flex items-center justify-center font-semibold text-xs text-shadow-sm"
			>
				{getInitials(user.firstname, user.lastname)}
			</div>
		{:else}
			<!-- Loading/Fallback state -->
			<div class="w-full h-full bg-[var(--color-surface-hover)]"></div>
		{/if}
	</button>

	<!-- Dropdown Menu -->
	{#if isOpen && user}
		<div
			id={menuId}
			class="absolute top-[calc(100%+0.5rem)] right-0 w-[220px] bg-[var(--color-surface)] rounded-xl shadow-lg ring-1 ring-[var(--color-border)] p-2 z-50 origin-top-right animate-in fade-in zoom-in-95 duration-200"
			bind:this={dropdownRef}
			role="menu"
		>
			<div class="px-3 pt-3 pb-2 border-b border-[var(--color-border)] mb-1">
				<span class="block font-semibold text-[var(--color-text)] text-[0.95rem]">
					{user.firstname}
					{user.lastname}
				</span>
			</div>

			<a
				href="https://www.strava.com/athletes/{user.athlete_id}"
				target="_blank"
				rel="noopener noreferrer"
				class="group flex items-center justify-between w-full px-3 py-2.5 rounded-lg text-[var(--color-text-muted)] no-underline text-sm transition-colors hover:bg-[var(--color-surface-hover)] hover:text-[var(--color-primary)] focus-visible:bg-[var(--color-surface-hover)] focus-visible:text-[var(--color-primary)] focus-visible:outline-none"
				aria-label="View on Strava (opens in new tab)"
				role="menuitem"
			>
				View on Strava
				<svg
					xmlns="http://www.w3.org/2000/svg"
					width="14"
					height="14"
					viewBox="0 0 24 24"
					fill="none"
					stroke="currentColor"
					stroke-width="2"
					stroke-linecap="round"
					stroke-linejoin="round"
					class="opacity-50 group-hover:opacity-100 transition-opacity"
				>
					<path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"></path>
					<polyline points="15 3 21 3 21 9"></polyline>
					<line x1="10" y1="14" x2="21" y2="3"></line>
				</svg>
			</a>

			<div class="h-px bg-[var(--color-border)] my-1"></div>

			<button
				class="flex items-center justify-between w-full px-3 py-2.5 rounded-lg text-sm transition-colors border-none bg-transparent cursor-pointer text-left text-red-500 hover:bg-red-500/10 hover:text-red-700 focus-visible:bg-red-500/10 focus-visible:text-red-700 focus-visible:outline-none"
				onclick={async () => {
					await onLogout();
					closeDropdown();
				}}
				disabled={isLoggingOut}
				role="menuitem"
			>
				{#if isLoggingOut}
					Logging out...
				{:else}
					Log Out
				{/if}
			</button>

			<button
				class="flex items-center justify-between w-full px-3 py-2.5 rounded-lg text-sm transition-colors border-none bg-transparent cursor-pointer text-left text-red-500 hover:bg-red-500/10 hover:text-red-700 font-medium mt-1 focus-visible:bg-red-500/10 focus-visible:text-red-700 focus-visible:outline-none"
				onclick={() => {
					showDeleteConfirmation = true;
					closeDropdown();
				}}
				disabled={isLoggingOut}
				role="menuitem"
			>
				Delete My Account
			</button>

			<div
				class="px-3 py-2 text-xs text-[var(--color-text-muted)] text-center border-t border-[var(--color-border)] mt-1 opacity-50"
			>
				Build: {import.meta.env.PUBLIC_BUILD_ID?.substring(0, 7) || 'dev'}
				{#if uiState.backendBuildId}
					/ {uiState.backendBuildId.substring(0, 7) || 'dev'}
				{/if}
			</div>
		</div>
	{/if}
</div>

{#if showDeleteConfirmation}
	<DeleteAccountModal
		onConfirm={async () => {
			await deleteAccount();
			// Redirect to processing page while deletion happens in background
			goto('/account-deletion-in-progress');
		}}
		onCancel={() => (showDeleteConfirmation = false)}
	/>
{/if}
