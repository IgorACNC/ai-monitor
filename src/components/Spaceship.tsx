import { useCallback, useEffect, useRef, useState } from "react";
import "./spaceship.css";

/** Um terminal (sessão) visto como tripulante da nave. */
export type CrewMember = {
  id: string;
  name: string;
  busy: boolean;
  /** Contexto usado, se conhecido. */
  percent: number | null;
};

// Coordenadas no desenho da nave (viewBox 900 x 260).
const W = 900;
const H = 260;
/** Velocidade de caminhada, em unidades do desenho por segundo. */
const SPEED = 45;

type StationKind = "console" | "window";
type Deck = {
  floor: number;
  minX: number;
  maxX: number;
  /** Onde os tripulantes novos chegam (teleporte ou escotilha). */
  spawn: number;
  stations: { x: number; kind: StationKind }[];
};

const DECKS: Deck[] = [
  {
    floor: 126,
    minX: 130,
    maxX: 770,
    spawn: 560,
    stations: [
      { x: 200, kind: "console" },
      { x: 300, kind: "window" },
      { x: 400, kind: "window" },
      { x: 480, kind: "console" },
      { x: 660, kind: "console" },
      { x: 760, kind: "window" },
    ],
  },
  {
    floor: 204,
    minX: 120,
    maxX: 760,
    spawn: 640,
    stations: [
      { x: 150, kind: "console" },
      { x: 320, kind: "console" },
      { x: 470, kind: "window" },
      { x: 560, kind: "console" },
    ],
  },
];

const VISORS = ["#f2a65a", "#6fa8dc", "#9ad18b", "#e58fb4", "#c3a6ff", "#f6d365", "#5fd3c6"];

function hash(text: string) {
  let h = 2166136261;
  for (let i = 0; i < text.length; i++) h = Math.imul(h ^ text.charCodeAt(i), 16777619);
  return h >>> 0;
}

const rand = (min: number, max: number) => min + Math.random() * (max - min);

/** Posto ocupado ("andar:x") → tripulante, para dois não pararem no mesmo lugar. */
const taken = new Map<string, string>();

function release(crewId: string) {
  for (const [key, id] of taken) if (id === crewId) taken.delete(key);
}

/**
 * Próximo destino: quem está trabalhando prefere os painéis; quem espera passeia e olha pela
 * janela. Só escolhe postos livres; sem nenhum, anda até um ponto qualquer do andar.
 */
function pickTarget(deck: Deck, busy: boolean, crewId: string): { x: number; kind: StationKind | null } {
  release(crewId);
  const wanted: StationKind = busy ? "console" : "window";
  if (busy ? Math.random() < 0.8 : Math.random() < 0.5) {
    const options = deck.stations.filter((s) => s.kind === wanted && !taken.has(`${deck.floor}:${s.x}`));
    const s = options[Math.floor(Math.random() * options.length)];
    if (s) {
      taken.set(`${deck.floor}:${s.x}`, crewId);
      return { x: s.x + rand(-6, 6), kind: s.kind };
    }
  }
  return { x: rand(deck.minX, deck.maxX), kind: null };
}

const reducedMotion = () => window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false;

const SHIP_KEY = "ai-monitor.ship";

/** "Lista" ou "Nave" nas sessões ativas; vale para todos os agentes e fica lembrado. */
export function useShipMode() {
  const [ship, setShip] = useState(() => {
    try {
      return localStorage.getItem(SHIP_KEY) === "1";
    } catch {
      return false;
    }
  });
  const update = useCallback((next: boolean) => {
    setShip(next);
    try {
      localStorage.setItem(SHIP_KEY, next ? "1" : "0");
    } catch {
      // Sem armazenamento, a escolha vale até fechar o app.
    }
  }, []);
  return [ship, update] as const;
}

export function ShipToggle({ ship, onChange }: { ship: boolean; onChange: (ship: boolean) => void }) {
  return (
    <div className="segmented segmented-small" role="tablist" aria-label="Como mostrar as sessões">
      <button role="tab" aria-selected={!ship} className={ship ? "" : "active"} onClick={() => onChange(false)}>
        Lista
      </button>
      <button role="tab" aria-selected={ship} className={ship ? "active" : ""} onClick={() => onChange(true)}>
        Modo nave
      </button>
    </div>
  );
}

function Astronaut({ visor, busy }: { visor: string; busy: boolean }) {
  return (
    <svg className="astronaut" viewBox="0 0 30 40" aria-hidden="true">
      <rect className="astro-pack" x="4" y="14" width="6" height="12" rx="2" />
      <g className="astro-leg astro-leg-a">
        <rect x="10" y="25" width="4.5" height="11" rx="2" />
      </g>
      <g className="astro-leg astro-leg-b">
        <rect x="15.5" y="25" width="4.5" height="11" rx="2" />
      </g>
      <rect className="astro-body" x="8" y="14" width="14" height="14" rx="4" />
      <circle className={busy ? "astro-light busy" : "astro-light"} cx="15" cy="19" r="1.7" />
      <g className="astro-arm">
        <rect x="18.5" y="15" width="4" height="10" rx="2" />
      </g>
      <g className="astro-head">
        <circle className="astro-helmet" cx="15" cy="8.5" r="8" />
        <ellipse cx="17.3" cy="8.5" rx="5" ry="4.2" fill={visor} />
        <ellipse cx="18.8" cy="7" rx="1.6" ry="1" fill="#fff" opacity="0.7" />
        <rect className="astro-helmet" x="13.8" y="-1.5" width="1.2" height="3" rx="0.6" />
      </g>
    </svg>
  );
}

