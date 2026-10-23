import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "@fontsource-variable/bricolage-grotesque/opsz.css";
import "./App.css";
import "./pet/pet.css";
import { TopBar, type Page } from "./components/TopBar";
import { GithubPage } from "./pages/GithubPage";
import { Home } from "./pages/Home";
import { Settings } from "./pages/Settings";
import { usePetAnimation, usePetLibrary } from "./pet/library";
import { Playground } from "./pet/Playground";
import { selectedPet, usePetSettings } from "./pet/settings";
import { analyzerAgents } from "./components/Analyzer";
import type { GithubState } from "./components/Commits";
import { usePrAnalyses, type PullRequestsState } from "./components/PullRequests";
import type {
  AccountStatus,
  AgentId,
  AgentInfo,
  AgentUsage,
  CaptureStatus,
  Commit,
  GithubCalendar,
  Limits,
  LiveSession,
  PullRequests,
} from "./types";

const TITLES: Record<Page, string> = {
  home: "Visão geral",
  github: "GitHub",
  settings: "Configurações",
};

/** Agentes cujo uso vem de `get_agent_usage` (o do Claude tem comandos próprios). */
const OTHER_AGENTS = ["opencode", "codex", "gemini"] as const;

const EMPTY_PERIOD = { sessions: 0, responses: 0, cost: null, topModel: null };

/** Status das sessões e porcentagens mudam sem aviso; são lidos a cada poucos segundos. */
const LIVE_POLL_MS = 3000;
/**
 * Commits novos chegam pelo evento `commits-changed`, assim que o git grava a referência.
 * A releitura periódica só cobre o que o observador não vê (ex.: repositório recém-criado).
 */
const COMMITS_POLL_MS = 60_000;
/** O gráfico do GitHub vem da internet; o próprio GitHub demora para atualizá-lo. */
const GITHUB_POLL_MS = 10 * 60_000;
/** PRs também vêm da internet; além disso, são relidos ao abrir a aba GitHub. */
const PRS_POLL_MS = 5 * 60_000;

