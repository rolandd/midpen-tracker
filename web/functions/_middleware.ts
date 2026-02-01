// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

/// <reference types="@cloudflare/workers-types" />

/**
 * Cloudflare Pages middleware that injects CSP nonces into HTML responses.
 *
 * This middleware:
 * 1. Generates a cryptographically random nonce per request
 * 2. Adds nonce="..." to all <script> tags in the HTML (streaming)
 * 3. Sets the Content-Security-Policy header with the nonce
 *
 * Uses HTMLRewriter for streaming transformation (no buffering).
 * Cloudflare automatically adds the nonce to their injected scripts
 * (analytics, bot fight) when they detect a nonce-based CSP.
 */

interface Env {
	PUBLIC_API_URL?: string;
}

export const onRequest: PagesFunction<Env> = async (context) => {
	const response = await context.next();

	// Only process HTML responses
	const contentType = response.headers.get('content-type') || '';
	if (!contentType.includes('text/html')) {
		return response;
	}

	// Generate a cryptographically random nonce per CSP spec:
	// at least 128 bits of random data, base64-encoded
	const nonceBytes = new Uint8Array(16); // 128 bits
	crypto.getRandomValues(nonceBytes);
	const nonce = btoa(String.fromCharCode(...nonceBytes));

	// Build the CSP header
	const apiUrl = context.env.PUBLIC_API_URL || '';
	const connectSrc = ["'self'", 'https://cloudflareinsights.com', 'https://*.run.app', apiUrl]
		.filter(Boolean)
		.join(' ');

	const csp = [
		"default-src 'self'",
		`script-src 'self' 'nonce-${nonce}' https://static.cloudflareinsights.com`,
		"style-src 'self' 'unsafe-inline' https://fonts.googleapis.com",
		"font-src 'self' https://fonts.gstatic.com",
		"img-src 'self' data: https:",
		`connect-src ${connectSrc}`,
		"frame-ancestors 'none'"
	].join('; ');

	// Use HTMLRewriter for streaming transformation (no buffering)
	const rewriter = new HTMLRewriter().on('script', {
		element(el) {
			el.setAttribute('nonce', nonce);
		}
	});

	// Transform the response
	const transformedResponse = rewriter.transform(response);

	// Create new Response with mutable headers (avoids errors on cached/immutable responses)
	const newHeaders = new Headers(transformedResponse.headers);
	newHeaders.set('Content-Security-Policy', csp);

	return new Response(transformedResponse.body, {
		status: transformedResponse.status,
		statusText: transformedResponse.statusText,
		headers: newHeaders
	});
};
