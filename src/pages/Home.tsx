import { useState } from "react";
import { ActiveSessions } from "../components/ActiveSessions";
import { Limits } from "../components/Limits";
import { OpencodeView } from "../components/OpencodeView";
import { dayKey, fmt } from "../format";
import type { AccountStatus, CaptureStatus, Commit, Limits as LimitsData, LiveSession, OpencodeUsage } from "../types";

type Agent = "claude" | "opencode";
const AGENT_KEY = "ai-monitor.agent";

function loadAgent(): Agent {
  try {
    return localStorage.getItem(AGENT_KEY) === "opencode" ? "opencode" : "claude";
  } catch {
    return "claude";
  }
}

/** Faixa fina com o resumo do GitHub; leva para a aba GitHub. */
function GithubStrip({
  prsWaiting,
  commits,
  now,
  onOpen,
}: {
  prsWaiting: number | null;
  commits: Commit[] | null;
  now: number;
  onOpen: () => void;
}) {
  const today = dayKey(now);
  const commitsToday = commits?.filter((c) => dayKey(c.ts) === today).length ?? null;
  const parts = [
    prsWaiting === null ? null : `${fmt.format(prsWaiting)} ${prsWaiting === 1 ? "PR esperando" : "PRs esperando"} você`,
    commitsToday === null ? null : `${fmt.format(commitsToday)} ${commitsToday === 1 ? "commit" : "commits"} hoje`,
  ].filter(Boolean);
  return (
    <button className={`github-strip${prsWaiting ? " has-waiting" : ""}`} onClick={onOpen}>
      <span className="github-strip-label">GitHub</span>
      <span>{parts.length > 0 ? parts.join(" · ") : "Carregando…"}</span>
      <span className="github-strip-go" aria-hidden="true">
        →
      </span>
    </button>
  );
}

export function Home({
  now,
  limits,
  capture,
  account,
  onEnableCapture,
  sessions,
  onSessionsChanged,
  opencode,
  prsWaiting,
  commits,
  onOpenGithub,
}: {
  now: number;
  limits: LimitsData | null;
  capture: CaptureStatus | null;
  account: AccountStatus | null;
  onEnableCapture: () => Promise<void>;
  sessions: LiveSession[];
  onSessionsChanged: () => void;
  opencode: OpencodeUsage | null;
  prsWaiting: number | null;
  commits: Commit[] | null;
  onOpenGithub: () => void;
}) {
  const [saved, setSaved] = useState<Agent>(loadAgent);
  // Só mostra a troca se houver outro agente instalado.
  const hasOpencode = opencode?.installed ?? false;
  const agent: Agent = hasOpencode ? saved : "claude";

  function choose(a: Agent) {
    setSaved(a);
    try {
      localStorage.setItem(AGENT_KEY, a);
    } catch {
      // Sem armazenamento, a escolha vale até fechar o app.
    }
  }

  return (
    <div className="home">
      <GithubStrip prsWaiting={prsWaiting} commits={commits} now={now} onOpen={onOpenGithub} />

      {hasOpencode && (
        <div className="segmented" role="tablist" aria-label="Agente">
          {(["claude", "opencode"] as const).map((a) => (
            <button key={a} role="tab" aria-selected={agent === a} className={agent === a ? "active" : ""} onClick={() => choose(a)}>
              {a === "claude" ? "Claude" : "opencode"}
              {a === "opencode" && opencode!.active.length > 0 && <span className="count">{opencode!.active.length}</span>}
              {a === "claude" && sessions.length > 0 && <span className="count">{sessions.length}</span>}
            </button>
          ))}
        </div>
      )}

      {agent === "claude" ? (
        <>
          <section className="section">
            <h2 className="section-title">Limites de uso</h2>
            <Limits limits={limits} capture={capture} account={account} onEnable={onEnableCapture} now={now} />
          </section>

          <section className="section">
            <h2 className="section-title">
              Sessões ativas {sessions.length > 0 && <span className="count">{sessions.length}</span>}
            </h2>
            <ActiveSessions sessions={sessions} now={now} onChanged={onSessionsChanged} />
          </section>
        </>
      ) : (
        <OpencodeView usage={opencode!} now={now} />
      )}
    </div>
  );
}
