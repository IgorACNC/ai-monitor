import { useCallback, useState } from "react";

export type PetSettings = {
  enabled: boolean;
  /** Nome do GIF escolhido; `null` usa o primeiro da lista. */
  pet: string | null;
  name: string;
};

const DEFAULTS: PetSettings = { enabled: true, pet: null, name: "Biscoito" };
const KEY = "ai-monitor.pet";

function load(): PetSettings {
  try {
    const raw = localStorage.getItem(KEY);
    const saved = raw ? JSON.parse(raw) : {};
    return {
      enabled: saved.enabled ?? DEFAULTS.enabled,
      pet: typeof saved.pet === "string" ? saved.pet : DEFAULTS.pet,
      name: typeof saved.name === "string" ? saved.name : DEFAULTS.name,
    };
  } catch {
    return DEFAULTS;
  }
}

export function usePetSettings() {
  const [settings, setSettings] = useState<PetSettings>(load);
  const update = useCallback((patch: Partial<PetSettings>) => {
    setSettings((prev) => {
      const next = { ...prev, ...patch };
      try {
        localStorage.setItem(KEY, JSON.stringify(next));
      } catch {
        // Sem armazenamento, a escolha vale só até fechar o app.
      }
      return next;
    });
  }, []);
  return [settings, update] as const;
}

/** O GIF escolhido, ou o primeiro disponível se o escolhido foi removido. */
export function selectedPet(settings: PetSettings, names: string[] | null): string | null {
  if (!names || names.length === 0) return null;
  return settings.pet && names.includes(settings.pet) ? settings.pet : names[0];
}
