import { useEffect, useRef, useState, type PointerEvent } from "react";
import type { PetAnimation } from "./gif";
import { PetCanvas } from "./PetCanvas";
import type { PetSettings } from "./settings";

type Mode = "idle" | "walk" | "run" | "sleep" | "happy";

type Ball = { x: number; y: number; vx: number; vy: number };

const GROUND = 10; // distância do chão até a base da barra, em px
const BALL_RADIUS = 6;
const GRAVITY = 1500; // px/s²
const BOUNCE = 0.55;
const WALK_SPEED = 45; // px/s
const RUN_SPEED = 260;
const NAP_AFTER_MS = 60_000;
/** Correndo atrás da bolinha, o GIF toca mais rápido que andando. */
const RUN_ANIMATION_SPEED = 1.6;

const rand = (min: number, max: number) => min + Math.random() * (max - min);

/**
 * Faixa superior onde o bichinho vive. Clicar nela joga a bolinha; sem bolinha,
 * ele passeia de tempos em tempos e, depois de um tempo sem brincadeira, cochila.
 */
export function Playground({
  settings,
  anim,
  notice,
}: {
  settings: PetSettings;
  anim: PetAnimation | null;
  /** Texto mostrado na faixa quando não há bichinho para exibir. */
  notice?: string | null;
}) {
  const areaRef = useRef<HTMLDivElement>(null);
  const petRef = useRef<HTMLDivElement>(null);
  const ballRef = useRef<HTMLDivElement>(null);
  const [mode, setMode] = useState<Mode>("idle");
  const [facingLeft, setFacingLeft] = useState(false);
  const [hasBall, setHasBall] = useState(false);
  const [everThrown, setEverThrown] = useState(false);

  const sim = useRef({
    x: 120,
    dir: 1,
    mode: "idle" as Mode,
    target: 120,
    ball: null as Ball | null,
    modeUntil: 0,
    nextWander: performance.now() + rand(2000, 5000),
    lastPlay: performance.now(),
  });

  useEffect(() => {
    if (!settings.enabled || !anim) return;
    const area = areaRef.current;
    if (!area) return;
    const petWidth = anim.width;
    const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    const s = sim.current;
    let last = performance.now();
    let raf = 0;

    const setModeIfChanged = (m: Mode) => {
      if (s.mode !== m) {
        s.mode = m;
        setMode(m);
      }
    };
    const moveToward = (target: number, speed: number, dt: number) => {
      const dx = target - s.x;
      const dir = Math.sign(dx) || s.dir;
      if (dir !== s.dir) {
        s.dir = dir;
        setFacingLeft(dir < 0);
      }
      s.x += Math.sign(dx) * Math.min(Math.abs(dx), speed * dt);
      return Math.abs(target - s.x) < 1;
    };

    const step = (t: number) => {
      const dt = Math.min(0.05, (t - last) / 1000);
      last = t;
      const width = area.clientWidth;
      const ballFloor = area.clientHeight - GROUND - BALL_RADIUS;
      const margin = petWidth / 2;
      s.x = Math.min(Math.max(s.x, margin), width - margin);

      const b = s.ball;
      if (b) {
        b.vy += GRAVITY * dt;
        b.x += b.vx * dt;
        b.y += b.vy * dt;
        if (b.y > ballFloor) {
          b.y = ballFloor;
          b.vy = Math.abs(b.vy) < 80 ? 0 : -b.vy * BOUNCE;
          b.vx *= 1 - Math.min(1, 2.5 * dt);
        }
        if (b.x < BALL_RADIUS || b.x > width - BALL_RADIUS) {
          b.x = Math.min(Math.max(b.x, BALL_RADIUS), width - BALL_RADIUS);
          b.vx = -b.vx * 0.6;
        }
        setModeIfChanged("run");
        moveToward(b.x, RUN_SPEED, dt);
        // Pega a bolinha quando ela está perto do chão e ao alcance da boca.
        if (Math.abs(b.x - s.x) < Math.max(14, petWidth * 0.25) && b.y > ballFloor - 12) {
          s.ball = null;
          setHasBall(false);
          s.lastPlay = t;
          s.modeUntil = t + 1400;
          setModeIfChanged("happy");
        }
      } else if (s.mode === "happy" || s.mode === "sleep") {
        if (t >= s.modeUntil) {
          setModeIfChanged("idle");
          s.lastPlay = t;
          s.nextWander = t + rand(1500, 4000);
        }
      } else if (s.mode === "walk" || s.mode === "run") {
        if (moveToward(s.target, s.mode === "run" ? RUN_SPEED : WALK_SPEED, dt)) {
          setModeIfChanged("idle");
          s.nextWander = t + rand(5000, 15000);
        }
      } else if (!reducedMotion) {
        if (t - s.lastPlay > NAP_AFTER_MS) {
          s.modeUntil = t + rand(20_000, 40_000);
          setModeIfChanged("sleep");
        } else if (t >= s.nextWander) {
          s.target = rand(margin, width - margin);
          setModeIfChanged("walk");
        }
      }

      if (petRef.current) petRef.current.style.transform = `translateX(${s.x - petWidth / 2}px)`;
      if (ballRef.current && s.ball) {
        ballRef.current.style.transform = `translate(${s.ball.x - BALL_RADIUS}px, ${s.ball.y - BALL_RADIUS}px)`;
      }
      raf = requestAnimationFrame(step);
    };
    raf = requestAnimationFrame(step);
    return () => cancelAnimationFrame(raf);
  }, [settings.enabled, anim]);

  function throwBall(e: PointerEvent<HTMLDivElement>) {
    if (e.button !== 0 || !anim) return;
    const rect = e.currentTarget.getBoundingClientRect();
    const s = sim.current;
    s.ball = { x: e.clientX - rect.left, y: e.clientY - rect.top, vx: rand(-160, 160), vy: -180 };
    s.lastPlay = performance.now();
    setHasBall(true);
    setEverThrown(true);
  }

  if (!settings.enabled) return null;

  if (!anim) {
    return <div className="playground">{notice && <p className="playground-hint">{notice}</p>}</div>;
  }

  return (
    <div ref={areaRef} className="playground" onPointerDown={throwBall}>
      {!everThrown && (
        <p className="playground-hint">Clique aqui para jogar a bolinha para {settings.name || "o bichinho"}</p>
      )}
      <div className="ground" aria-hidden="true" />
      {hasBall && <div ref={ballRef} className="ball" aria-hidden="true" />}
      <div
        ref={petRef}
        className={`pet pet-${mode}`}
        style={{ width: anim.width, height: anim.height }}
        role="img"
        aria-label={settings.name ? `${settings.name}, seu bichinho` : "Seu bichinho"}
        title={settings.name || undefined}
      >
        <div className={`pet-body${facingLeft ? " facing-left" : ""}`}>
          <PetCanvas
            anim={anim}
            playing={mode === "walk" || mode === "run"}
            speed={mode === "run" ? RUN_ANIMATION_SPEED : 1}
          />
        </div>
        {mode === "sleep" && <span className="pet-bubble">zZz</span>}
        {mode === "happy" && <span className="pet-bubble">♥</span>}
      </div>
    </div>
  );
}
