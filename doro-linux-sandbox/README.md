# doro-linux-sandbox

This crate is responsible for producing:

- a `doro-linux-sandbox` standalone executable for Linux that is bundled with the Node.js version of the Codex CLI
- a lib crate that exposes the business logic of the executable as `run_main()` so that
  - the `doro-exec` CLI can check if its arg0 is `doro-linux-sandbox` and, if so, execute as if it were `doro-linux-sandbox`
  - this should also be true of the `codex` multitool CLI
