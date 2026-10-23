# AI Monitor

Um app para o computador (Windows, Mac e Linux) que mostra quanto você já usou do **Claude Code**, quais
conversas estão abertas agora e como anda o seu GitHub, tudo numa janela só. Ele roda no seu computador e lê os
dados que o Claude Code já guarda; não tem conta, login nem servidor.

## O que ele mostra

- **Limites de uso**: a porcentagem já usada da sessão de 5 horas e da semana, e quando cada uma reinicia.
- **Sessões ativas**: os terminais do Claude Code abertos agora, com o quanto do contexto cada um já usou. Dá para
  encerrar um terminal por ali.
- **opencode**: se você usa o opencode, uma aba mostra o uso dele e as sessões abertas.
- **GitHub**: seu gráfico de contribuições, seus commits e os pull requests que esperam você. Dá para aprovar,
  pedir mudanças, fazer merge e pedir para o Claude (ou o opencode) analisar um PR.
- **Barrinha de uso** (opcional): uma barra pequena no canto da tela com as porcentagens quando o app está minimizado.
- **Um bichinho** na barra de cima que corre atrás da bolinha. Dá para trocar por qualquer GIF.

## Baixar

Abra a página da versão mais recente e baixe o arquivo do seu sistema em **Assets**:

**[github.com/IgorACNC/ai-monitor/releases/latest](https://github.com/IgorACNC/ai-monitor/releases/latest)**

| Sistema | Arquivo |
|---|---|
| Windows | o que termina em `_x64-setup.exe` |
| Mac (chip Apple ou Intel) | o que termina em `_universal.dmg` |
| Linux (Ubuntu, Debian e derivados) | o que termina em `_amd64.deb` |
| Linux (qualquer distribuição) | o que termina em `_amd64.AppImage` |

## Instalar

### Windows

1. Dê dois cliques no arquivo `.exe` baixado e avance até o fim.
2. Se aparecer **"O Windows protegeu o seu PC"**, clique em **Mais informações → Executar assim mesmo**. O aviso
   aparece porque o app não tem um certificado pago de assinatura; não é um problema no arquivo.
3. Abra o **AI Monitor** pelo menu Iniciar.

### Mac

1. Abra o `.dmg` e arraste o **AI Monitor** para a pasta **Aplicativos**.
2. Na primeira vez, em Aplicativos, clique com o **botão direito** no AI Monitor → **Abrir** → **Abrir**.
   Se não aparecer essa opção: **Ajustes do Sistema → Privacidade e Segurança**, role até o fim e clique em
   **"Abrir Mesmo Assim"**.
3. Se o Mac disser que **"o app está danificado"**, abra o **Terminal**, cole o comando abaixo e abra o app de novo:
   ```sh
   xattr -cr "/Applications/AI Monitor.app"
   ```

Isso só é preciso na primeira vez. Acontece porque o app não é assinado pela Apple (a assinatura exige uma conta
paga), não por haver algo errado com ele.

### Linux

- **`.deb`** (Ubuntu, Debian, Mint, Pop!_OS...): no terminal, na pasta onde o arquivo foi baixado,
  ```sh
  sudo apt install ./AI.Monitor_*_amd64.deb
  ```
  Depois abra o **AI Monitor** pelo menu de aplicativos.
- **`.AppImage`** (qualquer distribuição): dê permissão de execução e abra:
  ```sh
  chmod +x AI.Monitor_*_amd64.AppImage
  ./AI.Monitor_*_amd64.AppImage
  ```
  Deixe o arquivo sempre na mesma pasta: se ele mudar de lugar, as porcentagens de uso param de chegar até você
  abrir o app de novo.

## Primeiros passos

- **Precisa ter o [Claude Code](https://claude.com/claude-code) instalado e já usado pelo menos uma vez** neste
  computador. É dele que vêm os dados.
- **Porcentagens de uso**: na primeira abertura, clique em **"Mostrar porcentagens"**. O app passa a ler os números
  que o Claude Code informa a cada resposta. A statusline que você já usa continua funcionando, e um backup é feito
  antes. Dá para desligar em Configurações.
- **Uso da conta inteira (navegador incluído)**: em **Configurações → Uso da conta**, ligue a opção para o número
  incluir o que você usa no Claude pelo navegador e pelo app.
- **GitHub**: as partes de GitHub usam o [GitHub CLI](https://cli.github.com). Instale e rode uma vez no terminal:
  ```sh
  gh auth login
  ```
  Os commits aparecem mesmo sem isso; o gráfico do perfil e os pull requests precisam do `gh`.
- **Barrinha de uso**: ligue em **Configurações → Barrinha de uso**.
- **Fechar de verdade**: fechar a janela só esconde o app, que continua perto do relógio (bandeja). Para sair, clique
  com o botão direito no ícone dele → **Sair**.

## Atualizar

Baixe o instalador da versão nova na mesma página de [releases](https://github.com/IgorACNC/ai-monitor/releases/latest)
e instale por cima. Feche o app antes (botão direito no ícone perto do relógio → **Sair**). Configurações, histórico
e bichinhos continuam.

## Privacidade

O app funciona no seu computador e não envia seus dados para nenhum servidor próprio. Ele só se conecta à internet
quando você liga uma destas opções:

- **Uso da conta**: consulta suas porcentagens na Anthropic usando o login que o Claude Code já tem. O login não é
  copiado nem enviado para outro lugar.
- **GitHub**: usa o `gh` que você mesmo conectou à sua conta.
- **Analisar PR**: roda o Claude Code ou o opencode do seu computador, só com permissão de leitura do código. A análise
  com o Claude gasta do seu limite de uso.

## Problemas e sugestões

Abra uma [issue](https://github.com/IgorACNC/ai-monitor/issues) contando o que aconteceu e qual é o seu sistema.
