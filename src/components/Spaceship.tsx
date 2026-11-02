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
  /** Duto de ventilação de onde os tripulantes novos saem. */
  vent: number;
  stations: { x: number; kind: StationKind }[];
};

const DECKS: Deck[] = [
  {
    floor: 126,
    minX: 120,
    maxX: 770,
    vent: 540,
    stations: [
      { x: 175, kind: "console" },
      { x: 300, kind: "window" },
      { x: 410, kind: "console" },
      { x: 470, kind: "window" },
      { x: 650, kind: "console" },
      { x: 760, kind: "window" },
    ],
  },
  {
    floor: 204,
    minX: 110,
    maxX: 760,
    vent: 660,
    stations: [
      { x: 190, kind: "console" },
      { x: 330, kind: "console" },
      { x: 520, kind: "window" },
      { x: 735, kind: "window" },
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

  const [x, setX] = useState(still ? home.x : deck.vent);
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
    // Primeiro o feixe de chegada, depois começa a andar.
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

/** Duto de ventilação no piso. */
function Vent({ x, floor }: { x: number; floor: number }) {
  return (
    <g className="ship-vent">
      <rect x={x - 17} y={floor - 3} width="34" height="6" rx="1.5" />
      {[-11, -5, 1, 7].map((dx) => (
        <rect key={dx} className="ship-vent-slat" x={x + dx} y={floor - 2} width="3" height="4" />
      ))}
    </g>
  );
}

function Porthole({ id, cx, cy }: { id: string; cx: number; cy: number }) {
  return (
    <g>
      <circle className="ship-window" cx={cx} cy={cy} r="15" />
      <g clipPath={`url(#${id})`}>
        <g className="ship-streaks">
          {[0, 1, 2, 3].map((k) => (
            <rect key={k} x={cx - 30 + k * 18} y={cy - 12 + ((k * 7) % 22)} width="7" height="1.4" fill="#cfe3ff" />
          ))}
        </g>
      </g>
      <circle className="ship-ring" cx={cx} cy={cy} r="15" />
    </g>
  );
}

function Hull() {
  const portholes: [string, number, number][] = [
    ["ship-ph-a", 300, 80],
    ["ship-ph-b", 470, 80],
  ];
  return (
    <svg className="ship-art" viewBox={`0 0 ${W} ${H}`} aria-hidden="true">
      <defs>
        {portholes.map(([id, cx, cy]) => (
          <clipPath key={id} id={id}>
            <circle cx={cx} cy={cy} r="15" />
          </clipPath>
        ))}
        <clipPath id="ship-cockpit">
          <path d="M 735 60 Q 832 64 852 124 L 735 124 Z" />
        </clipPath>
        <linearGradient id="ship-core" x1="0" x2="1">
          <stop offset="0" stopColor="#2aa7d8" />
          <stop offset="0.5" stopColor="#b8f3ff" />
          <stop offset="1" stopColor="#2aa7d8" />
        </linearGradient>
        <pattern id="ship-hazard" width="12" height="12" patternUnits="userSpaceOnUse" patternTransform="rotate(45)">
          <rect width="12" height="12" fill="#f2c230" />
          <rect width="6" height="12" fill="#1b1d24" />
        </pattern>
        <pattern id="ship-tiles" width="20" height="8" patternUnits="userSpaceOnUse">
          <rect width="20" height="8" fill="#8e97ad" />
          <rect width="1.5" height="8" fill="#6d7690" />
        </pattern>
      </defs>

      {STARS.map((s, i) => (
        <circle key={i} className="ship-star" cx={s.x} cy={s.y} r={s.r} style={{ animationDelay: `${s.delay}s` }} />
      ))}

      {/* Motores redondos atrás, com chama */}
      {[92, 172].map((cy, i) => (
        <g key={cy}>
          <path className={`ship-flame${i ? " ship-flame-b" : ""}`} d={`M 22 ${cy - 13} Q -18 ${cy} 22 ${cy + 13} Z`} />
          <rect className="ship-line" x="18" y={cy - 22} width="44" height="44" rx="14" fill="#a3acbf" />
          <rect x="18" y={cy - 22} width="10" height="44" rx="5" fill="#6d7690" />
        </g>
      ))}

      {/* Casco */}
      <path className="ship-fin ship-line" d="M 190 40 L 250 10 L 340 10 L 315 40 Z" />
      <path className="ship-fin ship-line" d="M 190 224 L 250 252 L 340 252 L 315 224 Z" />
      <path
        className="ship-hull ship-line"
        d="M 72 38 L 700 38 Q 874 42 886 132 Q 874 222 700 226 L 72 226 Q 52 226 52 206 L 52 58 Q 52 38 72 38 Z"
      />
      {[110, 610, 840].map((x) => (
        <circle key={x} cx={x} cy="44" r="1.8" fill="#5d6680" />
      ))}

      {/* Salas: paredes de cada uma */}
      <path className="ship-line" fill="#3b4459" d="M 82 50 L 262 50 L 262 126 L 82 126 Z" />
      <path className="ship-line" fill="#56607a" d="M 262 50 L 560 50 L 560 126 L 262 126 Z" />
      <path className="ship-line" fill="#3a4a63" d="M 560 50 L 700 50 Q 856 56 866 126 L 560 126 Z" />
      <path className="ship-line" fill="#30405e" d="M 82 132 L 262 132 L 262 206 L 82 206 Z" />
      <path className="ship-line" fill="#4a4637" d="M 262 132 L 430 132 L 430 206 L 262 206 Z" />
      <path className="ship-line" fill="#2f5446" d="M 430 132 L 610 132 L 610 206 L 430 206 Z" />
      <path className="ship-line" fill="#4a4152" d="M 610 132 L 790 132 Q 820 170 790 206 L 610 206 Z" />
      {/* Portas entre as salas (vão aberto embaixo) */}
      {[
        [262, 50],
        [560, 50],
        [262, 132],
        [430, 132],
        [610, 132],
      ].map(([x, y]) => (
        <rect key={`${x}-${y}`} x={x - 5} y={y} width="10" height="20" fill="#1b1d24" />
      ))}

      {/* Comunicação: radar e painel */}
      <circle className="ship-radar ship-line" cx="212" cy="80" r="17" />
      <line className="ship-radar-sweep" x1="212" y1="80" x2="212" y2="64" />
      <rect className="ship-line" x="150" y="102" width="50" height="24" rx="4" fill="#c7ccd6" />
      <rect className="ship-screen" x="156" y="106" width="22" height="9" rx="2" />
      <rect className="ship-screen ship-screen-b" x="181" y="106" width="13" height="9" rx="2" />

      {/* Refeitório: janelas e a mesa com o botão vermelho de emergência */}
      {portholes.map(([id, cx, cy]) => (
        <Porthole key={id} id={id} cx={cx} cy={cy} />
      ))}
      <rect className="ship-line" x="404" y="111" width="12" height="15" fill="#9aa3b6" />
      <rect className="ship-line" x="366" y="104" width="88" height="9" rx="4.5" fill="#d9dde6" />
      <path className="ship-line" d="M 402 104 Q 402 92 410 92 Q 418 92 418 104 Z" fill="#e0262f" />
      <path
        d="M 405 100 Q 406 95 410 94.5"
        fill="none"
        stroke="#fff"
        strokeWidth="1.6"
        strokeLinecap="round"
        opacity="0.8"
      />

      {/* Navegação: painel, assento e a cabine */}
      <rect className="ship-line" x="622" y="98" width="62" height="28" rx="4" fill="#c7ccd6" />
      <rect className="ship-screen" x="628" y="103" width="22" height="10" rx="2" />
      <rect className="ship-screen ship-screen-b" x="655" y="103" width="23" height="10" rx="2" />
      <path className="ship-window" d="M 735 60 Q 832 64 852 124 L 735 124 Z" />
      <g clipPath="url(#ship-cockpit)">
        <g className="ship-streaks ship-streaks-slow">
          {[0, 1, 2, 3, 4, 5].map((k) => (
            <rect key={k} x={700 + k * 30} y={70 + ((k * 13) % 44)} width="12" height="1.6" fill="#cfe3ff" />
          ))}
        </g>
        <circle cx="815" cy="86" r="10" fill="#e9a36b" />
        <circle cx="811" cy="83" r="3" fill="#c97f4c" />
      </g>
      <path className="ship-ring" d="M 735 60 Q 832 64 852 124 L 735 124 Z" fill="none" />
      <rect className="ship-line" x="740" y="100" width="13" height="26" rx="4" fill="#d6262c" />

      {/* Piso de cima */}
      <rect className="ship-line" x="82" y="126" width="784" height="8" fill="url(#ship-tiles)" />

      {/* Reator: núcleo pulsando */}
      <rect className="ship-line" x="132" y="198" width="66" height="8" rx="2" fill="#9aa3b6" />
      <rect className="ship-reactor ship-line" x="149" y="140" width="32" height="58" rx="16" fill="url(#ship-core)" />
      <rect className="ship-line" x="206" y="176" width="34" height="30" rx="4" fill="#c7ccd6" />
      <rect className="ship-screen" x="211" y="181" width="24" height="8" rx="2" />

      {/* Elétrica: quadro com faixas de perigo e interruptores */}
      <rect className="ship-line" x="300" y="146" width="62" height="44" rx="3" fill="#7d838f" />
      <rect x="300" y="180" width="62" height="10" fill="url(#ship-hazard)" />
      {[310, 324, 338, 352].map((x, i) => (
        <rect
          key={x}
          className={i % 2 ? "ship-switch ship-switch-b" : "ship-switch"}
          x={x - 3}
          y="154"
          width="6"
          height="14"
          rx="2"
        />
      ))}
      <path d="M 300 160 Q 280 170 284 206" fill="none" stroke="#f2c230" strokeWidth="2" />
      <path d="M 362 166 Q 384 176 380 206" fill="none" stroke="#e0262f" strokeWidth="2" />

      {/* O2: plantas */}
      {[492, 548].map((x) => (
        <g key={x}>
          <rect className="ship-line" x={x - 13} y="190" width="26" height="16" rx="3" fill="#c7ccd6" />
          <path
            className="ship-plant"
            d={`M ${x} 190 Q ${x - 18} 172 ${x - 8} 160 M ${x} 190 Q ${x} 162 ${x} 152 M ${x} 190 Q ${x + 18} 174 ${x + 9} 162`}
          />
        </g>
      ))}

      {/* Armazém: caixas */}
      <rect className="ship-line" x="712" y="182" width="26" height="24" rx="2" fill="#a07a4a" />
      <rect className="ship-line" x="740" y="190" width="20" height="16" rx="2" fill="#8a6640" />
      <rect className="ship-line" x="720" y="164" width="18" height="18" rx="2" fill="#b58c58" />

      {/* Piso de baixo */}
      <rect className="ship-line" x="82" y="204" width="708" height="8" fill="url(#ship-tiles)" />

      {DECKS.map((d) => (
        <Vent key={d.floor} x={d.vent} floor={d.floor} />
      ))}
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
