import { describe, expect, it } from "vitest";
import { removeGroundStrip, removeSolidBackground, unionBox, type Rgba } from "./gif";

const W = [255, 255, 255];
const DOG = [221, 135, 65];
const EYE = [255, 255, 254]; // quase branco, mas dentro do bicho
const GROUND = [123, 76, 76];

/** Monta um quadro a partir de linhas de caracteres: '.' fundo branco, 'd' bicho, 'e' olho, 'g' chão, ' ' transparente. */
function frame(rows: string[]): Rgba {
  const height = rows.length;
  const width = rows[0].length;
  const data = new Uint8ClampedArray(width * height * 4);
  const colors: Record<string, number[]> = { ".": W, d: DOG, e: EYE, g: GROUND };
  rows.forEach((row, y) =>
    [...row].forEach((ch, x) => {
      const i = (y * width + x) * 4;
      if (ch === " ") return;
      data.set([...colors[ch], 255], i);
    }),
  );
  return { data, width, height };
}

const visible = (f: Rgba) => {
  const rows: string[] = [];
  for (let y = 0; y < f.height; y++) {
    let r = "";
    for (let x = 0; x < f.width; x++) r += f.data[(y * f.width + x) * 4 + 3] ? "#" : " ";
    rows.push(r);
  }
  return rows;
};

describe("removeSolidBackground", () => {
  it("apaga o fundo ligado às bordas e preserva o branco dentro do bicho", () => {
    const f = frame([
      "......", //
      ".dddd.",
      ".dedd.",
      ".dddd.",
      "......",
    ]);
    removeSolidBackground([f]);
    expect(visible(f)).toEqual(["      ", " #### ", " #### ", " #### ", "      "]);
  });

  it("não mexe em GIF que já é transparente", () => {
    const f = frame([" dd ", " dd "]);
    removeSolidBackground([f]);
    expect(visible(f)).toEqual([" ## ", " ## "]);
  });
});

describe("removeGroundStrip", () => {
  it("remove a faixa de chão que ocupa toda a largura", () => {
    const f = frame([
      "  dd  ", //
      " dddd ",
      " d  d ",
      "gggggg",
      "gggggg",
    ]);
    removeGroundStrip([f]);
    expect(visible(f)).toEqual(["  ##  ", " #### ", " #  # ", "      ", "      "]);
  });

  it("remove o chão mesmo quando o bicho é mais largo que ele", () => {
    const f = frame([
      "dddddddddd", //
      " d      d ",
      " gggggggg ",
    ]);
    removeGroundStrip([f]);
    expect(visible(f)).toEqual(["##########", " #      # ", "          "]);
  });

  it("não apaga as patas quando não há chão", () => {
    const f = frame([" dddd ", "dddddd", " d  d "]);
    removeGroundStrip([f]);
    expect(visible(f)).toEqual([" #### ", "######", " #  # "]);
  });
});

describe("unionBox", () => {
  it("cobre o bicho em todos os quadros", () => {
    const a = frame(["d   ", "    "]);
    const b = frame(["    ", "   d"]);
    expect(unionBox([a, b])).toEqual({ x: 0, y: 0, w: 4, h: 2 });
    expect(unionBox([frame(["  "])])).toBeNull();
  });
});
