// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

import type { UserResponse } from './generated';

export const uiState = $state({
	isAboutOpen: false,
	backendBuildId: null as string | null,
	user: null as UserResponse | null,
	isUserLoading: true
});
