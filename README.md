# ai-monitor

Software desktop (Windows) para acompanhar o uso do Claude Code. 100% local. Escopo completo em [docs/PRD.md](docs/PRD.md).

## Rodar em desenvolvimento

Pré-requisitos: [Node.js](https://nodejs.org), [Rust](https://rustup.rs) e as Build Tools do Visual Studio (C++).

```sh
npm install
npm run tauri dev
```

## Gerar o instalador da versão nova

Sempre que o código mudar, gere um instalador novo e instale por cima do anterior. As configurações, o histórico e os
GIFs do bichinho ficam guardados fora da pasta do programa, então não se perdem.

1. **Feche o app**: clique com o botão direito no ícone dele na bandeja do Windows (perto do relógio) → **Sair**.
   Fechar só a janela não basta, porque o app continua rodando na bandeja.
2. **Abra um terminal** (PowerShell ou Windows Terminal) e entre na pasta do projeto:
   ```sh
   cd A:\ai-monitor
   ```
3. **Pegue a versão mais recente do código** (se ela estiver no GitHub) e instale as dependências:
   ```sh
   git pull
   npm install
   ```
4. **Gere o instalador**:
   ```sh
   npm run tauri build
   ```
   Leva de 5 a 15 minutos, porque compila tudo de forma otimizada. Termina com a mensagem `Finished 1 bundle at:`.
5. **Instale**: dê dois cliques no arquivo gerado em
   ```
   src-tauri\target\release\bundle\nsis\AI Monitor_<versão>_x64-setup.exe
   ```
   Se o Windows mostrar "O Windows protegeu o seu PC", clique em **Mais informações → Executar assim mesmo** (acontece
   com programas sem certificado de assinatura). Escolha a mesma pasta da instalação anterior.
6. **Abra o AI Monitor** pelo menu Iniciar ou pelo atalho.

Para saber qual versão está instalada, aumente o número da versão antes do passo 4, nos três arquivos:
`package.json`, `src-tauri/tauri.conf.json` e `src-tauri/Cargo.toml` (campo `version`, por exemplo de `0.1.0` para
`0.2.0`). O número aparece no nome do instalador.

## Testes

```sh
cd src-tauri
cargo test
cd ..
npm test
```