function Crew({
  member,
  selected,
  still,
  onSelect,
}: {
  member: CrewMember;
  selected: boolean;
  still: boolean;
  onSelect: () => void;
}) {
  const h = hash(member.id);
  const deck = DECKS[h % DECKS.length];
  const visor = VISORS[(h >>> 3) % VISORS.length];
  // Parado (animação reduzida): cada um fica no seu posto.
  const home = deck.stations[(h >>> 7) % deck.stations.length];

  const [x, setX] = useState(still ? home.x : deck.spawn);
  const [dur, setDur] = useState(0);
  const [facing, setFacing] = useState<1 | -1>(1);
  const [phase, setPhase] = useState<"arriving" | "walking" | "typing" | "looking">(still ? "typing" : "arriving");
  const xRef = useRef(x);
  const busyRef = useRef(member.busy);
  busyRef.current = member.busy;

  useEffect(() => {
    if (still) return;
    let timer: number;
    const walk = () => {
      const target = pickTarget(deck, busyRef.current, member.id);
      const seconds = Math.abs(target.x - xRef.current) / SPEED;
      setFacing(target.x < xRef.current ? -1 : 1);
      xRef.current = target.x;
      setDur(seconds);
      setX(target.x);
      setPhase("walking");
      timer = window.setTimeout(() => {
        setPhase(target.kind === "console" ? "typing" : "looking");
        const stay = busyRef.current ? rand(4000, 9000) : rand(2500, 6000);
        timer = window.setTimeout(walk, stay);
      }, seconds * 1000);
    };
    // Primeiro o teleporte, depois começa a andar.
    timer = window.setTimeout(walk, 1400);
    return () => {
      window.clearTimeout(timer);
      release(member.id);
    };
  }, [deck, still, member.id]);

  const short = member.name.length > 14 ? `${member.name.slice(0, 13)}…` : member.name;

  return (
    <button
      className={`crew crew-${phase}${selected ? " selected" : ""}`}
      style={{
        left: `${(x / W) * 100}%`,
        top: `${(deck.floor / H) * 100}%`,
        transitionDuration: `${dur}s`,
      }}
      aria-pressed={selected}
      title={`${member.name}: ${member.busy ? "trabalhando" : "aguardando você"}`}
      onClick={onSelect}
    >
      <span className="crew-label">
        <span className={`status-dot ${member.busy ? "status-busy" : "status-idle"}`} aria-hidden="true" />
        {short}
        {member.percent !== null && <span className="crew-pct">{Math.round(member.percent)}%</span>}
      </span>
      <span className="crew-figure" style={{ transform: `scaleX(${facing})` }}>
        <Astronaut visor={visor} busy={member.busy} />
      </span>
      {phase === "arriving" && <span className="crew-beam" aria-hidden="true" />}
    </button>
  );
}

/** Estrelas fixas (mesmas a cada abertura) em volta da nave. */
const STARS = Array.from({ length: 70 }, (_, i) => {
  const h = hash(`estrela-${i}`);
  return { x: h % W, y: (h >>> 10) % H, r: 0.6 + ((h >>> 20) % 10) / 10, delay: ((h >>> 5) % 40) / 10 };
});

