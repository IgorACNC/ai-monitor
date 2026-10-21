# ai-monitor

Software desktop (Windows) para acompanhar o uso do Claude Code. 100% local. Escopo completo em [docs/PRD.md](docs/PRD.md).

## Rodar em desenvolvimento

Pré-requisitos: [Node.js](https://nodejs.org), [Rust](https://rustup.rs) e as Build Tools do Visual Studio (C++).

```sh
npm install
npm run tauri dev
```

## Testes

```sh
cd src-tauri
cargo test
```
