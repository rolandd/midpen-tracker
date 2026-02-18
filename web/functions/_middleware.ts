// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

/// <reference types="@cloudflare/workers-types" />

import { PERMISSIONS_POLICY } from './security-config';

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
	PUBLIC_BASE_URL?: string;
}

export const onRequest: PagesFunction<Env> = async (context) => {
	const url = new URL(context.request.url);

	// Redirect pages.dev to custom domain (if configured)
	// This prevents issues with Cloudflare Access policies on pages.dev
	// affecting the custom domain via CNAME resolution
	if (url.hostname.endsWith('.pages.dev') && context.env.PUBLIC_BASE_URL) {
		const customDomain = new URL(context.env.PUBLIC_BASE_URL);
		url.hostname = customDomain.hostname;
		url.protocol = customDomain.protocol;
		return Response.redirect(url.toString(), 301);
	}

	const response = await context.next();

	// Create new Response with mutable headers (avoids errors on cached/immutable responses)
	// We do this early so we can apply security headers to ALL responses
	const newHeaders = new Headers(response.headers);

	// Add Security Headers (Global)
	newHeaders.set('X-Content-Type-Options', 'nosniff');
	newHeaders.set('X-Frame-Options', 'DENY');
	newHeaders.set('Referrer-Policy', 'no-referrer');
	newHeaders.set('Permissions-Policy', PERMISSIONS_POLICY);
	newHeaders.set('Strict-Transport-Security', 'max-age=31536000; includeSubDomains; preload');

	// Only process HTML responses for CSP injection
	const contentType = response.headers.get('content-type') || '';
	if (!contentType.includes('text/html')) {
		// Return response with added security headers
		return new Response(response.body, {
			status: response.status,
			statusText: response.statusText,
			headers: newHeaders
		});
	}

	// Generate a cryptographically random nonce per CSP spec:
	// at least 128 bits of random data, base64-encoded
	const nonceBytes = new Uint8Array(16); // 128 bits
	crypto.getRandomValues(nonceBytes);
	const nonce = btoa(String.fromCharCode(...nonceBytes));

	// Build the CSP header
	const apiUrl = context.env.PUBLIC_API_URL || '';
	const connectSrc = ["'self'", 'https://cloudflareinsights.com', apiUrl].filter(Boolean).join(' ');

	const csp = [
		"default-src 'self'",
		`script-src 'self' 'nonce-${nonce}' https://static.cloudflareinsights.com`,
		"style-src 'self' 'unsafe-inline' https://fonts.googleapis.com",
		"font-src 'self' https://fonts.gstatic.com",
		"img-src 'self' data: https:",
		`connect-src ${connectSrc}`,
		"object-src 'none'",
		"base-uri 'self'",
		"form-action 'self'",
		"frame-ancestors 'none'"
	].join('; ');

	// Set CSP header on the mutable headers object
	newHeaders.set('Content-Security-Policy', csp);

	// Use HTMLRewriter for streaming transformation (no buffering)
	const rewriter = new HTMLRewriter().on('script', {
		element(el) {
			el.setAttribute('nonce', nonce);
		}
	});

	// Transform the response
	const transformedResponse = rewriter.transform(
		new Response(response.body, {
			status: response.status,
			statusText: response.statusText,
			headers: newHeaders
		})
	);

	return transformedResponse;
};
