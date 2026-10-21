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
  /** Do contexto ocupado, pela última statusline; `null` sem a captura ativa. */
  contextPercent: number | null;
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
