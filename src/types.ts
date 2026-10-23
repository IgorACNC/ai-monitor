/** Console do Claude Code aberto agora. */
export type LiveSession = {
  pid: number;
  sessionId: string;
  cwd: string | null;
  name: string | null;
  status: string | null;
  startedAt: number | null;
  title: string | null;
  projectDir: string | null;
  gitBranch: string | null;
  model: string | null;
  totals: { messages: number };
  subagentTotals: { messages: number };
  /** Contexto ocupado, pela última statusline; `null` sem a captura ativa. */
  context: ContextUsage | null;
};

export type ContextUsage = {
  percent: number;
  usedTokens: number | null;
  /** Tamanho da janela de contexto do modelo. */
  windowSize: number | null;
};

export type LimitWindow = {
  id: string;
  label: string;
  usedPercentage: number;
  resetsAt: number | null;
};

export type Limits = { capturedAt: number; windows: LimitWindow[] };

export type CaptureStatus = { enabled: boolean };

export type Commit = {
  sha: string;
  repo: string;
  branch: string | null;
  ts: number;
  subject: string;
  /** `null` quando o repositório não tem remoto. */
  pushed: boolean | null;
};

/** Gráfico de contribuições do perfil no GitHub. */
export type GithubCalendar = {
  login: string;
  total: number;
  days: { date: string; count: number }[];
};

/** Uso da conta consultado direto na Anthropic (inclui navegador e app do Claude). */
export type AccountStatus = {
  enabled: boolean;
  /** Parte do uso da semana que veio de cada produto. */
  breakdown: { key: string; label: string; percent: number }[];
  fetchedAt: number | null;
  error: string | null;
};

export type CheckOutcome = "success" | "failure" | "pending" | "neutral";

/** PR aberto no GitHub, lido pelo `gh`. */
export type PullRequest = {
  repo: string;
  number: number;
  title: string;
  url: string;
  author: string;
  draft: boolean;
  createdAt: number;
  updatedAt: number;
  body: string;
  head: string;
  base: string;
  reviewDecision: string | null;
  headSha: string;
  checks: CheckOutcome | null;
  checkRuns: { name: string; outcome: CheckOutcome }[];
  additions: number;
  deletions: number;
  changedFiles: number;
  files: { path: string; additions: number; deletions: number }[];
  /** "MERGEABLE", "CONFLICTING" ou "UNKNOWN" (o GitHub ainda está calculando). */
  mergeable: string;
  /** Forma de merge que o app usa: a primeira que o repositório permite. */
  mergeMethod: MergeMethod | null;
};

export type MergeMethod = "merge" | "squash" | "rebase";

export type PullRequests = {
  /** De outras pessoas, pedindo a sua revisão. */
  reviewRequested: PullRequest[];
  /** Abertos por você. */
  mine: PullRequest[];
};

/** Quem analisa um PR: o Claude Code (modelo e desempenho) ou o opencode. */
export type Analyzer = { agent: "claude"; model: string; effort: string } | { agent: "opencode" };

export type Analysis = {
  repo: string;
  number: number;
  /** Commit analisado; diferente do atual quer dizer que o PR mudou depois. */
  headSha: string;
  analyzer: Analyzer;
  verdict: "pronto" | "ajustes" | "arriscado" | null;
  text: string;
  localRepo: boolean;
  createdAt: number;
};

export type AgentId = "claude" | "opencode" | "codex" | "gemini";

export type AgentInfo = {
  id: AgentId;
  connected: boolean;
  /** Há sinal dele neste computador (pasta de dados ou programa instalado). */
  found: boolean;
};

/** Sessão do opencode, Codex ou Gemini com resposta nos últimos minutos. */
export type AgentSession = {
  id: string;
  title: string;
  directory: string;
  model: string | null;
  busy: boolean;
  startedAt: number;
  lastActivity: number;
  responses: number;
  /** Em dólares; só o opencode registra. */
  cost: number | null;
  contextTokens: number;
  /** `null` quando o limite do modelo não é conhecido. */
  contextLimit: number | null;
};

export type AgentPeriod = {
  sessions: number;
  responses: number;
  cost: number | null;
  topModel: string | null;
};

export type AgentUsage = {
  /** Há dados dele neste computador. */
  installed: boolean;
  active: AgentSession[];
  today: AgentPeriod;
  week: AgentPeriod;
  /** % do plano; só o Codex informa. */
  limits: Limits | null;
  error: string | null;
};
