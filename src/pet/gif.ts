/**
 * Transforma um GIF qualquer num bichinho pronto para a faixa: quadros sem fundo,
 * sem chão desenhado, recortados no espaço que o bicho ocupa e todos da mesma altura.
 */

export type Rgba = { data: Uint8ClampedArray<ArrayBuffer>; width: number; height: number };
export type Box = { x: number; y: number; w: number; h: number };

export type PetAnimation = {
  frames: ImageBitmap[];
  /** Duração de cada quadro, em ms, como veio no GIF. */
  durations: number[];
  /** Tamanho de exibição, em px de CSS. */
  width: number;
  height: number;
};

/** Altura comum a todos os bichinhos, em px de CSS. */
export const PET_HEIGHT = 48;

const COLOR_TOLERANCE = 40; // soma das diferenças de R, G e B
const MIN_ALPHA = 16;
/** Navegadores tratam quadros de GIF com 0–10 ms como 100 ms; seguimos o mesmo padrão. */
const DEFAULT_FRAME_MS = 100;

const near = (d: Uint8ClampedArray, i: number, c: readonly number[]) =>
  Math.abs(d[i] - c[0]) + Math.abs(d[i + 1] - c[1]) + Math.abs(d[i + 2] - c[2]) <= COLOR_TOLERANCE;

/**
 * Se o GIF tem fundo sólido (os quatro cantos opacos e da mesma cor), apaga esse fundo
 * a partir das bordas, sem furar áreas da mesma cor que estejam dentro do bicho.
 */
export function removeSolidBackground(frames: Rgba[]): void {
  const first = frames[0];
  if (!first) return;
  const { width: w, height: h, data } = first;
  const corners = [0, w - 1, (h - 1) * w, h * w - 1].map((p) => p * 4);
  if (corners.some((i) => data[i + 3] < 255)) return;
  const bg = [data[corners[0]], data[corners[0] + 1], data[corners[0] + 2]];
  if (!corners.every((i) => near(data, i, bg))) return;

  for (const f of frames) {
    const visited = new Uint8Array(w * h);
    const stack: number[] = [];
    for (let x = 0; x < w; x++) stack.push(x, (h - 1) * w + x);
    for (let y = 0; y < h; y++) stack.push(y * w, y * w + w - 1);
    while (stack.length) {
      const p = stack.pop()!;
      if (visited[p]) continue;
      visited[p] = 1;
      const i = p * 4;
      if (f.data[i + 3] === 0 || !near(f.data, i, bg)) continue;
      f.data[i + 3] = 0;
      const x = p % w;
      if (x > 0) stack.push(p - 1);
      if (x < w - 1) stack.push(p + 1);
      if (p >= w) stack.push(p - w);
      if (p < w * (h - 1)) stack.push(p + w);
    }
  }
}

/** Menor retângulo que contém todos os pixels visíveis de todos os quadros. */
export function unionBox(frames: Rgba[]): Box | null {
  let x0 = Infinity, y0 = Infinity, x1 = -1, y1 = -1;
  for (const f of frames) {
    for (let y = 0; y < f.height; y++) {
      for (let x = 0; x < f.width; x++) {
        if (f.data[(y * f.width + x) * 4 + 3] >= MIN_ALPHA) {
          if (x < x0) x0 = x;
          if (x > x1) x1 = x;
          if (y < y0) y0 = y;
          if (y > y1) y1 = y;
        }
      }
    }
  }
  return x1 < 0 ? null : { x: x0, y: y0, w: x1 - x0 + 1, h: y1 - y0 + 1 };
}

/**
 * Alguns ciclos de corrida trazem uma faixa de chão embaixo do bicho. Se a última linha
 * visível é uma faixa larga de uma cor só, apaga essa cor de baixo para cima até ela sumir.
 */
export function removeGroundStrip(frames: Rgba[]): void {
  const box = unionBox(frames);
  const first = frames[0];
  if (!box || !first) return;
  const bottom = box.y + box.h - 1;
  const counts = new Map<number, number>();
  let opaque = 0;
  for (let x = box.x; x < box.x + box.w; x++) {
    const i = (bottom * first.width + x) * 4;
    if (first.data[i + 3] < MIN_ALPHA) continue;
    opaque++;
    const key = (first.data[i] << 16) | (first.data[i + 1] << 8) | first.data[i + 2];
    counts.set(key, (counts.get(key) ?? 0) + 1);
  }
  // Chão = linha larga e praticamente de uma cor só. Patas nunca preenchem a linha assim.
  if (opaque < box.w * 0.5) return;
  const [key, count] = [...counts.entries()].sort((a, b) => b[1] - a[1])[0];
  if (count < opaque * 0.95) return;
  const ground = [(key >> 16) & 255, (key >> 8) & 255, key & 255];

  for (const f of frames) {
    for (let y = bottom; y >= box.y; y--) {
      let found = false;
      for (let x = box.x; x < box.x + box.w; x++) {
        const i = (y * f.width + x) * 4;
        if (f.data[i + 3] >= MIN_ALPHA && near(f.data, i, ground)) {
          f.data[i + 3] = 0;
          found = true;
        }
      }
      if (!found) break;
    }
  }
}

async function decodeGif(bytes: ArrayBuffer): Promise<{ frames: Rgba[]; durations: number[] }> {
  const decoder = new ImageDecoder({ data: bytes, type: "image/gif" });
  await decoder.tracks.ready;
  const count = decoder.tracks.selectedTrack?.frameCount ?? 1;
  const frames: Rgba[] = [];
  const durations: number[] = [];
  for (let i = 0; i < count; i++) {
    const { image } = await decoder.decode({ frameIndex: i });
    const canvas = new OffscreenCanvas(image.displayWidth, image.displayHeight);
    const ctx = canvas.getContext("2d")!;
    ctx.drawImage(image, 0, 0);
    const { data, width, height } = ctx.getImageData(0, 0, canvas.width, canvas.height);
    frames.push({ data, width, height });
    const ms = (image.duration ?? 0) / 1000;
    durations.push(ms > 10 ? ms : DEFAULT_FRAME_MS);
    image.close();
  }
  decoder.close();
  return { frames, durations };
}

export async function loadPetAnimation(bytes: ArrayBuffer): Promise<PetAnimation> {
  const { frames, durations } = await decodeGif(bytes);
  removeSolidBackground(frames);
  removeGroundStrip(frames);
  const box = unionBox(frames);
  if (!box) throw new Error("O GIF está vazio ou totalmente transparente.");

  const dpr = window.devicePixelRatio || 1;
  const width = Math.round((box.w * PET_HEIGHT) / box.h);
  const out = await Promise.all(
    frames.map((f) => {
      const src = new OffscreenCanvas(f.width, f.height);
      src.getContext("2d")!.putImageData(new ImageData(f.data, f.width, f.height), 0, 0);
      const dst = new OffscreenCanvas(Math.round(width * dpr), Math.round(PET_HEIGHT * dpr));
      const ctx = dst.getContext("2d")!;
      ctx.imageSmoothingQuality = "high";
      ctx.drawImage(src, box.x, box.y, box.w, box.h, 0, 0, dst.width, dst.height);
      return createImageBitmap(dst);
    }),
  );
  return { frames: out, durations, width, height: PET_HEIGHT };
}