function Hull() {
  return (
    <svg className="ship-art" viewBox={`0 0 ${W} ${H}`} aria-hidden="true">
      <defs>
        <clipPath id="ship-porthole-a">
          <circle cx="300" cy="78" r="17" />
        </clipPath>
        <clipPath id="ship-porthole-b">
          <circle cx="400" cy="78" r="17" />
        </clipPath>
        <clipPath id="ship-cockpit">
          <path d="M 735 62 Q 830 66 852 124 L 735 124 Z" />
        </clipPath>
        <radialGradient id="ship-reactor">
          <stop offset="0" stopColor="#bff6ff" />
          <stop offset="0.5" stopColor="#4fd1e8" />
          <stop offset="1" stopColor="#4fd1e8" stopOpacity="0" />
        </radialGradient>
      </defs>

      {STARS.map((s, i) => (
        <circle key={i} className="ship-star" cx={s.x} cy={s.y} r={s.r} style={{ animationDelay: `${s.delay}s` }} />
      ))}

      {/* Motores */}
      <path className="ship-flame" d="M 52 82 Q 10 92 52 104 Z" />
      <path className="ship-flame ship-flame-b" d="M 52 160 Q 4 172 52 184 Z" />
      <rect className="ship-engine" x="40" y="76" width="18" height="34" rx="4" />
      <rect className="ship-engine" x="40" y="154" width="18" height="34" rx="4" />

      {/* Casco */}
      <path
        className="ship-hull"
        d="M 72 38 L 700 38 Q 872 42 884 132 Q 872 222 700 226 L 72 226 Q 52 226 52 206 L 52 58 Q 52 38 72 38 Z"
      />
      <path className="ship-fin" d="M 180 38 L 240 8 L 330 8 L 300 38 Z" />
      <path className="ship-fin" d="M 180 226 L 240 252 L 330 252 L 300 226 Z" />
      <path className="ship-inside" d="M 82 50 L 700 50 Q 856 56 866 132 Q 856 208 700 214 L 82 214 Z" />

      {/* Janelas com estrelas passando */}
      {[
        ["ship-porthole-a", 300],
        ["ship-porthole-b", 400],
      ].map(([clip, cx]) => (
        <g key={clip}>
          <circle className="ship-window" cx={cx} cy="78" r="17" />
          <g clipPath={`url(#${clip})`}>
            <g className="ship-streaks">
              {[0, 1, 2, 3].map((k) => (
                <rect
                  key={k}
                  x={Number(cx) - 30 + k * 18}
                  y={66 + ((k * 7) % 22)}
                  width="7"
                  height="1.4"
                  fill="#cfe3ff"
                />
              ))}
            </g>
          </g>
          <circle className="ship-window-ring" cx={cx} cy="78" r="17" />
        </g>
      ))}
      <path className="ship-window" d="M 735 62 Q 830 66 852 124 L 735 124 Z" />
      <g clipPath="url(#ship-cockpit)">
        <g className="ship-streaks ship-streaks-slow">
          {[0, 1, 2, 3, 4, 5].map((k) => (
            <rect key={k} x={700 + k * 30} y={72 + ((k * 13) % 44)} width="12" height="1.6" fill="#cfe3ff" />
          ))}
        </g>
        <circle cx="815" cy="88" r="9" fill="#e9a36b" />
      </g>
      <path className="ship-window-ring" d="M 735 62 Q 830 66 852 124 L 735 124 Z" fill="none" />

      {/* Andar de cima: mapa, radar, painel principal, assento do piloto */}
      <rect className="ship-console" x="180" y="100" width="40" height="26" rx="3" />
      <rect className="ship-screen" x="185" y="104" width="30" height="9" rx="2" />
      <circle className="ship-radar" cx="480" cy="78" r="18" />
      <line className="ship-radar-sweep" x1="480" y1="78" x2="480" y2="61" />
      <rect className="ship-console" x="630" y="98" width="60" height="28" rx="3" />
      <rect className="ship-screen" x="636" y="102" width="20" height="10" rx="2" />
      <rect className="ship-screen ship-screen-b" x="662" y="102" width="22" height="10" rx="2" />
      <rect className="ship-seat" x="742" y="100" width="12" height="26" rx="3" />

      {/* Piso de cima e escada */}
      <rect className="ship-floor" x="82" y="126" width="784" height="6" />
      {[140, 156, 172, 188].map((y) => (
        <rect key={y} className="ship-ladder" x="540" y={y} width="30" height="2" />
      ))}
      <rect className="ship-ladder" x="538" y="132" width="3" height="72" />
      <rect className="ship-ladder" x="569" y="132" width="3" height="72" />

      {/* Andar de baixo: reator, laboratório, plantas, teleporte, caixas */}
      <circle className="ship-reactor" cx="150" cy="168" r="26" fill="url(#ship-reactor)" />
      <rect className="ship-console" x="130" y="186" width="40" height="18" rx="3" />
      <rect className="ship-console" x="300" y="180" width="44" height="24" rx="3" />
      <rect className="ship-screen" x="306" y="184" width="32" height="8" rx="2" />
      <rect className="ship-pot" x="458" y="190" width="24" height="14" rx="2" />
      <path
        className="ship-plant"
        d="M 470 190 Q 452 170 462 160 M 470 190 Q 470 160 470 152 M 470 190 Q 490 172 480 160"
      />
      <ellipse className="ship-pad" cx="640" cy="203" rx="26" ry="5" />
      <rect className="ship-crate" x="720" y="182" width="24" height="22" rx="2" />
      <rect className="ship-crate" x="746" y="190" width="18" height="14" rx="2" />
      <rect className="ship-floor" x="82" y="204" width="700" height="6" />
    </svg>
  );
}

/** A nave com um tripulante andando para cada sessão; clicar num tripulante seleciona a sessão. */
export function Spaceship({
  crew,
  selectedId,
  onSelect,
}: {
  crew: CrewMember[];
  selectedId: string | null;
  onSelect: (id: string | null) => void;
}) {
  const [still] = useState(reducedMotion);
  return (
    <div
      className="ship"
      role="group"
      aria-label={`Nave com ${crew.length} ${crew.length === 1 ? "tripulante" : "tripulantes"}`}
    >
      <Hull />
      {crew.map((m) => (
        <Crew
          key={m.id}
          member={m}
          selected={m.id === selectedId}
          still={still}
          onSelect={() => onSelect(m.id === selectedId ? null : m.id)}
        />
      ))}
      {crew.length === 0 && <p className="ship-empty">Nenhum tripulante a bordo agora.</p>}
    </div>
  );
}
