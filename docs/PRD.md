# AI Monitor — PRD

Software desktop (Windows) que funciona como central de verificação do uso do **Claude Code**. 100% local: sem servidor, sem login, sem nuvem (exceto a atividade do GitHub, opcional).

## Stack

- **Tauri 2**: backend em Rust (leitura de arquivos, file watcher, SQLite), UI em React + TypeScript.
- Plataforma inicial: Windows. Arquitetura sem dependências que impeçam portar para macOS/Linux depois.

## Fontes de dados

| Dado | Origem |
|---|---|
| Uso (tokens, modelo, projeto, branch, ferramentas, skills) | `~/.claude/projects/<projeto>/<sessionId>.jsonl` — campo `usage` de cada resposta |
| Consoles abertos agora | `~/.claude/sessions/<pid>.json` |
| % da janela de 5h e da semana | `rate_limits.five_hour` / `rate_limits.seven_day` entregues à statusline do Claude Code (apenas Pro/Max) |

- O app observa `~/.claude` em tempo real (file watcher) e indexa de forma incremental.
- Banco SQLite local guarda **apenas métricas e metadados por resposta** (nunca o conteúdo das conversas). Estimativa: ~150 bytes/resposta, ~2 MB/ano em uso intenso. Isso preserva o histórico após a limpeza automática do Claude Code (`cleanupPeriodDays`, padrão 30 dias).
- Cada resposta aparece em várias linhas do `.jsonl` (uma por bloco de conteúdo); o indexador deduplica por `message.id` + `requestId`.
- O conteúdo das conversas é lido diretamente do `.jsonl` enquanto ele existir.

## Uso da conta (opcional, desligado por padrão)

- Os limites do plano (5 h e semana) são da conta: navegador, app e Claude Code somam juntos. A statusline só traz número novo quando o Claude Code responde, então uso só no navegador ficava desatualizado.
- Com a opção ligada em Configurações, o app consulta `GET https://api.anthropic.com/api/oauth/usage` (a mesma consulta do `/usage` do Claude Code, **não documentada**) a cada 5 min, em segundo plano, com o token de `~/.claude/.credentials.json`. O token só é lido: não é exibido, copiado, gravado nem renovado pelo app (quem renova é o Claude Code). Expirado → aviso para usar o Claude Code.
- Os anéis mostram o número mais recente entre statusline e esta consulta. Abaixo deles, a divisão do uso da semana por produto (`seven_day_breakdown`: Claude Code, Chats, Cowork…).
- Conversas, tokens e sessões do navegador continuam fora do escopo (não há API).

## Captura de limites (statusline)

- Ativada **somente com consentimento explícito** ("Ativar captura de limites").
- Faz backup do `settings.json` antes de alterar; instala um script que grava `rate_limits` num arquivo observado pelo app e repassa a saída para a statusline original do usuário, se houver.
- "Desativar" restaura a statusline original.
- Limitação: o valor só atualiza enquanto alguma sessão está respondendo; fora disso, mostra o último valor conhecido e o horário de reset.

## Navegação

Barra superior com as abas (Visão geral, GitHub, Configurações); a aba GitHub mostra quantos PRs esperam a sua revisão. Acima do conteúdo: uma **barra de status** fina com data, hora e bateria do computador, e abaixo dela a **faixa do bichinho** (ver seção própria).

A interface mostra **porcentagens, não tokens** (decisão do usuário em 2026-10-21): os tokens continuam no banco para o custo estimado do M4.

## Telas

