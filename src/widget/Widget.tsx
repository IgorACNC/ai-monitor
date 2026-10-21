import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { resetsIn, severity } from "../format";
import type { LimitWindow, Limits } from "../types";

const POLL_MS = 3000;
const SHOWN: { id: string; label: string }[] = [
  { id: "five_hour", label: "5 h" },
  { id: "seven_day", label: "sem." },
];

function Meter({ label, w, now }: { label: string; w: LimitWindow | undefined; now: number }) {
  const pct = w ? Math.min(100, Math.max(0, w.usedPercentage)) : 0;
  const title = w ? `${w.label}: ${Math.round(w.usedPercentage)}%${w.resetsAt ? `, ${resetsIn(w.resetsAt, now)}` : ""}` : undefined;
  return (
    <div className={`meter${w ? ` meter-${severity(w.usedPercentage)}` : " meter-empty"}`} title={title}>
      <span className="meter-pct">{w ? `${Math.round(w.usedPercentage)}%` : "—"}</span>
      <span className="meter-track" data-tauri-drag-region>
        <span className="meter-fill" style={{ height: `${pct}%` }} />
      </span>
      <span className="meter-label">{label}</span>
    </div>
  );
}

/** Barrinha que fica na tela com o app minimizado: % da sessão de 5 h e da semana. */
export function Widget() {
  const [limits, setLimits] = useState<Limits | null>(null);
  const [now, setNow] = useState(Date.now());

  useEffect(() => {
    const load = () =>
      invoke<Limits | null>("get_limits")
        .then((l) => {
          setLimits(l);
          setNow(Date.now());
        })
        .catch(() => setLimits(null));
    load();
    const id = setInterval(load, POLL_MS);
    return () => clearInterval(id);
  }, []);

  return (
    <div className="widget-bar" data-tauri-drag-region>
      <div className="widget-meters" data-tauri-drag-region>
        {SHOWN.map(({ id, label }) => (
          <Meter key={id} label={label} w={limits?.windows.find((w) => w.id === id)} now={now} />
        ))}
      </div>
      <button className="widget-open" onClick={() => invoke("show_main")} title="Abrir o AI Monitor" aria-label="Abrir o AI Monitor">
        <svg viewBox="0 0 16 16" aria-hidden="true">
          <path d="M6 3h7v7M13 3 4 12" />
        </svg>
      </button>
    </div>
  );
}