function App() {
  const [page, setPage] = useState<Page>("home");
  const [now, setNow] = useState(Date.now());
  const [live, setLive] = useState<LiveSession[]>([]);
  const [limits, setLimits] = useState<Limits | null>(null);
  const [capture, setCapture] = useState<CaptureStatus | null>(null);
  const [account, setAccount] = useState<AccountStatus | null>(null);
  const [agents, setAgents] = useState<AgentInfo[] | null>(null);
  // O refresh periódico lê a lista atual sem precisar ser recriado a cada mudança.
  const agentsRef = useRef<AgentInfo[] | null>(null);
  const [usage, setUsage] = useState<Partial<Record<AgentId, AgentUsage>>>({});
  const [commits, setCommits] = useState<Commit[] | null>(null);
  const [calendar, setCalendar] = useState<GithubCalendar | null>(null);
  const [calendarError, setCalendarError] = useState<string | null>(null);
  const [calendarAt, setCalendarAt] = useState<number | null>(null);
  const [prs, setPrs] = useState<PullRequests | null>(null);
  const [prsError, setPrsError] = useState<string | null>(null);
  const [prsAt, setPrsAt] = useState<number | null>(null);
  const prAnalyses = usePrAnalyses(analyzerAgents(agents));
  const [error, setError] = useState<string | null>(null);
  const [pet, updatePet] = usePetSettings();
  const petLibrary = usePetLibrary();
  const { anim: petAnim, error: petError } = usePetAnimation(pet.enabled ? selectedPet(pet, petLibrary.names) : null);

  const refresh = useCallback(() => {
    const on = (id: AgentId) => agentsRef.current?.some((a) => a.id === id && a.connected) ?? false;
    const claude = on("claude");
    Promise.all([
      claude ? invoke<LiveSession[]>("get_live_sessions") : Promise.resolve([]),
      claude ? invoke<Limits | null>("get_limits") : Promise.resolve(null),
      // Ficam nas Configurações mesmo com o Claude desconectado.
      invoke<CaptureStatus>("get_capture_status"),
      invoke<AccountStatus>("get_account_usage"),
    ])
      .then(([l, lim, cap, acc]) => {
        setLive(l);
        setLimits(lim);
        setCapture(cap);
        setAccount(acc);
        setNow(Date.now());
        setError(null);
      })
      .catch((e) => setError(String(e)));
    for (const id of OTHER_AGENTS) {
      if (!on(id)) continue;
      invoke<AgentUsage>("get_agent_usage", { agent: id })
        .catch(
          (e): AgentUsage => ({
            installed: true,
            active: [],
            today: EMPTY_PERIOD,
            week: EMPTY_PERIOD,
            limits: null,
            error: String(e),
          }),
        )
        .then((u) => setUsage((prev) => ({ ...prev, [id]: u })));
    }
  }, []);

  const applyAgents = useCallback(
    (list: AgentInfo[]) => {
      agentsRef.current = list;
      setAgents(list);
      refresh();
    },
    [refresh],
  );

  useEffect(() => {
    invoke<AgentInfo[]>("get_agents")
      .then(applyAgents)
      .catch((e) => setError(String(e)));
  }, [applyAgents]);

  const setAgentConnected = useCallback(
    async (agent: AgentId, connected: boolean) => {
      applyAgents(await invoke<AgentInfo[]>("set_agent_connected", { agent, connected }));
    },
    [applyAgents],
  );

  const refreshCommits = useCallback(() => {
    invoke<Commit[]>("get_commits")
      .then(setCommits)
      .catch(() => setCommits([]));
  }, []);

  const refreshGithub = useCallback(async () => {
    try {
      setCalendar(await invoke<GithubCalendar>("get_github_calendar"));
      setCalendarAt(Date.now());
      setCalendarError(null);
    } catch (e) {
      // Mantém o último gráfico baixado; só avisa do erro se nunca houve um.
      setCalendarError(String(e));
    }
  }, []);

  useEffect(() => {
    refreshGithub();
    const id = setInterval(refreshGithub, GITHUB_POLL_MS);
    return () => clearInterval(id);
  }, [refreshGithub]);

  const github: GithubState = { calendar, error: calendarError, fetchedAt: calendarAt, refresh: refreshGithub };

  const refreshPrs = useCallback(async () => {
    try {
      setPrs(await invoke<PullRequests>("get_pull_requests"));
      setPrsAt(Date.now());
      setPrsError(null);
    } catch (e) {
      // Mantém a última lista baixada e mostra o erro ao lado dela.
      setPrsError(String(e));
    }
  }, []);

  useEffect(() => {
    refreshPrs();
    const id = setInterval(refreshPrs, PRS_POLL_MS);
    return () => clearInterval(id);
  }, [refreshPrs]);

  useEffect(() => {
    if (page === "github") refreshPrs();
  }, [page, refreshPrs]);

  const prsState: PullRequestsState = {
    data: prs,
    error: prsError,
    fetchedAt: prsAt,
    refresh: refreshPrs,
    analyses: prAnalyses,
  };

  useEffect(() => {
    refresh();
    refreshCommits();
    const unlisten = listen("usage-updated", refresh);
    // Commit, push ou troca de branch num repositório observado: relê na hora.
    const unlistenCommits = listen("commits-changed", refreshCommits);
    const live = setInterval(refresh, LIVE_POLL_MS);
    const commitsTimer = setInterval(refreshCommits, COMMITS_POLL_MS);
    return () => {
      clearInterval(live);
      clearInterval(commitsTimer);
      unlisten.then((f) => f());
      unlistenCommits.then((f) => f());
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
      <TopBar page={page} onNavigate={setPage} badges={{ github: prs?.reviewRequested.length }} />
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
              account={account}
              onEnableCapture={() => setCaptureEnabled(true)}
              sessions={live}
              onSessionsChanged={refresh}
              agents={agents}
              usage={usage}
              onConnect={(id) => setAgentConnected(id, true)}
              prsWaiting={prs ? prs.reviewRequested.length : null}
              commits={commits}
              onOpenGithub={() => setPage("github")}
            />
          )}
          {page === "github" && <GithubPage commits={commits} github={github} prs={prsState} now={now} />}
          {page === "settings" && (
            <Settings
              pet={pet}
              onPetChange={updatePet}
              library={petLibrary}
              capture={capture}
              onCaptureChange={setCaptureEnabled}
              account={account}
              onAccountChange={async (enabled) => {
                setAccount(await invoke<AccountStatus>("set_account_usage", { enabled }));
                refresh();
              }}
              agents={agents}
              onAgentChange={setAgentConnected}
            />
          )}
        </main>
      </div>
    </div>
  );
}

export default App;
