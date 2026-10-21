import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "@fontsource-variable/bricolage-grotesque/opsz.css";
import "./App.css";
import "./pet/pet.css";
import { TopBar, type Page } from "./components/TopBar";
import { CommitsPage } from "./pages/CommitsPage";
import { Home } from "./pages/Home";
import { Settings } from "./pages/Settings";
import { usePetAnimation, usePetLibrary } from "./pet/library";
import { Playground } from "./pet/Playground";
import { selectedPet, usePetSettings } from "./pet/settings";
import type { CaptureStatus, Commit, Limits, LiveSession } from "./types";

const TITLES: Record<Page, string> = {
  home: "Visão geral",
  commits: "Commits",
  settings: "Configurações",
};

/** Status das sessões e porcentagens mudam sem aviso; são lidos a cada poucos segundos. */
const LIVE_POLL_MS = 3000;
/** Ler os repositórios chama o git várias vezes, então é bem menos frequente. */
const COMMITS_POLL_MS = 60_000;

function App() {
  const [page, setPage] = useState<Page>("home");
  const [now, setNow] = useState(Date.now());
  const [live, setLive] = useState<LiveSession[]>([]);
  const [limits, setLimits] = useState<Limits | null>(null);
  const [capture, setCapture] = useState<CaptureStatus | null>(null);
  const [commits, setCommits] = useState<Commit[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [pet, updatePet] = usePetSettings();
  const petLibrary = usePetLibrary();
  const { anim: petAnim, error: petError } = usePetAnimation(pet.enabled ? selectedPet(pet, petLibrary.names) : null);

  const refresh = useCallback(() => {
    Promise.all([
      invoke<LiveSession[]>("get_live_sessions"),
      invoke<Limits | null>("get_limits"),
      invoke<CaptureStatus>("get_capture_status"),
    ])
      .then(([l, lim, cap]) => {
        setLive(l);
        setLimits(lim);
        setCapture(cap);
        setNow(Date.now());
        setError(null);
      })
      .catch((e) => setError(String(e)));
  }, []);

  const refreshCommits = useCallback(() => {
    invoke<Commit[]>("get_commits")
      .then(setCommits)
      .catch(() => setCommits([]));
  }, []);

  useEffect(() => {
    refresh();
    refreshCommits();
    const unlisten = listen("usage-updated", refresh);
    const live = setInterval(refresh, LIVE_POLL_MS);
    const commitsTimer = setInterval(refreshCommits, COMMITS_POLL_MS);
    return () => {
      clearInterval(live);
      clearInterval(commitsTimer);
      unlisten.then((f) => f());
    };
  }, [refresh, refreshCommits]);

  const setCaptureEnabled = useCallback(
    async (enabled: boolean) => {
      setCapture(await invoke<CaptureStatus>("set_capture", { enabled }));
      refresh();
    },
    [refresh],
  );

  return (
    <div className="shell">
      <TopBar page={page} onNavigate={setPage} />
      <div className="main">
        {pet.enabled && (
          <Playground
            settings={pet}
            anim={petAnim}
            notice={
              petError
                ? `Não foi possível mostrar o bichinho: ${petError}`
                : petLibrary.names?.length === 0
                  ? "Adicione um GIF de bichinho em Configurações para ele aparecer aqui."
                  : null
            }
          />
        )}

        <main className="content">
          <h1 className="page-title">{TITLES[page]}</h1>
          {error && <p className="message message-error">Não foi possível ler os dados do Claude Code: {error}</p>}
          {page === "home" && (
            <Home
              now={now}
              limits={limits}
              capture={capture}
              onEnableCapture={() => setCaptureEnabled(true)}
              sessions={live}
              onSessionsChanged={refresh}
              commits={commits}
              onSeeCommits={() => setPage("commits")}
            />
          )}
          {page === "commits" && <CommitsPage commits={commits} now={now} />}
          {page === "settings" && (
            <Settings
              pet={pet}
              onPetChange={updatePet}
              library={petLibrary}
              capture={capture}
              onCaptureChange={setCaptureEnabled}
            />
          )}
        </main>
      </div>
    </div>
  );
}

export default App;
