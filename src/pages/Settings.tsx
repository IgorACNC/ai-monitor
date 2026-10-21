import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { forgetPet, usePetAnimation } from "../pet/library";
import { PetCanvas } from "../pet/PetCanvas";
import { selectedPet, type PetSettings } from "../pet/settings";
import type { CaptureStatus } from "../types";

function PetOption({
  name,
  selected,
  onSelect,
  onRemove,
}: {
  name: string;
  selected: boolean;
  onSelect: () => void;
  onRemove: () => void;
}) {
  const { anim, error } = usePetAnimation(name);
  return (
    <div className={`pet-option${selected ? " selected" : ""}`}>
      <label className="pet-option-pick">
        <input type="radio" name="pet" checked={selected} onChange={onSelect} />
        <span className="pet-option-preview">
          {anim ? <PetCanvas anim={anim} playing /> : <span className="muted">{error ? "GIF inválido" : "Carregando…"}</span>}
        </span>
        <span className="pet-option-name">{name}</span>
      </label>
      <button type="button" className="pet-option-remove" onClick={onRemove} aria-label={`Remover ${name}`}>
        Remover
      </button>
    </div>
  );
}

function CaptureSection({
  capture,
  onChange,
}: {
  capture: CaptureStatus | null;
  onChange: (enabled: boolean) => Promise<void>;
}) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function toggle(enabled: boolean) {
    setBusy(true);
    setError(null);
    try {
      await onChange(enabled);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="settings-section">
      <div className="settings-intro">
        <h2>Porcentagens de uso</h2>
        <p>
          Para mostrar quanto dos limites de 5 horas e da semana já foi usado, e quanto do contexto cada sessão ocupa, o
          app lê a statusline do Claude Code. A statusline que já existia continua funcionando. Desligar devolve o
          settings.json do Claude Code como estava.
        </p>
      </div>
      <label className="toggle">
        <input
          type="checkbox"
          checked={capture?.enabled ?? false}
          disabled={busy || capture === null}
          onChange={(e) => toggle(e.target.checked)}
        />
        <span>Ler as porcentagens do Claude Code</span>
      </label>
      {error && <p className="inline-error">{error}</p>}
    </section>
  );
}

export function Settings({
  pet,
  onPetChange,
  library,
  capture,
  onCaptureChange,
}: {
  pet: PetSettings;
  onPetChange: (patch: Partial<PetSettings>) => void;
  library: { names: string[] | null; refresh: () => void };
  capture: CaptureStatus | null;
  onCaptureChange: (enabled: boolean) => Promise<void>;
}) {
  const [error, setError] = useState<string | null>(null);
  const current = selectedPet(pet, library.names);

  async function addGif() {
    setError(null);
    const path = await open({ multiple: false, filters: [{ name: "GIF", extensions: ["gif"] }] });
    if (typeof path !== "string") return;
    try {
      const name = await invoke<string>("add_pet", { path });
      library.refresh();
      onPetChange({ pet: name });
    } catch (e) {
      setError(String(e));
    }
  }

  async function removeGif(name: string) {
    setError(null);
    try {
      await invoke("remove_pet", { name });
      forgetPet(name);
      library.refresh();
    } catch (e) {
      setError(String(e));
    }
  }

  return (
    <div className="settings">
      <CaptureSection capture={capture} onChange={onCaptureChange} />
      <section className="settings-section">
        <div className="settings-intro">
          <h2>Bichinho</h2>
          <p>Ele vive na barra de cima. Clique lá para jogar a bolinha.</p>
        </div>

        <label className="toggle">
          <input type="checkbox" checked={pet.enabled} onChange={(e) => onPetChange({ enabled: e.target.checked })} />
          <span>Mostrar o bichinho</span>
        </label>

        <fieldset className="field" disabled={!pet.enabled}>
          <legend>Qual bichinho</legend>
          <p className="field-help">
            Qualquer GIF serve. Fundo liso e chão desenhado embaixo são removidos, e todos ficam do mesmo tamanho. Os GIFs
            ficam só neste computador.
          </p>
          {library.names && library.names.length === 0 && (
            <p className="muted">Nenhum GIF ainda. Adicione um para o bichinho aparecer.</p>
          )}
          <div className="pet-options">
            {library.names?.map((name) => (
              <PetOption
                key={name}
                name={name}
                selected={name === current}
                onSelect={() => onPetChange({ pet: name })}
                onRemove={() => removeGif(name)}
              />
            ))}
          </div>
          <div>
            <button type="button" className="button button-quiet" onClick={addGif}>
              Adicionar GIF
            </button>
          </div>
          {error && <p className="inline-error">{error}</p>}
        </fieldset>

        <label className="field" aria-disabled={!pet.enabled}>
          <span className="field-label">Nome</span>
          <input
            className="text-input"
            type="text"
            maxLength={24}
            value={pet.name}
            disabled={!pet.enabled}
            onChange={(e) => onPetChange({ name: e.target.value })}
          />
        </label>
      </section>
    </div>
  );
}
