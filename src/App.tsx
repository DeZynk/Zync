import { listen } from "@tauri-apps/api/event";
import { AnimatePresence, motion } from "framer-motion";
import { AlertCircle, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Sidebar, type NavTab } from "./components/Sidebar";
import { Titlebar } from "./components/Titlebar";
import { api } from "./lib/api";
import { DownloadPage, type DownloadCategory } from "./pages/DownloadPage";
import { InstancesPage } from "./pages/InstancesPage";
import { JavaPage } from "./pages/JavaPage";
import { PlayPage } from "./pages/PlayPage";
import { SettingsPage } from "./pages/SettingsPage";
import type {
  DownloadProgress,
  GameInstance,
  JavaRuntime,
  LauncherConfig,
} from "./types/launcher";

export function App() {
  const { i18n } = useTranslation();
  const [activeTab, setActiveTab] = useState<NavTab>("play");
  const [downloadCategory, setDownloadCategory] = useState<DownloadCategory>("game");
  const [config, setConfig] = useState<LauncherConfig | null>(null);
  const [instances, setInstances] = useState<GameInstance[]>([]);
  const [javaRuntimes, setJavaRuntimes] = useState<JavaRuntime[]>([]);
  const [isLaunching, setIsLaunching] = useState(false);
  const [isGameRunning, setIsGameRunning] = useState(false);
  const [progress, setProgress] = useState<DownloadProgress | null>(null);
  const [logs, setLogs] = useState<string[]>([]);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);
  const logBufferRef = useRef<string[]>([]);

  const loadAll = async () => {
    try {
      const [cfg, insts] = await Promise.all([api.loadConfig(), api.listInstances()]);
      setConfig(cfg);
      setInstances(insts);
      if (cfg.language && cfg.language !== i18n.language) {
        await i18n.changeLanguage(cfg.language);
      }
    } catch (e) {
      setErrorMsg(String(e));
    }
  };

  useEffect(() => {
    loadAll();

    // Flush buffered game logs every 250ms to prevent high-frequency React re-renders while playing
    const flushTimer = setInterval(() => {
      if (logBufferRef.current.length > 0) {
        const batch = logBufferRef.current.splice(0, logBufferRef.current.length);
        setLogs((prev) => [...prev.slice(-300), ...batch]);
      }
    }, 250);

    const unlistenProgress = listen<DownloadProgress>("download-progress", (e) => {
      setProgress(e.payload);
    });
    const unlistenLog = listen<string>("game-log", (e) => {
      logBufferRef.current.push(e.payload);
    });
    const unlistenExit = listen<number>("game-exited", (e) => {
      setIsLaunching(false);
      setIsGameRunning(false);
      logBufferRef.current.push(`[Zync] Game exited with code ${e.payload}`);
    });

    return () => {
      clearInterval(flushTimer);
      unlistenProgress.then((u) => u());
      unlistenLog.then((u) => u());
      unlistenExit.then((u) => u());
    };
  }, []);

  const handleUpdateConfig = async (next: LauncherConfig) => {
    const saved = await api.saveConfig(next);
    setConfig(saved);
  };

  const handleSelectInstance = async (id: string) => {
    if (!config) return;
    await handleUpdateConfig({ ...config, activeInstanceId: id });
  };

  const handleLaunch = async (id: string) => {
    setIsLaunching(true);
    setLogs([]);
    logBufferRef.current = [];
    setProgress({ task: "minecraft", current: 0, total: 100, message: "Starting launch..." });
    try {
      await handleSelectInstance(id);
      await api.launchInstance(id);
      // Game process spawned! Return Launch button to "Launch Minecraft" and enable "Kill Instance"
      setIsLaunching(false);
      setIsGameRunning(true);
    } catch (e) {
      setIsLaunching(false);
      setIsGameRunning(false);
      setProgress({
        task: "minecraft",
        current: 0,
        total: 100,
        message: "Launch Failed",
      });
      setErrorMsg(String(e));
    }
  };

  const handleKill = async () => {
    try {
      await api.killInstance();
      setIsGameRunning(false);
      setIsLaunching(false);
    } catch (e) {
      setErrorMsg(String(e));
    }
  };

  const openDownloadCategory = (cat: DownloadCategory) => {
    setDownloadCategory(cat);
    setActiveTab("download");
  };

  const activeInstance =
    instances.find((i) => i.id === config?.activeInstanceId) ?? instances[0];
  const activeAccount = (config?.accounts ?? []).find((a) => a.id === config?.activeAccountId);
  const enableAnimations = config?.enableAnimations ?? true;

  return (
    <div className="h-screen w-screen flex flex-col bg-[#0d1114] text-slate-100 overflow-hidden border border-white/10">
      <Titlebar />

      <div className="flex-1 min-h-0 flex">
        <Sidebar
          activeTab={activeTab}
          onSelectTab={setActiveTab}
          activeAccount={activeAccount}
          enableAnimations={enableAnimations}
          collapsed={config?.sidebarCollapsed ?? false}
          onToggleCollapse={() => {
            if (config) {
              handleUpdateConfig({
                ...config,
                sidebarCollapsed: !config.sidebarCollapsed,
              }).catch((e) => setErrorMsg(String(e)));
            }
          }}
        />

        <main className="flex-1 min-w-0 p-6 overflow-hidden relative">
          <AnimatePresence mode="wait">
            <motion.div
              key={activeTab}
              initial={enableAnimations ? { opacity: 0, y: 8 } : false}
              animate={{ opacity: 1, y: 0 }}
              exit={enableAnimations ? { opacity: 0, y: -8 } : undefined}
              transition={{ duration: enableAnimations ? 0.14 : 0 }}
              className="h-full"
            >
              {activeTab === "play" && (
                <PlayPage
                  instances={instances}
                  activeInstance={activeInstance}
                  onSelectInstance={handleSelectInstance}
                  onLaunch={handleLaunch}
                  onKill={handleKill}
                  onGoToInstances={() => openDownloadCategory("game")}
                  isLaunching={isLaunching}
                  isGameRunning={isGameRunning}
                  progress={progress}
                  logs={logs}
                  onClearLogs={() => setLogs([])}
                />
              )}

              {activeTab === "instances" && (
                <InstancesPage
                  instances={instances}
                  activeInstanceId={activeInstance?.id}
                  viewStyle={config?.instancesViewStyle === "list" ? "list" : "grid"}
                  onChangeViewStyle={(style) => {
                    if (config) {
                      handleUpdateConfig({ ...config, instancesViewStyle: style }).catch((e) =>
                        setErrorMsg(String(e))
                      );
                    }
                  }}
                  onSelectAndLaunch={(id) => {
                    setActiveTab("play");
                    handleLaunch(id);
                  }}
                  onGoToDownloadGame={() => openDownloadCategory("game")}
                  onRefresh={loadAll}
                  onError={setErrorMsg}
                />
              )}

              {activeTab === "download" && config && (
                <DownloadPage
                  initialCategory={downloadCategory}
                  instances={instances}
                  activeInstance={activeInstance}
                  config={config}
                  progress={progress}
                  enableAnimations={enableAnimations}
                  onSelectInstance={handleSelectInstance}
                  onRefreshInstances={loadAll}
                  onJavaInstalled={async (rt) => {
                    const list = await api.detectJavaRuntimes();
                    setJavaRuntimes(list);
                    if (!config.defaultJavaPath) {
                      await handleUpdateConfig({ ...config, defaultJavaPath: rt.path });
                    }
                  }}
                  onError={setErrorMsg}
                />
              )}

              {activeTab === "java" && config && (
                <JavaPage
                  config={config}
                  runtimes={javaRuntimes}
                  onSetRuntimes={setJavaRuntimes}
                  onUpdateConfig={handleUpdateConfig}
                  onGoToDownloadJava={() => openDownloadCategory("java")}
                  onError={setErrorMsg}
                />
              )}

              {activeTab === "settings" && config && (
                <SettingsPage
                  config={config}
                  onUpdateConfig={handleUpdateConfig}
                  onReloadConfig={loadAll}
                  onError={setErrorMsg}
                />
              )}
            </motion.div>
          </AnimatePresence>

          {/* Error Toast */}
          <AnimatePresence>
            {errorMsg && (
              <motion.div
                initial={{ opacity: 0, y: 20 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0, y: 20 }}
                className="fixed bottom-5 right-5 z-50 max-w-md flex items-start gap-3 p-4 rounded-sm bg-rose-950/95 border border-rose-500/50 text-rose-100 shadow-2xl backdrop-blur-md"
              >
                <AlertCircle className="w-5 h-5 text-rose-400 shrink-0 mt-0.5" />
                <div className="text-xs leading-relaxed flex-1">{errorMsg}</div>
                <button
                  onClick={() => setErrorMsg(null)}
                  className="text-rose-300 hover:text-white cursor-pointer"
                >
                  <X className="w-4 h-4" />
                </button>
              </motion.div>
            )}
          </AnimatePresence>
        </main>
      </div>
    </div>
  );
}

export default App;
