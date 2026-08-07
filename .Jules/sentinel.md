## 2026-08-07 - Crossbeam-epoch vulnerability
**Vulnerability:** The `crossbeam-epoch` dependency version `0.9.18` has an Invalid pointer dereference vulnerability (RUSTSEC-2026-0204).
**Learning:** Outdated dependencies can contain known security flaws.
**Prevention:** Regularly run `cargo audit` in CI to catch vulnerabilities.
