import { useEffect, useRef } from "react";
import type { PetAnimation } from "./gif";

/**
 * Desenha o bichinho. Tocando, segue o tempo de cada quadro do GIF (multiplicado por `speed`);
 * parado, mostra o primeiro quadro.
 */
export function PetCanvas({ anim, playing, speed = 1 }: { anim: PetAnimation; playing: boolean; speed?: number }) {
  const ref = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = ref.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d")!;
    const draw = (i: number) => {
      ctx.clearRect(0, 0, canvas.width, canvas.height);
      ctx.drawImage(anim.frames[i], 0, 0);
    };
    draw(0);
    if (!playing || anim.frames.length < 2) return;

    let frame = 0;
    let elapsed = 0;
    let last = performance.now();
    let raf = 0;
    const tick = (t: number) => {
      elapsed += (t - last) * speed;
      last = t;
      let advanced = false;
      while (elapsed >= anim.durations[frame]) {
        elapsed -= anim.durations[frame];
        frame = (frame + 1) % anim.frames.length;
        advanced = true;
      }
      if (advanced) draw(frame);
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, [anim, playing, speed]);

  const first = anim.frames[0];
  return (
    <canvas
      ref={ref}
      className="pet-canvas"
      width={first.width}
      height={first.height}
      style={{ width: anim.width, height: anim.height }}
    />
  );
}
