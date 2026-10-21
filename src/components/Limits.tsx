import { useState } from "react";
import { elapsed, resetsIn, severity } from "../format";
import type { CaptureStatus, LimitWindow, Limits as LimitsData } from "../types";

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

function LimitCard({ w, now }: { w: LimitWindow; now: number }) {
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

export function Limits({
  limits,
  capture,
  onEnable,
  now,
}: {
  limits: LimitsData | null;
  capture: CaptureStatus | null;
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

  if (capture && !capture.enabled) {
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
      <p className="section-note">Atualizado {elapsed(limits.capturedAt, now)}</p>
    </div>
  );
}
