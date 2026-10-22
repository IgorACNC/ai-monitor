import { useEffect, useState } from "react";

type BatteryManager = EventTarget & { level: number; charging: boolean };
type NavigatorWithBattery = Navigator & { getBattery?: () => Promise<BatteryManager> };

const dateFmt = new Intl.DateTimeFormat("pt-BR", { weekday: "short", day: "numeric", month: "short" });
const timeFmt = new Intl.DateTimeFormat("pt-BR", { hour: "2-digit", minute: "2-digit" });

function useClock() {
  const [now, setNow] = useState(() => new Date());
  useEffect(() => {
    // Acorda na virada de cada minuto, para o relógio não ficar atrasado.
    let timer = 0;
    const schedule = () => {
      timer = window.setTimeout(() => {
        setNow(new Date());
        schedule();
      }, 60_000 - (Date.now() % 60_000) + 50);
    };
    schedule();
    return () => clearTimeout(timer);
  }, []);
  return now;
}

function useBattery() {
  const [battery, setBattery] = useState<{ level: number; charging: boolean } | null>(null);
  useEffect(() => {
    const getBattery = (navigator as NavigatorWithBattery).getBattery;
    if (!getBattery) return;
    let manager: BatteryManager | null = null;
    const update = () => manager && setBattery({ level: manager.level, charging: manager.charging });
    getBattery
      .call(navigator)
      .then((m) => {
        manager = m;
        update();
        m.addEventListener("levelchange", update);
        m.addEventListener("chargingchange", update);
      })
      .catch(() => setBattery(null));
    return () => {
      manager?.removeEventListener("levelchange", update);
      manager?.removeEventListener("chargingchange", update);
    };
  }, []);
  return battery;
}

function BatteryIcon({ level, charging }: { level: number; charging: boolean }) {
  const width = Math.max(1, Math.round(level * 14));
  return (
    <svg viewBox="0 0 22 12" className={`battery-icon${level <= 0.2 && !charging ? " battery-low" : ""}`} aria-hidden="true">
      <rect x="0.75" y="0.75" width="18" height="10.5" rx="2.5" className="battery-shell" />
      <rect x="19.5" y="4" width="2" height="4" rx="1" className="battery-tip" />
      <rect x="2.5" y="2.5" width={width} height="7" rx="1.2" className="battery-fill" />
      {charging && <path d="M10.5 2.2 7.5 6.6h2.6l-1 3.2 3.4-4.6H9.9z" className="battery-bolt" />}
    </svg>
  );
}

export type Page = "home" | "github" | "settings";

const PAGES: { id: Page; label: string }[] = [
  { id: "home", label: "Visão geral" },
  { id: "github", label: "GitHub" },
  { id: "settings", label: "Configurações" },
];

/** Barra do topo: marca e navegação à esquerda; data, hora e bateria à direita. */
export function TopBar({
  page,
  onNavigate,
  badges,
}: {
  page: Page;
  onNavigate: (p: Page) => void;
  /** Número ao lado do nome da aba, ex.: PRs esperando a sua revisão. */
  badges: Partial<Record<Page, number>>;
}) {
  const now = useClock();
  const battery = useBattery();
  const pct = battery ? Math.round(battery.level * 100) : null;
  return (
    <header className="topbar">
      <div className="brand">
        <svg viewBox="0 0 24 24" className="brand-mark" aria-hidden="true">
          <path d="M4 16a8 8 0 1 1 16 0" />
          <path d="M12 16l5-6" className="brand-needle" />
        </svg>
        <span>AI Monitor</span>
      </div>
      <nav className="tabs" aria-label="Navegação principal">
        {PAGES.map((p) => (
          <button
            key={p.id}
            className={`tab${page === p.id ? " active" : ""}`}
            aria-current={page === p.id ? "page" : undefined}
            onClick={() => onNavigate(p.id)}
          >
            {p.label}
            {!!badges[p.id] && <span className="tab-badge">{badges[p.id]}</span>}
          </button>
        ))}
      </nav>
      <div className="status">
        <span className="status-date">{dateFmt.format(now)}</span>
        <time className="status-time" dateTime={now.toISOString()}>
          {timeFmt.format(now)}
        </time>
        {battery && pct !== null && (
          <span className="status-battery" title={battery.charging ? `Bateria em ${pct}%, carregando` : `Bateria em ${pct}%`}>
            <BatteryIcon level={battery.level} charging={battery.charging} />
            {pct}%
          </span>
        )}
      </div>
    </header>
  );
}
