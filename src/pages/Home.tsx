import { useEffect, useRef, useState } from "react";
import { ActiveSessions } from "../components/ActiveSessions";
import { AGENT_NAMES, AgentView } from "../components/AgentView";
import { Limits } from "../components/Limits";
import { dayKey, fmt } from "../format";
import type {
  AccountStatus,
  AgentId,
  AgentInfo,
  AgentUsage,
  CaptureStatus,
  Commit,
  Limits as LimitsData,
  LiveSession,
} from "../types";

const AGENT_KEY = "ai-monitor.agent";

function loadAgent(): string | null {
  try {
    return localStorage.getItem(AGENT_KEY);
  } catch {
    return null;
  }
}

/** Botão + com a lista dos agentes ainda não conectados. */
function AddAgent({
  agents,
  onConnect,
  label,
}: {
  agents: AgentInfo[];
  onConnect: (id: AgentId) => Promise<void>;
  label?: string;
}) {
  const [open, setOpen] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const box = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const close = (e: Event) => {
      if (e instanceof KeyboardEvent ? e.key === "Escape" : !box.current?.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", close);
    document.addEventListener("keydown", close);
    return () => {
      document.removeEventListener("mousedown", close);
      document.removeEventListener("keydown", close);
    };
  }, [open]);

  async function connect(id: AgentId) {
    setError(null);
    try {
      await onConnect(id);
      setOpen(false);
    } catch (e) {
      setError(String(e));
    }
  }

  return (
    <div className="agent-add" ref={box}>
      <button
        className={label ? "button button-primary" : "agent-add-button"}
        aria-haspopup="menu"
        aria-expanded={open}
        title="Conectar um agente"
        aria-label={label ? undefined : "Conectar um agente"}
        onClick={() => setOpen(!open)}
      >
        {label ?? "+"}
      </button>
      {open && (
        <div className="agent-add-menu card" role="menu">
          {agents.map((a) => (
            <button key={a.id} role="menuitem" onClick={() => connect(a.id)}>
              <span>{AGENT_NAMES[a.id]}</span>
              <span className={a.found ? "agent-found" : "muted-inline"}>
                {a.found ? "encontrado neste computador" : "não encontrado"}
              </span>
            </button>
          ))}
          {error && <p className="inline-error">{error}</p>}
        </div>
      )}
    </div>
  );
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
  agents,
  usage,
  onConnect,
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
  agents: AgentInfo[] | null;
  usage: Partial<Record<AgentId, AgentUsage>>;
  onConnect: (id: AgentId) => Promise<void>;
  prsWaiting: number | null;
  commits: Commit[] | null;
  onOpenGithub: () => void;
}) {
  const [saved, setSaved] = useState<string | null>(loadAgent);
  const connected = (agents ?? []).filter((a) => a.connected).map((a) => a.id);
  const notConnected = (agents ?? []).filter((a) => !a.connected);
  // A escolha salva vale enquanto o agente estiver conectado.
  const agent = connected.find((a) => a === saved) ?? connected[0] ?? null;

  function choose(a: AgentId) {
    setSaved(a);
    try {
      localStorage.setItem(AGENT_KEY, a);
    } catch {
      // Sem armazenamento, a escolha vale até fechar o app.
    }
  }

  async function connect(a: AgentId) {
    await onConnect(a);
    choose(a);
  }

  const count = (a: AgentId) => (a === "claude" ? sessions.length : (usage[a]?.active.length ?? 0));

  return (
    <div className="home">
      <GithubStrip prsWaiting={prsWaiting} commits={commits} now={now} onOpen={onOpenGithub} />

      {agents !== null && connected.length === 0 && (
        <div className="card limits-setup">
          <div>
            <h3>Conecte um agente para ver o uso</h3>
            <p>Claude Code, opencode, Codex ou Gemini: escolha os que você usa neste computador.</p>
          </div>
          <AddAgent agents={notConnected} onConnect={connect} label="Conectar agente" />
        </div>
      )}

      {connected.length > 0 && (
        <div className="agent-bar">
          <div className="segmented" role="tablist" aria-label="Agente">
            {connected.map((a) => (
              <button key={a} role="tab" aria-selected={agent === a} className={agent === a ? "active" : ""} onClick={() => choose(a)}>
                {AGENT_NAMES[a]}
                {count(a) > 0 && <span className="count">{count(a)}</span>}
              </button>
            ))}
          </div>
          {notConnected.length > 0 && <AddAgent agents={notConnected} onConnect={connect} />}
        </div>
      )}

      {agent === "claude" && (
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
      )}
      {agent !== null && agent !== "claude" && <AgentView key={agent} agent={agent} usage={usage[agent] ?? null} now={now} />}
    </div>
  );
}
