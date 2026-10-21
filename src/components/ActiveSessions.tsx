import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { baseName, elapsed, fmt, severity } from "../format";
import type { LiveSession } from "../types";

const STATUS: Record<string, { label: string; className: string }> = {
  busy: { label: "Trabalhando", className: "status-busy" },
  idle: { label: "Aguardando você", className: "status-idle" },
};

const statusOf = (s: LiveSession) =>
  STATUS[s.status ?? ""] ?? { label: s.status ?? "Status desconhecido", className: "" };

const titleOf = (s: LiveSession) => s.title ?? s.name ?? "Sessão sem título";

function EndSession({ session, onEnded }: { session: LiveSession; onEnded: () => void }) {
  const [confirming, setConfirming] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function end() {
    try {
      await invoke("kill_session", { pid: session.pid, sessionId: session.sessionId });
      onEnded();
    } catch (e) {
      setError(String(e));
      setConfirming(false);
    }
  }

  if (!confirming) {
    return (
      <div className="end-session">
        {error && <span className="inline-error">{error}</span>}
        <button className="button button-quiet" onClick={() => setConfirming(true)}>
          Encerrar console
        </button>
      </div>
    );
  }
  return (
    <div className="end-session confirming" role="group" aria-label="Confirmar encerramento">
      <span>A conversa fica salva e pode ser retomada depois.</span>
      <button className="button button-danger" onClick={end}>
        Encerrar
      </button>
      <button className="button button-quiet" onClick={() => setConfirming(false)}>
        Cancelar
      </button>
    </div>
  );
}

function Detail({ session, now, onEnded }: { session: LiveSession; now: number; onEnded: () => void }) {
  const status = statusOf(session);
  const project = session.cwd ? baseName(session.cwd) : session.projectDir;
  const sub = session.subagentTotals.messages;
  const ctx = session.contextPercent;

  return (
    <div className="session-detail" id={`session-${session.pid}`}>
      <div className="session-detail-head">
        <div>
          <p className="session-status">
            <span className={`status-dot ${status.className}`} aria-hidden="true" />
            {status.label}
            {session.startedAt && <span className="muted-inline"> · aberta {elapsed(session.startedAt, now)}</span>}
          </p>
          <h3 className="session-title">{titleOf(session)}</h3>
        </div>
        <EndSession session={session} onEnded={onEnded} />
      </div>

      <dl className="session-facts">
        <div>
          <dt>Contexto usado</dt>
          <dd>
            {ctx === null ? (
              <span className="muted-inline">aparece com as porcentagens ativas</span>
            ) : (
              <span className={`context context-${severity(ctx)}`}>
                <span className="context-bar">
                  <span style={{ width: `${Math.min(100, ctx)}%` }} />
                </span>
                {Math.round(ctx)}%
              </span>
            )}
          </dd>
        </div>
        <div>
          <dt>Projeto</dt>
          <dd title={session.cwd ?? undefined}>{project ?? "—"}</dd>
        </div>
        <div>
          <dt>Branch</dt>
          <dd>{session.gitBranch ?? "—"}</dd>
        </div>
        <div>
          <dt>Modelo</dt>
          <dd>{session.model ?? "—"}</dd>
        </div>
        <div>
          <dt>Respostas</dt>
          <dd>
            {fmt.format(session.totals.messages)}
            {sub > 0 && <span className="muted-inline"> ({fmt.format(sub)} de subagentes)</span>}
          </dd>
        </div>
      </dl>
    </div>
  );
}

/** Pílulas com os consoles abertos; clicar numa mostra os detalhes logo abaixo. */
export function ActiveSessions({ sessions, now, onChanged }: { sessions: LiveSession[]; now: number; onChanged: () => void }) {
  const [openPid, setOpenPid] = useState<number | null>(null);
  const open = sessions.find((s) => s.pid === openPid) ?? null;

  if (sessions.length === 0) {
    return (
      <p className="empty-line">
        Nenhum console do Claude Code aberto. Rode <code>claude</code> num terminal e ele aparece aqui.
      </p>
    );
  }

  return (
    <div className="active-sessions">
      <ul className="pills">
        {sessions.map((s) => {
          const status = statusOf(s);
          const selected = s.pid === openPid;
          return (
            <li key={s.pid}>
              <button
                className={`pill${selected ? " selected" : ""}`}
                aria-expanded={selected}
                aria-controls={`session-${s.pid}`}
                title={`${titleOf(s)}: ${status.label}`}
                onClick={() => setOpenPid(selected ? null : s.pid)}
              >
                <span className={`status-dot ${status.className}`} aria-hidden="true" />
                <span className="pill-title">{titleOf(s)}</span>
                {s.contextPercent !== null && <span className="pill-meta">{Math.round(s.contextPercent)}%</span>}
              </button>
            </li>
          );
        })}
      </ul>
      {open && <Detail session={open} now={now} onEnded={() => (setOpenPid(null), onChanged())} />}
    </div>
  );
}
