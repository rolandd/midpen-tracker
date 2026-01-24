# Contributing to Midpen Tracker

Thank you for your interest in contributing to Midpen Tracker! We welcome contributions from the community to help improve this project.

## Getting Started

1.  **Fork the repository** on GitHub.
2.  **Clone your fork** locally:
    ```bash
    git clone https://github.com/your-username/midpen-strava.git
    cd midpen-strava
    ```
3.  **Install dependencies**:
    - Backend: Ensure you have [Rust](https://www.rust-lang.org/tools/install) installed.
    - Frontend: Ensure you have [Node.js](https://nodejs.org/) installed.
    - Infrastructure: [Terraform](https://www.terraform.io/) (optional, for deployment).
    - [Just](https://github.com/casey/just) command runner.

## Development Workflow

We use `just` to manage development tasks.

### Backend (Rust)
To run the backend locally:
```bash
just dev-api
```
This will start the server on `http://localhost:8080`.

### Frontend (SvelteKit)
To run the frontend locally:
```bash
cd web
npm install
npm run dev
```
The frontend will be available at `http://localhost:5173`.

## Testing

Please ensure all tests pass before submitting a pull request.

- **Backend Tests**:
  ```bash
  cargo test
  ```
- **Frontend Checks**:
  ```bash
  cd web
  npm run check
  npm run lint
  ```

## Code Style

- **Rust**: We use `rustfmt` and `clippy`.
  ```bash
  cargo fmt
  cargo clippy -- -D warnings
  ```
- **Frontend**: We use `prettier` and `eslint`.
  ```bash
  cd web
  npm run format
  npm run lint
  ```

## Submitting Changes

1.  Create a new branch for your feature or bugfix:
    ```bash
    git checkout -b feature/my-new-feature
    ```
2.  Commit your changes with clear, descriptive messages.
3.  Push your branch to your fork.
4.  Open a **Pull Request** against the `main` branch of the original repository.

## License

By contributing, you agree that your contributions will be licensed under the MIT License, as defined in the [LICENSE](LICENSE) file.
