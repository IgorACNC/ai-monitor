# ai-monitor

Software desktop (Windows, Mac e Linux) para acompanhar o uso do Claude Code. 100% local. Escopo completo em [docs/PRD.md](docs/PRD.md).

## Rodar em desenvolvimento

Pré-requisitos: [Node.js](https://nodejs.org), [Rust](https://rustup.rs) e as Build Tools do Visual Studio (C++).

```sh
npm install
npm run tauri dev
```

## Gerar o instalador no seu Windows

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

## Lançar uma versão para Windows, Mac e Linux

Os instaladores de Mac e Linux só podem ser gerados nesses sistemas, então quem gera é o **GitHub Actions**:
computadores do GitHub que seguem as receitas da pasta `.github/workflows/`.

- `check.yml` roda os testes nos três sistemas a cada PR (aparece como ✅/❌ na página do PR) e a cada push na `main`.
- `release.yml` gera os instaladores quando você marca uma versão com um tag.

Para lançar:

1. Com tudo já na `main` (PR mergeado), aumente a versão nos três arquivos: `package.json`,
   `src-tauri/tauri.conf.json` e `src-tauri/Cargo.toml` (por exemplo de `0.1.0` para `0.2.0`). Faça o commit e o push.
2. Crie o tag com o mesmo número e envie:
   ```sh
   git checkout main
   git pull
   git tag v0.2.0
   git push origin v0.2.0
   ```
3. Acompanhe na aba **Actions** do repositório (leva uns 15 a 20 minutos).
4. Em **Releases** aparece um rascunho "AI Monitor v0.2.0" com o `.exe`, o `.dmg`, o `.deb` e o `.AppImage`.
   Confira e clique em **Publish release**. A partir daí qualquer pessoa pode baixar.

Se algo der errado no Actions, apague o rascunho da Release e o tag (`git push --delete origin v0.2.0` e
`git tag -d v0.2.0`), corrija e repita.

### Instalar no Mac

O app não é assinado pela Apple (isso exige uma conta paga). Na primeira vez:

1. Abra o `.dmg` e arraste o **AI Monitor** para **Aplicativos**.
2. Em Aplicativos, clique com o **botão direito** no AI Monitor → **Abrir** → **Abrir**. Nas versões mais novas do
   macOS, se não houver o botão: **Ajustes do Sistema → Privacidade e Segurança → "Abrir Mesmo Assim"**.
3. Se aparecer "o app está danificado", rode no Terminal e abra de novo:
   ```sh
   xattr -cr "/Applications/AI Monitor.app"
   ```

### Instalar no Linux

- Ubuntu/Debian: `sudo apt install ./AI.Monitor_<versão>_amd64.deb`
- Qualquer distribuição: dê permissão de execução ao `.AppImage` (`chmod +x AI.Monitor_*.AppImage`) e abra.
  Para a captura das porcentagens continuar funcionando, mantenha o `.AppImage` sempre na mesma pasta.

## Testes

```sh
cd src-tauri
cargo test
cd ..
npm test
```