1. **Visão geral**:
   - **Limites de uso**: anéis com a % da sessão de 5 h, da semana (e de Opus/Sonnet na semana, quando o Claude Code informa) e quando cada um reinicia. Sem a captura ligada, mostra o convite para ativá-la.
   - **Sessões ativas**: pílulas compactas (status, título, % de contexto), como os tópicos da página inicial do GitHub; clicar abre os detalhes (status, tempo aberta, contexto usado como "X de Y tokens (Z%)" com o tamanho da janela do modelo, projeto, branch, modelo, respostas) e o botão de encerrar.
   - **Faixa do GitHub** no topo: "N PRs esperando você · N commits hoje" (commits locais de hoje); leva à aba GitHub. Substitui o card "Seus commits".
   - **Seletor de agente** "Claude | opencode" (aparece só com o opencode instalado; lembra a escolha; cada botão mostra quantas sessões ativas tem). Claude: limites, divisão por produto e sessões ativas, como antes.
   - **opencode** (lido só para leitura de `~/.local/share/opencode/opencode.db`):
     - **Uso** de hoje e dos últimos 7 dias: sessões, respostas, gasto em US$ (o que o opencode registra) e modelo mais usado. Subagentes contam na sessão principal.
     - **Sessões ativas**: sessões com resposta gravada nos últimos 10 min (uma resposta interrompida não fica ativa para sempre), em pílulas com o % de contexto; o detalhe mostra trabalhando/aguardando, contexto "X de Y tokens (Z%)" (tokens da última resposta / limite do modelo em `~/.cache/opencode/models.json`, a mesma conta da tela do opencode), projeto, modelo, gasto e respostas. Sem encerrar.
     - O opencode não informa % de plano. As análises de PR feitas pelo app aparecem no opencode com o título "AI Monitor: análise de …".
   - A barrinha lateral continua só com o Claude.
2. **Uso**: gráficos e tabelas filtráveis por projeto, modelo, ferramentas e skills (classificação determinística, sem IA).
3. **Histórico de sessões**: lista de chats (título/primeiro prompt, projeto, data, duração, tokens, custo), busca, visualizador somente leitura e botão **Retomar** (abre o Windows Terminal, com fallback para PowerShell, na pasta do projeto executando `claude --resume <id>`).
   - Uso de subagentes soma na sessão-mãe, com detalhamento "dos quais X em subagentes".
   - Sessões cujo `.jsonl` foi apagado continuam listadas como "conteúdo indisponível".
