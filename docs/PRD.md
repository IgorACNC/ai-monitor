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

## Captura de limites (statusline)

- Ativada **somente com consentimento explícito** ("Ativar captura de limites").
- Faz backup do `settings.json` antes de alterar; instala um script que grava `rate_limits` num arquivo observado pelo app e repassa a saída para a statusline original do usuário, se houver.
- "Desativar" restaura a statusline original.
- Limitação: o valor só atualiza enquanto alguma sessão está respondendo; fora disso, mostra o último valor conhecido e o horário de reset.

## Navegação

Barra lateral (Visão geral, Commits, Configurações). Acima do conteúdo: uma **barra de status** fina com data, hora e bateria do computador, e abaixo dela a **faixa do bichinho** (ver seção própria).

A interface mostra **porcentagens, não tokens** (decisão do usuário em 2026-10-21): os tokens continuam no banco para o custo estimado do M4.

## Telas

1. **Visão geral**:
   - **Limites de uso**: anéis com a % da sessão de 5 h, da semana (e de Opus/Sonnet na semana, quando o Claude Code informa) e quando cada um reinicia. Sem a captura ligada, mostra o convite para ativá-la.
   - **Sessões ativas**: pílulas compactas (status, título, % de contexto), como os tópicos da página inicial do GitHub; clicar abre os detalhes (status, tempo aberta, contexto usado como "X de Y tokens (Z%)" com o tamanho da janela do modelo, projeto, branch, modelo, respostas) e o botão de encerrar.
   - **Seus commits**: grade de contribuições de 13 semanas, números rápidos (últimos 7 dias, dias seguidos, não enviados) e os 5 commits mais recentes.
2. **Uso**: gráficos e tabelas filtráveis por projeto, modelo, ferramentas e skills (classificação determinística, sem IA).
3. **Histórico de sessões**: lista de chats (título/primeiro prompt, projeto, data, duração, tokens, custo), busca, visualizador somente leitura e botão **Retomar** (abre o Windows Terminal, com fallback para PowerShell, na pasta do projeto executando `claude --resume <id>`).
   - Uso de subagentes soma na sessão-mãe, com detalhamento "dos quais X em subagentes".
   - Sessões cujo `.jsonl` foi apagado continuam listadas como "conteúdo indisponível".
4. **Sessões ativas**: um painel por console do Claude Code aberto agora, lido de `~/.claude/sessions/<pid>.json` (com verificação de que o processo ainda existe, pelo horário de início, para não confundir PIDs reaproveitados). Cada painel mostra status ao vivo (trabalhando / aguardando você), tempo aberto, projeto, branch, modelo, tamanho do contexto atual, tokens por tipo (incluindo subagentes), número de respostas, ferramentas mais usadas e o último prompt. Ação: **encerrar** o console, com confirmação. Trazer a janela do terminal para frente está fora do escopo.
5. **Commits**:
   - **Commits locais**: `git log` (90 dias, branches locais) dos repositórios onde o Claude Code já trabalhou (pastas conhecidas pelas sessões) + pastas adicionadas manualmente (pendente). São "do usuário" os commits com os e-mails configurados no git e com os e-mails usados em commits assinados com o nome configurado (cobre e-mails antigos). Mostra mensagem, repositório, branch, data e se já foi enviado (push), agrupados por dia.
   - **Atividade do GitHub** (opcional, exceção ao 100% local, ligada nas configurações): feed de eventos da conta via `gh` já autenticado (sem guardar token); fallback: atividade pública por nome de usuário.
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

- Instalador `.msi`/`.exe` no GitHub Releases, auto-update pelo updater do Tauri, repositório público com licença MIT.
- Sem assinatura de código no início (o Windows SmartScreen exibirá um aviso).

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
| **M3** | Commits locais ✅ + % de limites e contexto via statusline ✅ (adiantado do M5) + barra de status; falta: atividade do GitHub via `gh` | Commits batem com `git log`; % aparece após uma resposta |
| **M4** | Custo estimado + gráficos + histórico de sessões com visualizador e Retomar | Números batem com os `.jsonl`; retomar abre o terminal |
| **M5** | Alertas + bandeja colorida | % aparece após uma resposta; notificação dispara |
| **M6** | Perfis, i18n, auto-start, instalador, auto-update | Instalar via `.msi` numa máquina limpa |
