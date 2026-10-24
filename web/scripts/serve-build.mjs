#!/usr/bin/env node
// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

/**
 * Minimal static server for the adapter-static output, used by the Playwright
 * smoke tests.
 *
 * Cloudflare Pages serves `index.html` for every path matched by
 * `_routes.json` (include: ["/*"]), which is why the SPA can deep-link to
 * `/dashboard`. `vite preview` does not reproduce that, so this server mirrors
 * it: unknown extension-less paths fall back to the built shell, hashed assets
 * 404 instead of returning HTML.
 */

import { createReadStream } from 'node:fs';
import { stat } from 'node:fs/promises';
import { createServer } from 'node:http';
import path from 'node:path';

const ROOT = path.resolve(import.meta.dirname, '..', 'build');
const PORT = Number(process.env.PORT ?? 4173);
const HOST = process.env.HOST ?? '127.0.0.1';

const MIME_TYPES = {
	'.html': 'text/html; charset=utf-8',
	'.js': 'text/javascript; charset=utf-8',
	'.mjs': 'text/javascript; charset=utf-8',
	'.css': 'text/css; charset=utf-8',
	'.json': 'application/json; charset=utf-8',
	'.webmanifest': 'application/manifest+json',
	'.svg': 'image/svg+xml',
	'.png': 'image/png',
	'.ico': 'image/x-icon',
	'.txt': 'text/plain; charset=utf-8',
	'.woff': 'font/woff',
	'.woff2': 'font/woff2',
	'.map': 'application/json; charset=utf-8',
	'.br': 'application/octet-stream',
	'.gz': 'application/octet-stream'
};

async function fileOrNull(candidate) {
	// Reject path traversal.
	if (!candidate.startsWith(ROOT + path.sep)) return null;
	try {
		const info = await stat(candidate);
		return info.isFile() ? candidate : null;
	} catch {
		return null;
	}
}

const server = createServer(async (request, response) => {
	const url = new URL(request.url ?? '/', `http://${HOST}:${PORT}`);
	let pathname;
	try {
		pathname = decodeURIComponent(url.pathname);
	} catch {
		response.writeHead(400).end('Bad request');
		return;
	}

	const candidates = [
		path.join(ROOT, pathname),
		path.join(ROOT, `${pathname}.html`),
		path.join(ROOT, pathname, 'index.html')
	];

	let file = null;
	for (const candidate of candidates) {
		file = await fileOrNull(candidate);
		if (file) break;
	}

	// Pages-style SPA fallback for extension-less routes (e.g. /dashboard).
	if (!file && !path.extname(pathname)) {
		file = await fileOrNull(path.join(ROOT, 'index.html'));
	}

	if (!file) {
		response.writeHead(404, { 'Content-Type': 'text/plain; charset=utf-8' }).end('Not found');
		return;
	}

	const extension = path.extname(file);
	response.writeHead(200, {
		'Content-Type': MIME_TYPES[extension] ?? 'application/octet-stream',
		'Cache-Control': extension === '.html' ? 'no-store' : 'public, max-age=60'
	});
	createReadStream(file).pipe(response);
});

server.listen(PORT, HOST, () => {
	console.log(`serving ${ROOT} at http://${HOST}:${PORT}`);
});