4. **Sessões ativas**: um painel por console do Claude Code aberto agora, lido de `~/.claude/sessions/<pid>.json` (com verificação de que o processo ainda existe, pelo horário de início, para não confundir PIDs reaproveitados). Cada painel mostra status ao vivo (trabalhando / aguardando você), tempo aberto, projeto, branch, modelo, tamanho do contexto atual, tokens por tipo (incluindo subagentes), número de respostas, ferramentas mais usadas e o último prompt. Ação: **encerrar** o console, com confirmação. Trazer a janela do terminal para frente está fora do escopo.
5. **GitHub** (antes "Commits"): no topo, o gráfico de contribuições com os números rápidos; abaixo, um seletor **Pull requests | Commits** que mostra uma coisa por vez, para não encher a tela.
   - **Commits locais**: `git log` (90 dias, branches locais) dos repositórios onde o Claude Code já trabalhou (pastas conhecidas pelas sessões) + pastas adicionadas manualmente (pendente). São "do usuário" os commits com os e-mails configurados no git e com os e-mails usados em commits assinados com o nome configurado (cobre e-mails antigos). Mostra mensagem, repositório, branch, data e se já foi enviado (push), agrupados por dia.
   - **Gráfico de contribuições**: o mesmo do perfil do GitHub, lido via `gh api graphql` (o `gh` já autenticado; o app não guarda token). Atualiza ao abrir, a cada 10 min e no botão "Atualizar". Exceção consciente ao "100% local". Sem `gh`, a grade usa os commits locais e avisa. Os commits locais e o gráfico do GitHub contam coisas diferentes (o GitHub inclui merges, PRs e atividade de outros computadores; os locais incluem commits ainda não enviados).
   - Commits locais são relidos ~1 s após commit, push ou troca de branch (observador nas pastas `.git`), além da releitura a cada 60 s.
   - **Commits** aparecem agrupados por dia, 7 dias de cada vez, com "Ver mais".
   - **Pull requests** (via `gh api graphql`, atualiza ao abrir o app, ao abrir a aba, a cada 5 min e no botão "Atualizar"):
     - Duas listas: **Esperando sua revisão** (`review-requested:@me`) e **Seus PRs abertos** (`author:@me`), em linhas compactas (título, repositório#número, autor, atualização, resumo dos checks).
     - Clicar num PR abre um painel ao lado com descrição, branches, situação da revisão, +/−, checks e arquivos alterados, e "Abrir no GitHub".
     - **Ações** no painel, sempre com confirmação (aparecem para outras pessoas e não se desfazem pelo app):
       - PRs dos outros: **Aprovar** (comentário opcional) e **Pedir mudanças** (comentário obrigatório), via `gh pr review`.
       - Seus PRs: **Fazer merge** via `gh pr merge`, com a primeira forma que o repositório permite (merge, squash, rebase) e `--match-head-commit` (só junta se o PR ainda estiver no commit visto no app). Bloqueado em rascunhos e PRs com conflito, com o motivo na tela. A branch não é apagada.
     - **Análise** no painel: botão "Analisar" com seletor de quem analisa (Claude Haiku/Sonnet/Opus ou opencode) e desempenho (`--effort`: baixo, médio, alto, muito alto, máximo). O padrão fica em Configurações (começa em Sonnet + médio) e a última escolha feita num PR vira o novo padrão. Avisa que o Claude gasta do limite.
       - Claude: `claude -p --no-session-persistence` com o prompt e o diff pela entrada padrão; ferramentas só `Read,Grep,Glob` (nenhuma sem o repositório local).
       - opencode: `opencode run --agent plan --format json --dir <pasta>` (o opencode ignora a pasta do processo) com o diff anexado e `OPENCODE_CONFIG_CONTENT` negando comandos, edição e internet; a resposta é o texto da última etapa.
       - Os dois rodam dentro do repositório quando ele está no computador (pasta de sessão com remoto apontando para o PR); senão, numa pasta vazia, só com o diff (`gh pr diff`, cortado em 150 KB). O app chama o `.exe` real (resolve o `.cmd` do npm), com limite de 15 min.
       - Formato pedido: VEREDITO (Pronto / Precisa de ajustes / Arriscado), resumo, problemas e sugestões. O parecer fica salvo no banco (tabela `pr_analyses`, o último por PR) e aparece como "desatualizada" quando o PR recebe commits novos. Uma análise em andamento continua se a pessoa trocar de aba.
       - Sem notificações do Windows por enquanto.
6. **Configurações**: bichinho, GitHub, pastas, perfis, alertas, idioma, inicialização com o Windows, statusline, oferta (com botão e explicação) de aumentar `cleanupPeriodDays`.

## Perfis

- A pasta `~/.claude` é detectada automaticamente; pastas adicionais (outro `CLAUDE_CONFIG_DIR`, WSL) podem ser adicionadas.
- O usuário cadastra manualmente o nome do perfil e o plano (Pro, Max 5x, Max 20x), usado para contextualizar o custo estimado.

## Custo estimado

- Tokens × preço de tabela da API, com a tabela **embutida no app** (atualizada por versão).
- Modelos desconhecidos aparecem como "preço desconhecido", nunca estimados por chute.
- Exibição em USD, com opção de BRL.

## Bandeja e alertas

- Ícone na bandeja com cor verde/amarelo/vermelho conforme o maior percentual de limite.
- Ao clicar/passar o mouse: % de 5h, % semanal, contagem até os resets e a sessão ativa.
- Notificações nativas do Windows ao cruzar 75% e 90% (configuráveis, ligadas por padrão) e quando a janela de 5h reseta.
- Inicia com o Windows, minimizado na bandeja (desativável). Fechar a janela apenas a esconde.

## Barrinha de uso (opcional)

- Ligada em Configurações (desligada por padrão; preferência salva em `ui.json` na pasta de dados do app).
- Ao minimizar ou fechar a janela principal, aparece uma janela pequena, sem bordas e sempre visível, na borda direita da tela, com a % da sessão de 5 h e da semana. Pode ser arrastada; o botão dela reabre o app. Some quando a janela principal volta.

## Uma cópia só

- Abrir o app de novo (atalho, menu Iniciar) com ele já rodando apenas traz a janela existente de volta, em vez de iniciar uma segunda cópia.

## Bichinho

- Desenhado no próprio app (SVG), sem imagens externas. Personalizável: espécie (gato, cachorro, raposa, pato), cor e nome. Pode ser desligado.
- Clicar na faixa superior joga uma bolinha (com quique); o bichinho corre até ela e a pega.
- Sem bolinha: passeia sem parar, de um ponto a outro da faixa (os GIFs são ciclos de corrida; parado, o GIF ficaria congelado). Após 4 min sem brincadeira, cochila por 10–15 s.
- Com "reduzir movimento" do Windows ligado, ele só se move quando a bolinha é jogada.

## Idioma

- i18n desde o início: PT-BR (padrão) e EN.

## Distribuição

- Windows, Mac e Linux. Instaladores gerados pelo GitHub Actions (`.github/workflows/release.yml`) ao enviar um tag `v*`: `.exe` (NSIS), `.dmg` universal (Apple Silicon e Intel), `.deb` e `.AppImage`, numa Release em rascunho que o usuário publica. `check.yml` roda os testes nos três sistemas em todo PR.
- Sem assinatura de código: o Windows SmartScreen exibe um aviso; no Mac o app tem só a assinatura ad-hoc (gratuita) e precisa ser liberado na primeira abertura (README). Decisão do usuário (2026-10-22): não pagar a conta de desenvolvedor da Apple.
- Diferenças fora do Windows, todas atrás de `cfg`: encerrar sessão manda SIGTERM ao console e aos processos filhos (no Windows, `taskkill /T`); a conferência do PID é pelo nome do processo (o `procStart` só é lido no formato do Windows); `claude`/`opencode` são achados sem extensão no PATH; o PATH do app é completado com o do shell do usuário e pastas comuns (apps abertos pelo Finder/menu não herdam o do terminal); no AppImage a statusline aponta para o arquivo `.AppImage` (variável `APPIMAGE`); no Mac, clicar no Dock reabre a janela.
- Auto-update pelo updater do Tauri: ainda não implementado.

## Fora do escopo

- Automações / tarefas agendadas.
- Focar a janela/aba do terminal de uma sessão.
- claude.ai (chats do site/app) e API/Console (Admin API).
- Categorização de temas por IA.
- Editar ou excluir sessões.
- macOS e Linux (por enquanto).

## Marcos

| Marco | Entrega | Verificação |
|---|---|---|
| **M1** ✅ | Esqueleto Tauri + bandeja + indexador `.jsonl` → SQLite | Testes automatizados; totais conferidos com script independente |
| **M2** ✅ | Barra lateral + Sessões ativas + bichinho (GIFs importados) | Sessões abertas aparecem e somem; encerrar funciona; bichinho persegue a bolinha |
| **M3** | Commits locais ✅ + % de limites e contexto via statusline ✅ (adiantado do M5) + barra de status ✅ + gráfico do GitHub ✅ + aba GitHub com PRs, ações e análise ✅ + opencode na Visão geral ✅ | Commits batem com `git log`; % aparece após uma resposta |
| **M4** | Custo estimado + gráficos + histórico de sessões com visualizador e Retomar | Números batem com os `.jsonl`; retomar abre o terminal |
| **M5** | Alertas + bandeja colorida | % aparece após uma resposta; notificação dispara |
| **M6** | Perfis, i18n, auto-start, instalador, auto-update | Instalar via `.msi` numa máquina limpa |
