#!/usr/bin/env node
// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

/**
 * Verifies the adapter-static output contract.
 *
 * Framework upgrades (SvelteKit 3, adapter-static 4, Vite 8) can silently change
 * where files are emitted, which asset paths the shell references, or whether
 * precompression / Cloudflare Pages files are produced. `svelte-check` and unit
 * tests do not catch any of that, so the deploy contract is asserted here.
 *
 * Usage: pnpm build && pnpm test:build
 *
 * NOTE: prerendering is currently disabled (no root `export const prerender = true`),
 * so the build is a single SPA shell (`index.html`) plus static assets. If
 * prerendering is enabled later (see docs/specs/web-major-upgrades.md), extend
 * the EXPECTED_SHELL/EXPECTED_PAGES section below.
 */

import { readFile, stat } from 'node:fs/promises';
import path from 'node:path';

const BUILD_DIR = path.resolve(import.meta.dirname, '..', 'build');

/** Files the SPA shell must reference and that must exist on disk. */
const REQUIRED_STATIC = [
	'fouc.js',
	'robots.txt',
	'site.webmanifest',
	'app-logo.svg',
	'favicon.ico',
	'_headers',
	'_routes.json'
];

/** Headers that Cloudflare Pages must apply (from web/static/_headers). */
const REQUIRED_HEADERS = [
	'X-Frame-Options: DENY',
	'X-Content-Type-Options: nosniff',
	'Referrer-Policy: strict-origin-when-cross-origin',
	'Permissions-Policy: accelerometer=()'
];

const failures = [];
let checks = 0;

function check(description, condition) {
	checks += 1;
	if (!condition) failures.push(description);
}

async function exists(relativePath) {
	try {
		await stat(path.join(BUILD_DIR, relativePath));
		return true;
	} catch {
		return false;
	}
}

async function read(relativePath) {
	return readFile(path.join(BUILD_DIR, relativePath), 'utf8');
}

/** Extract root-relative asset URLs (`/_app/...`) referenced from HTML. */
function referencedAssets(html) {
	const matches = html.matchAll(/(?:src|href)=["'](\/_app\/[^"'?#]+)/g);
	return [...new Set([...matches].map((match) => match[1]))];
}

async function main() {
	let buildStat;
	try {
		buildStat = await stat(BUILD_DIR);
	} catch {
		console.error(`✗ build directory not found: ${BUILD_DIR}`);
		console.error('  Run `pnpm build` first.');
		process.exit(1);
	}
	check('build directory is a directory', buildStat.isDirectory());

	// --- SPA shell -----------------------------------------------------------
	const hasIndex = await exists('index.html');
	check('build/index.html exists', hasIndex);
	if (!hasIndex) return report();

	const html = await read('index.html');
	check('shell contains the SvelteKit bootstrap', html.includes('__sveltekit_'));
	check('shell starts the app via kit.start()', html.includes('kit.start('));
	check(
		'shell imports an entry/start chunk',
		/\/_app\/immutable\/entry\/start\.[\w-]+\.js/.test(html)
	);
	check('shell imports an entry/app chunk', /\/_app\/immutable\/entry\/app\.[\w-]+\.js/.test(html));
	check(
		'shell references at least one stylesheet',
		/\/_app\/immutable\/assets\/[^"']+\.css/.test(html)
	);
	check('shell declares the FOUC script', html.includes('/fouc.js'));
	check('shell keeps preload hints on <body>', html.includes('data-sveltekit-preload-data'));

	const assets = referencedAssets(html);
	check('shell references app assets', assets.length > 0);
	for (const asset of assets) {
		check(`asset exists: ${asset}`, await exists(asset.slice(1)));
	}

	// --- Static files --------------------------------------------------------
	for (const file of REQUIRED_STATIC) {
		check(`static file exists: ${file}`, await exists(file));
	}

	// --- Precompression (adapter-static precompress: true) -------------------
	check('index.html.gz exists', await exists('index.html.gz'));
	check('index.html.br exists', await exists('index.html.br'));
	const compressedSample = assets.filter((asset) => asset.endsWith('.js')).slice(0, 5);
	for (const asset of compressedSample) {
		check(`asset is gzip-compressed: ${asset}.gz`, await exists(`${asset.slice(1)}.gz`));
		check(`asset is brotli-compressed: ${asset}.br`, await exists(`${asset.slice(1)}.br`));
	}

	// --- Cloudflare Pages contract ------------------------------------------
	if (await exists('_headers')) {
		const headers = await read('_headers');
		for (const header of REQUIRED_HEADERS) {
			check(`_headers contains "${header}"`, headers.includes(header));
		}
	}

	if (await exists('_routes.json')) {
		let routes = null;
		try {
			routes = JSON.parse(await read('_routes.json'));
		} catch {
			failures.push('_routes.json is valid JSON');
		}
		if (routes) {
			check('_routes.json version is 1', routes.version === 1);
			check('_routes.json includes /* (SPA fallback)', routes.include?.includes('/*') === true);
			check('_routes.json excludes hashed assets', routes.exclude?.includes('/_app/*') === true);
		}
	}

	report();
}

function report() {
	if (failures.length > 0) {
		console.error(`✗ build contract failed (${failures.length} of ${checks} checks):`);
		for (const failure of failures) console.error(`  - ${failure}`);
		process.exit(1);
	}
	console.log(`✓ build contract OK (${checks} checks)`);
}

await main();
