import { useState } from "react";
import { baseName, compact, elapsed, fmt, severity } from "../format";
import type { AgentId, AgentPeriod, AgentSession, AgentUsage, Limits } from "../types";
import { LimitCard } from "./Limits";

export const AGENT_NAMES: Record<AgentId, string> = {
  claude: "Claude",
  opencode: "opencode",
  codex: "Codex",
  gemini: "Gemini",
};

/** O que cada agente mostra (ou não) sobre plano e gasto. */
const USAGE_NOTE: Partial<Record<AgentId, string>> = {
  opencode:
    "O opencode não informa porcentagem de plano; o que ele mostra é o contexto de cada sessão, que aparece abaixo. O gasto vem dos preços que o opencode registra (modelos grátis ficam em US$ 0).",
  gemini:
    "O Gemini não guarda a porcentagem do plano no computador; o que dá para ver é o contexto de cada sessão, que aparece abaixo.",
};

const usd = new Intl.NumberFormat("pt-BR", { style: "currency", currency: "USD" });

/** "opencode/nemotron-3-ultra-free" → "nemotron-3-ultra-free". */
const modelName = (m: string | null) => (m ? m.slice(m.indexOf("/") + 1) : "—");

const titleOf = (s: AgentSession) => s.title || "Sessão sem título";

function contextPercent(s: AgentSession) {
  return s.contextLimit ? (s.contextTokens / s.contextLimit) * 100 : null;
}

function Detail({ session, now }: { session: AgentSession; now: number }) {
  const pct = contextPercent(session);
  return (
    <div className="session-detail" id={`agent-${session.id}`}>
      <div className="session-detail-head">
        <div>
          <p className="session-status">
            <span className={`status-dot ${session.busy ? "status-busy" : "status-idle"}`} aria-hidden="true" />
            {session.busy ? "Trabalhando" : "Aguardando você"}
            <span className="muted-inline"> · última resposta {elapsed(session.lastActivity, now)}</span>
          </p>
          <h3 className="session-title">{titleOf(session)}</h3>
        </div>
      </div>
      <dl className="session-facts">
        <div className="fact-wide">
          <dt>Contexto usado</dt>
          <dd>
            {pct === null ? (
              <span>
                {compact.format(session.contextTokens)} tokens{" "}
                <span className="muted-inline">(o limite deste modelo não é conhecido)</span>
              </span>
            ) : (
              <span className={`context context-${severity(pct)}`}>
                <span className="context-bar">
                  <span style={{ width: `${Math.min(100, pct)}%` }} />
                </span>
                <span>
                  {compact.format(session.contextTokens)} de {compact.format(session.contextLimit!)} tokens{" "}
                  <span className="muted-inline">({Math.round(pct)}%)</span>
                </span>
              </span>
            )}
          </dd>
        </div>
        <div>
          <dt>Projeto</dt>
          <dd title={session.directory}>{session.directory ? baseName(session.directory) : "—"}</dd>
        </div>
        <div>
          <dt>Modelo</dt>
          <dd>{modelName(session.model)}</dd>
        </div>
        {session.cost !== null && (
          <div>
            <dt>Gasto</dt>
            <dd>{usd.format(session.cost)}</dd>
          </div>
        )}
        <div>
          <dt>Respostas</dt>
          <dd>{fmt.format(session.responses)}</dd>
        </div>
      </dl>
    </div>
  );
}

function PeriodCard({ title, period }: { title: string; period: AgentPeriod }) {
  return (
    <div className="oc-period">
      <h3>{title}</h3>
      <dl className="commit-stats">
        <div>
          <dt>Sessões</dt>
          <dd>{fmt.format(period.sessions)}</dd>
        </div>
        <div>
          <dt>Respostas</dt>
          <dd>{fmt.format(period.responses)}</dd>
        </div>
        {period.cost !== null && (
          <div>
            <dt>Gasto</dt>
            <dd>{usd.format(period.cost)}</dd>
          </div>
        )}
      </dl>
      <p className="muted">Modelo mais usado: {modelName(period.topModel)}</p>
    </div>
  );
}

/** % do plano que o Codex grava a cada resposta. */
function PlanLimits({ limits, name, now }: { limits: Limits | null; name: string; now: number }) {
  if (!limits || limits.windows.length === 0) {
    return (
      <div className="card limits-waiting">
        <p>As porcentagens do plano aparecem aqui depois da próxima resposta do {name}.</p>
      </div>
    );
  }
  return (
    <div className="limits">
      <div className="limits-grid">
        {limits.windows.map((w) => (
          <LimitCard key={w.id} w={w} now={now} />
        ))}
      </div>
      <p className="section-note">
        Atualizado {elapsed(limits.capturedAt, now)}, na última resposta do {name}. Uso pelo site ou por outro computador
        só aparece depois da próxima resposta aqui.
      </p>
    </div>
  );
}

/** Visão geral de opencode, Codex ou Gemini: % do plano (Codex), resumo do uso e sessões ativas. */
export function AgentView({ agent, usage, now }: { agent: AgentId; usage: AgentUsage | null; now: number }) {
  const [openId, setOpenId] = useState<string | null>(null);
  const name = AGENT_NAMES[agent];

  if (!usage) return <p className="empty-line">Lendo os dados do {name}…</p>;
  if (!usage.installed) {
    return (
      <div className="card limits-waiting">
        <p>
          Ainda não há dados de uso do {name} neste computador. Eles aparecem aqui sozinhos depois que você usar o{" "}
          {name}.
        </p>
      </div>
    );
  }

  const open = usage.active.find((s) => s.id === openId) ?? null;
  const note = USAGE_NOTE[agent];

  return (
    <>
      {usage.error && <p className="message message-error">{usage.error}</p>}
      {agent === "codex" && (
        <section className="section">
          <h2 className="section-title">Limites de uso</h2>
          <PlanLimits limits={usage.limits} name={name} now={now} />
        </section>
      )}

      <section className="section">
        <h2 className="section-title">Uso</h2>
        <div className="card oc-summary">
          <PeriodCard title="Hoje" period={usage.today} />
          <PeriodCard title="Últimos 7 dias" period={usage.week} />
        </div>
        {note && <p className="section-note">{note}</p>}
      </section>

      <section className="section">
        <h2 className="section-title">
          Sessões ativas {usage.active.length > 0 && <span className="count">{usage.active.length}</span>}
        </h2>
        {usage.active.length === 0 ? (
          <p className="empty-line">Nenhuma sessão do {name} respondeu nos últimos 10 minutos.</p>
        ) : (
          <div className="active-sessions">
            <ul className="pills">
              {usage.active.map((s) => {
                const selected = s.id === openId;
                const pct = contextPercent(s);
                return (
                  <li key={s.id}>
                    <button
                      className={`pill${selected ? " selected" : ""}`}
                      aria-expanded={selected}
                      aria-controls={`agent-${s.id}`}
                      title={titleOf(s)}
                      onClick={() => setOpenId(selected ? null : s.id)}
                    >
                      <span className={`status-dot ${s.busy ? "status-busy" : "status-idle"}`} aria-hidden="true" />
                      <span className="pill-title">{titleOf(s)}</span>
                      {pct !== null && <span className="pill-meta">{Math.round(pct)}%</span>}
                    </button>
                  </li>
                );
              })}
            </ul>
            {open && <Detail session={open} now={now} />}
          </div>
        )}
      </section>
    </>
  );
}
