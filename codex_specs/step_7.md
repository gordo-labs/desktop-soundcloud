Harden the project:
- Strengthen the CSP (avoid `unsafe-inline` and `unsafe-eval` if possible; use nonces/hashes only if absolutely required by SoundCloud).
- Block unnecessary protocols (`file://`).
- Review the Tauri API allowlist and restrict `shell.open` to https/http with validation.
- Disable drag & drop of files if unused.
- Document a final security checklist.
Return the final JSON and code changes.
