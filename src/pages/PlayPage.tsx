import { motion } from "framer-motion";
import { Play, Plus, Square, Terminal, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { CustomSelect } from "../components/CustomSelect";
import type { DownloadProgress, GameInstance } from "../types/launcher";
import logoUrl from "../assets/logo.png";

interface PlayPageProps {
  instances: GameInstance[];
  activeInstance?: GameInstance;
  onSelectInstance: (id: string) => void;
  onLaunch: (id: string) => void;
  onKill: () => void;
  onGoToInstances: () => void;
  isLaunching: boolean;
  isGameRunning: boolean;
  progress: DownloadProgress | null;
  logs: string[];
  onClearLogs: () => void;
}

export function PlayPage({
  instances,
  activeInstance,
  onSelectInstance,
  onLaunch,
  onKill,
  onGoToInstances,
  isLaunching,
  isGameRunning,
  progress,
  logs,
  onClearLogs,
}: PlayPageProps) {
  const { t } = useTranslation();

  const isFailed = progress?.message === "Launch Failed";
  const pct =
    progress && progress.total > 0
      ? Math.min(100, Math.round((progress.current / progress.total) * 100))
      : 0;

  return (
    <div className="h-full flex flex-col gap-5">
      {/* Hero Card */}
      <div className="relative z-30 @container rounded-sm bg-gradient-to-br from-[#131b21] via-[#0f161b] to-[#0c1317] border border-white/15 p-6 shadow-xl">
        <div className="absolute inset-0 overflow-hidden rounded-sm pointer-events-none">
          <div className="absolute right-8 bottom-4 opacity-10">
            <img src={logoUrl} alt="" className="w-36 h-36 rounded-sm" />
          </div>
        </div>

        {activeInstance ? (
          <div className="relative z-30 flex items-center justify-between gap-4">
            <div className="min-w-0 flex-1 space-y-3">
              <div className="inline-flex items-center gap-2 px-2.5 py-1 rounded-sm bg-emerald-500/15 border border-emerald-500/30 text-emerald-300 text-xs font-medium">
                <span className="w-1.5 h-1.5 bg-emerald-400" />
                {isLaunching
                  ? t("play.launching")
                  : isGameRunning
                  ? t("play.running")
                  : t("play.readyToLaunch")}
              </div>

              <div className="min-w-0">
                <h1 className="text-3xl font-bold tracking-tight text-white truncate">
                  {activeInstance.name}
                </h1>
                <div className="flex flex-wrap items-center gap-2 mt-1.5 text-sm text-slate-400">
                  <span className="px-2 py-0.5 rounded-sm bg-white/5 border border-white/10 text-slate-200 font-mono text-xs">
                    MC {activeInstance.mcVersion}
                  </span>
                  <span className="px-2 py-0.5 rounded-sm bg-emerald-500/10 border border-emerald-500/25 text-emerald-300 capitalize text-xs font-medium">
                    {activeInstance.loader}
                    {activeInstance.loaderVersion ? ` ${activeInstance.loaderVersion}` : ""}
                  </span>
                </div>
              </div>

              {instances.length > 1 && (
                <div className="pt-1 w-full max-w-64">
                  <CustomSelect
                    value={activeInstance.id}
                    onChange={(val) => onSelectInstance(val)}
                    options={instances.map((inst) => ({
                      value: inst.id,
                      label: `${inst.name} (${inst.mcVersion} - ${inst.loader})`,
                    }))}
                  />
                </div>
              )}
            </div>

            <div className="shrink-0 flex items-center gap-2.5">
              <button
                type="button"
                disabled={!isGameRunning}
                onClick={onKill}
                title={t("play.killInstance")}
                className="flex items-center justify-center gap-2 px-4 @[680px]:px-5 py-4 rounded-sm bg-rose-500/15 hover:bg-rose-500 border border-rose-500/40 text-rose-300 hover:text-white disabled:opacity-35 font-bold text-sm transition-colors cursor-pointer disabled:cursor-not-allowed"
              >
                <Square className="w-4 h-4 fill-current shrink-0" />
                <span className="hidden @[680px]:inline whitespace-nowrap">
                  {t("play.killInstance")}
                </span>
              </button>

              <motion.button
                whileHover={{ scale: isLaunching ? 1 : 1.02 }}
                whileTap={{ scale: isLaunching ? 1 : 0.98 }}
                disabled={isLaunching}
                onClick={() => onLaunch(activeInstance.id)}
                title={isLaunching ? t("play.launching") : t("play.launchGame")}
                className="flex items-center justify-center gap-3 px-5 @[680px]:px-8 py-4 rounded-sm bg-gradient-to-r from-emerald-500 to-teal-500 hover:from-emerald-400 hover:to-teal-400 disabled:opacity-60 text-slate-950 font-bold text-base shadow-[0_0_30px_-5px_rgba(16,185,129,0.5)] transition-all cursor-pointer disabled:cursor-not-allowed"
              >
                <Play className="w-5 h-5 fill-current shrink-0" />
                <span className="hidden @[680px]:inline whitespace-nowrap">
                  {isLaunching ? t("play.launching") : t("play.launchGame")}
                </span>
              </motion.button>
            </div>
          </div>
        ) : (
          <div className="relative z-10 flex flex-col items-start gap-4 py-4">
            <div>
              <h1 className="text-2xl font-bold text-white">{t("play.noInstanceTitle")}</h1>
              <p className="text-sm text-slate-400 mt-1">{t("play.noInstanceDesc")}</p>
            </div>
            <button
              onClick={onGoToInstances}
              className="flex items-center gap-2 px-5 py-2.5 rounded-sm bg-emerald-500 hover:bg-emerald-400 text-slate-950 font-semibold text-sm transition-colors cursor-pointer"
            >
              <Plus className="w-4 h-4" />
              {t("play.createFirst")}
            </button>
          </div>
        )}

        {progress && (
          <div className="relative z-10 mt-5 pt-4 border-t border-white/10 space-y-2">
            <div className="flex items-center justify-between text-xs">
              <span className={isFailed ? "text-rose-400 font-bold" : "text-slate-300"}>
                {progress.message}
              </span>
              <span className={`font-mono ${isFailed ? "text-rose-400" : "text-emerald-300"}`}>
                {pct}%
              </span>
            </div>
            <div className="h-2 w-full rounded-sm bg-white/5 overflow-hidden">
              <motion.div
                className={`h-full ${
                  isFailed
                    ? "bg-rose-500"
                    : "bg-gradient-to-r from-emerald-500 to-teal-400"
                }`}
                initial={{ width: 0 }}
                animate={{ width: `${isFailed ? 100 : pct}%` }}
                transition={{ duration: 0.2 }}
              />
            </div>
          </div>
        )}
      </div>

      {/* Console Logs (Text selection enabled via select-text) */}
      <div className="relative z-10 flex-1 min-h-0 flex flex-col rounded-sm bg-[#0a0e11] border border-white/15 overflow-hidden">
        <div className="flex items-center justify-between px-4 py-2.5 border-b border-white/10 bg-white/[0.02]">
          <div className="flex items-center gap-2 text-xs font-semibold text-slate-300">
            <Terminal className="w-3.5 h-3.5 text-emerald-400" />
            <span>{t("play.console")}</span>
          </div>
          <button
            onClick={onClearLogs}
            className="flex items-center gap-1 text-xs text-slate-400 hover:text-slate-200 transition-colors cursor-pointer"
          >
            <Trash2 className="w-3.5 h-3.5" />
            <span>{t("play.clearLogs")}</span>
          </button>
        </div>
        <div className="flex-1 overflow-y-auto p-3 font-mono text-xs text-slate-300 space-y-1 select-text cursor-text">
          {logs.length === 0 ? (
            <div className="text-slate-500 italic select-none">Waiting for game output...</div>
          ) : (
            logs.map((line, i) => (
              <div
                key={i}
                className={
                  line.startsWith("[ERR]")
                    ? "text-rose-400 break-all"
                    : "text-slate-300 break-all"
                }
              >
                {line}
              </div>
            ))
          )}
        </div>
      </div>
    </div>
  );
}
