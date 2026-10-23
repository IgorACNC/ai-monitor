import { useState } from "react";
import { elapsed, resetsIn, severity } from "../format";
import type { AccountStatus, CaptureStatus, LimitWindow, Limits as LimitsData } from "../types";

const RADIUS = 34;
const CIRCUMFERENCE = 2 * Math.PI * RADIUS;

function Ring({ pct }: { pct: number }) {
  const clamped = Math.min(100, Math.max(0, pct));
  return (
    <svg viewBox="0 0 80 80" className={`ring ring-${severity(pct)}`} aria-hidden="true">
      <circle cx="40" cy="40" r={RADIUS} className="ring-track" />
      <circle
        cx="40"
        cy="40"
        r={RADIUS}
        className="ring-value"
        strokeDasharray={CIRCUMFERENCE}
        strokeDashoffset={CIRCUMFERENCE * (1 - clamped / 100)}
      />
    </svg>
  );
}

export function LimitCard({ w, now }: { w: LimitWindow; now: number }) {
  const pct = Math.round(w.usedPercentage);
  return (
    <div className="limit">
      <div className="limit-ring">
        <Ring pct={w.usedPercentage} />
        <span className="limit-pct">
          {pct}
          <small>%</small>
        </span>
      </div>
      <div className="limit-text">
        <span className="limit-label">{w.label}</span>
        <span className="limit-reset">{w.resetsAt ? resetsIn(w.resetsAt, now) : "sem horário de reinício"}</span>
      </div>
    </div>
  );
}

const PRODUCT_CLASS: Record<string, string> = { claude_code: "product-code", chat: "product-chat", cowork: "product-cowork" };

/** Barra com a parte do uso da semana que veio de cada produto (Claude Code, navegador...). */
function Breakdown({ account }: { account: AccountStatus }) {
  if (account.breakdown.length === 0) return null;
  return (
    <div className="breakdown">
      <p className="breakdown-title">Uso da semana por produto</p>
      <div className="breakdown-bar" aria-hidden="true">
        {account.breakdown.map((p) => (
          <span key={p.key} className={PRODUCT_CLASS[p.key] ?? "product-other"} style={{ flexGrow: p.percent }} />
        ))}
      </div>
      <ul className="breakdown-legend">
        {account.breakdown.map((p) => (
          <li key={p.key}>
            <span className={`swatch ${PRODUCT_CLASS[p.key] ?? "product-other"}`} />
            {p.label}
            <strong>{Math.round(p.percent)}%</strong>
          </li>
        ))}
      </ul>
    </div>
  );
}

function SourceNote({ limits, account, now }: { limits: LimitsData; account: AccountStatus | null; now: number }) {
  if (account?.enabled) {
    return (
      <p className="section-note">
        Uso da conta inteira (navegador, app e Claude Code), atualizado {elapsed(limits.capturedAt, now)}.
        {account.error && <span className="inline-error"> Última consulta falhou: {account.error}</span>}
      </p>
    );
  }
  return (
    <p className="section-note">
      Atualizado {elapsed(limits.capturedAt, now)}, na última resposta do Claude Code. Para ver o número atualizado
      também quando você usa o Claude no navegador, ligue "Uso da conta" em Configurações.
    </p>
  );
}

export function Limits({
  limits,
  capture,
  account,
  onEnable,
  now,
}: {
  limits: LimitsData | null;
  capture: CaptureStatus | null;
  account: AccountStatus | null;
  onEnable: () => Promise<void>;
  now: number;
}) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function enable() {
    setBusy(true);
    setError(null);
    try {
      await onEnable();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  if (capture && !capture.enabled && !limits) {
    return (
      <div className="card limits-setup">
        <div>
          <h3>Veja quanto do seu limite já foi usado</h3>
          <p>
            O Claude Code informa a porcentagem da sessão de 5 horas e da semana a cada resposta. Para ler esse número,
            o app precisa se ligar à statusline do Claude Code. A statusline atual (como a do Orca) continua
            funcionando, um backup do arquivo é feito antes e dá para desligar em Configurações.
          </p>
          {error && <p className="inline-error">{error}</p>}
        </div>
        <button className="button button-primary" onClick={enable} disabled={busy}>
          {busy ? "Ativando…" : "Mostrar porcentagens"}
        </button>
      </div>
    );
  }

  if (!limits || limits.windows.length === 0) {
    return (
      <div className="card limits-waiting">
        <p>
          Tudo pronto. As porcentagens aparecem aqui depois da próxima resposta do Claude em qualquer sessão. Sessões
          que já estavam abertas passam a enviar os números quando forem reiniciadas.
        </p>
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
      <SourceNote limits={limits} account={account} now={now} />
      {account?.enabled && <Breakdown account={account} />}
    </div>
  );
}
