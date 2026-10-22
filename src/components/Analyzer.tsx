import { useCallback, useState } from "react";
import type { Analyzer } from "../types";

type ClaudeAnalyzer = Extract<Analyzer, { agent: "claude" }>;

const KEY = "ai-monitor.analyzer";
const DEFAULT: ClaudeAnalyzer = { agent: "claude", model: "sonnet", effort: "medium" };

const MODELS: { id: string; label: string }[] = [
  { id: "haiku", label: "Claude Haiku (rápido)" },
  { id: "sonnet", label: "Claude Sonnet" },
  { id: "opus", label: "Claude Opus (mais caprichado)" },
];

const EFFORTS: { id: string; label: string }[] = [
  { id: "low", label: "Baixo" },
  { id: "medium", label: "Médio" },
  { id: "high", label: "Alto" },
  { id: "xhigh", label: "Muito alto" },
  { id: "max", label: "Máximo" },
];

const OPENCODE = "opencode";

function load(): Analyzer {
  try {
    const saved = JSON.parse(localStorage.getItem(KEY) ?? "null");
    if (saved?.agent === "opencode") return { agent: "opencode" };
    if (
      saved?.agent === "claude" &&
      MODELS.some((m) => m.id === saved.model) &&
      EFFORTS.some((e) => e.id === saved.effort)
    ) {
      return { agent: "claude", model: saved.model, effort: saved.effort };
    }
  } catch {
    // Sem armazenamento: usa o padrão.
  }
  return DEFAULT;
}

/** Escolha de quem analisa os PRs; vale como padrão e guarda a última usada. */
export function useAnalyzer() {
  const [analyzer, setAnalyzer] = useState<Analyzer>(load);
  const update = useCallback((next: Analyzer) => {
    setAnalyzer(next);
    try {
      localStorage.setItem(KEY, JSON.stringify(next));
    } catch {
      // Sem armazenamento, a escolha vale só até fechar o app.
    }
  }, []);
  return [analyzer, update] as const;
}

export function analyzerLabel(a: Analyzer) {
  if (a.agent === "opencode") return "opencode";
  const model = MODELS.find((m) => m.id === a.model)?.label.replace(/ \(.*\)$/, "") ?? a.model;
  const effort = EFFORTS.find((e) => e.id === a.effort)?.label.toLowerCase() ?? a.effort;
  return `${model}, desempenho ${effort}`;
}

/** Dois seletores: modelo (ou opencode) e desempenho. */
export function AnalyzerPicker({
  value,
  onChange,
  disabled,
}: {
  value: Analyzer;
  onChange: (a: Analyzer) => void;
  disabled?: boolean;
}) {
  const claude: ClaudeAnalyzer | null = value.agent === "claude" ? value : null;
  return (
    <div className="analyzer-picker">
      <label>
        <span>Quem analisa</span>
        <select
          value={claude ? claude.model : OPENCODE}
          disabled={disabled}
          onChange={(e) =>
            onChange(
              e.target.value === OPENCODE
                ? { agent: "opencode" }
                : { agent: "claude", model: e.target.value, effort: claude?.effort ?? DEFAULT.effort },
            )
          }
        >
          {MODELS.map((m) => (
            <option key={m.id} value={m.id}>
              {m.label}
            </option>
          ))}
          <option value={OPENCODE}>opencode (grátis, não gasta o Claude)</option>
        </select>
      </label>
      <label>
        <span>Desempenho</span>
        <select
          value={claude ? claude.effort : ""}
          disabled={disabled || !claude}
          onChange={(e) => claude && onChange({ ...claude, effort: e.target.value })}
        >
          {!claude && <option value="">Padrão do opencode</option>}
          {EFFORTS.map((e) => (
            <option key={e.id} value={e.id}>
              {e.label}
            </option>
          ))}
        </select>
      </label>
    </div>
  );
}
