# Contributing Guide

Thanks for your interest in improving SoundCloud Desktop! Contributions of all sizes are welcome.

## Ways to contribute
- Report bugs and propose enhancements by opening an issue.
- Improve documentation and examples.
- Submit pull requests for bug fixes or small features.
- Help triage issues and review PRs.

## Development setup
1. Install prerequisites (Node.js 18+, Rust stable).
2. From the repo root:
   ```bash
   cd soundcloud-wrapper-tauri
   npm install
   npm run tauri:dev
   ```

## Pull request checklist
- Keep PRs focused and small when possible.
- Add or update tests when appropriate (`npm run test`).
- Run the app locally on at least one platform.
- Follow existing code style and naming conventions.
- Update docs (`README.md`, `docs/`) if behavior or setup changes.

## Commit messages
Use clear, imperative subjects, e.g. "Add media key handler on Linux". Reference issues with `Closes #123` when relevant.

## Code of Conduct
By participating, you agree to abide by the [Code of Conduct](CODE_OF_CONDUCT.md).

## License
By contributing, you agree that your contributions are licensed under the repository's [MIT License](LICENSE).



