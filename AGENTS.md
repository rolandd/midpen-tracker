# AI Agent Guidelines

Guidance for AI coding agents working on this codebase.

For project architecture, flows, and setup see [CONTRIBUTING.md](CONTRIBUTING.md).

## Meta-Constraints

1. **Challenge Outdated Constraints**: If a constraint in this file
   conflicts with a modern best practice, causes deprecation warnings,
   or makes a solution unnecessarily complex — **stop**. Don't blindly
   follow it. Instead, propose a revision explaining why the
   constraint should be updated.

2. **Keep This File Current**: AGENTS.md is a living document. If you
   implement a change that affects architecture (e.g., new top-level
   directories, new build steps, new dependencies), update this file
   or the relevant section of CONTRIBUTING.md in the same commit.

3. **Clarify, Don't Assume**: If a task is ambiguous or missing
   critical details, ask clarifying questions rather than guessing.

4. **Design First for Complex Changes**: Before writing code for
   complex features or architectural changes:
   - Create a design doc in `docs/specs/` outlining data model
     changes, API endpoints, and UI components
   - Record significant trade-offs (e.g., library choices) as ADRs in
     `docs/adr/`
   - These directories can be created when first needed

## Commit Conventions

Use [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <description>

[optional body]
```

**Types**: `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `build`, `ci`, `chore`

**Scopes**: `api`, `web`, `auth`, `db`, `infra`, `webhook`, `tasks`, `strava`

Examples:
```
feat(api): add endpoint for activity deletion
fix(auth): validate OAuth state signature before use
refactor(db): use concurrent Firestore operations
docs: update README with new deploy instructions
```

## Code Standards

### Rust
- Run `cargo fmt` before committing
- Ensure `cargo clippy -- -D warnings` passes
- Add doc comments (`///`) on public functions

### Frontend (Svelte/TypeScript)
- **Strictly use TypeScript**, never raw JavaScript
- Run `npm run format`, `npm run lint`, and `npm run check` in `web/` — all must pass cleanly
- Use generated types from `web/src/lib/generated/`

## Before Committing

Run the full check:
```bash
just check-all
```

If you modified Rust DTOs in `src/models/` or `src/routes/api.rs`:
```bash
just generate-bindings
```

## Testing

- Unit tests: `cargo test`
- Integration tests require Firestore emulator: `./scripts/test-with-emulator.sh`
- Frontend: `cd web && npm run check`

## Key Files Reference

| Area | Key Files |
|------|-----------|
| Config & secrets | `src/config.rs` |
| Auth flow | `src/routes/auth.rs`, `src/middleware/auth.rs` |
| API endpoints | `src/routes/api.rs` |
| Async tasks | `src/routes/tasks.rs`, `src/services/tasks.rs` |
| Database | `src/db/firestore.rs` |
| Strava client | `src/services/strava.rs` |
| Preserve detection | `src/services/preserve.rs` |
| Infrastructure | `infra/*.tf` |
| Build/deploy | `justfile`, `cloudbuild.yaml` |

## DOs and DON'Ts

### DOs

- **DO** run `just check-all` before considering work complete
- **DO** run integration tests with emulator after DB changes: `./scripts/test-with-emulator.sh`
- **DO** regenerate TypeScript bindings after modifying Rust DTOs: `just generate-bindings`
- **DO** use `Config::test_default()` for unit tests, not `Config::from_env()`
- **DO** validate security-sensitive inputs (OAuth state, webhook signatures, JWT claims)
- **DO** use HMAC signatures for state parameters to prevent CSRF
- **DO** prefer environment variables over runtime API calls for secrets (Cloud Run injects them)
- **DO** use `futures_util::stream::buffer_unordered` for concurrent Firestore operations
- **DO** ensure `npm run check` and `npm run lint` pass cleanly for frontend code
- **DO** check `just check-webhooks` after deploying to verify webhook registration
- **DO** consider race conditions when multiple requests can modify the same data
- **DO** use Firestore transactions for read-modify-write operations to ensure consistency

### DON'Ts

- **DON'T** add dependencies that require OpenSSL — backend runs on distroless, use `rustls` instead
- **DON'T** use raw JavaScript — use type-safe TypeScript for all frontend code
- **DON'T** use the `Host` header to construct URLs — use explicit `API_URL` env var instead
- **DON'T** mix GET/POST for auth routes — logout is POST, OAuth callback is GET
- **DON'T** trust webhook payloads without validating the verify token
- **DON'T** forget that Cloud Tasks adds headers — validate `X-CloudTasks-QueueName`
- **DON'T** use `unwrap()` in production code paths — use proper error handling
- **DON'T** make direct Secret Manager API calls — secrets are pre-injected as env vars
- **DON'T** skip the Firestore emulator for integration tests — they will fail or be flaky

### Security Checklist

When modifying auth or security-related code:
- [ ] OAuth state is signed and verified
- [ ] JWT expiration is checked
- [ ] CORS allows only expected origins
- [ ] Cloud Tasks requests validate queue name header
- [ ] Webhook requests verify the token
- [ ] No secrets logged or exposed in responses

## Common Gotchas

1. **Firestore emulator required** — Integration tests fail without it
2. **TS bindings stale** — Regenerate after modifying Rust DTOs
3. **Webhook URL mismatch** — After deploy, verify with `just check-webhooks`
4. **Secrets in prod** — Injected as env vars by Cloud Run, not direct API calls
5. **Route method mismatch** — Frontend expects POST for logout, backend must match
