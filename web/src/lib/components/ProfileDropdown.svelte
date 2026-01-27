<!-- SPDX-License-Identifier: MIT -->
<!-- Copyright 2026 Roland Dreier <roland@rolandd.dev> -->

<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import type { UserResponse } from '../generated';
	import { DeleteAccountModal } from '$lib/components';
	import { deleteAccount } from '$lib/api';

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

	function toggleDropdown() {
		isOpen = !isOpen;
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

	onMount(() => {
		document.addEventListener('click', handleClickOutside);
		return () => {
			document.removeEventListener('click', handleClickOutside);
		};
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
		bind:this={triggerRef}
		aria-expanded={isOpen}
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
			class="absolute top-[calc(100%+0.5rem)] right-0 w-[220px] bg-[var(--color-surface)] rounded-xl shadow-lg ring-1 ring-[var(--color-border)] p-2 z-50 origin-top-right animate-in fade-in zoom-in-95 duration-200"
			bind:this={dropdownRef}
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
				class="group flex items-center justify-between w-full px-3 py-2.5 rounded-lg text-[var(--color-text-muted)] no-underline text-sm transition-colors hover:bg-[var(--color-surface-hover)] hover:text-[var(--color-primary)]"
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
				class="flex items-center justify-between w-full px-3 py-2.5 rounded-lg text-sm transition-colors border-none bg-transparent cursor-pointer text-left text-red-500 hover:bg-red-500/10 hover:text-red-700"
				onclick={async () => {
					await onLogout();
					closeDropdown();
				}}
				disabled={isLoggingOut}
			>
				{#if isLoggingOut}
					Logging out...
				{:else}
					Log Out
				{/if}
			</button>

			<button
				class="flex items-center justify-between w-full px-3 py-2.5 rounded-lg text-sm transition-colors border-none bg-transparent cursor-pointer text-left text-red-500 hover:bg-red-500/10 hover:text-red-700 font-medium mt-1"
				onclick={() => {
					showDeleteConfirmation = true;
					closeDropdown();
				}}
				disabled={isLoggingOut}
			>
				Delete My Account
			</button>

			<div
				class="px-3 py-2 text-xs text-[var(--color-text-muted)] text-center border-t border-[var(--color-border)] mt-1 opacity-50"
			>
				Build: {import.meta.env.PUBLIC_BUILD_ID?.substring(0, 7) || 'dev'}
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
