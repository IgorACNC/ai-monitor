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
