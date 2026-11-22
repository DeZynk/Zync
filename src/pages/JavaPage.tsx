import { CheckCircle2, Coffee, Download, RefreshCw, Sparkles } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { api } from "../lib/api";
import type { JavaRuntime, LauncherConfig } from "../types/launcher";

interface JavaPageProps {
  config: LauncherConfig;
  runtimes: JavaRuntime[];
  onSetRuntimes: (list: JavaRuntime[]) => void;
  onUpdateConfig: (cfg: LauncherConfig) => Promise<void>;
  onGoToDownloadJava: () => void;
  onError: (msg: string) => void;
}

export function JavaPage({
  config,
  runtimes,
  onSetRuntimes,
  onUpdateConfig,
  onGoToDownloadJava,
  onError,
}: JavaPageProps) {
  const { t } = useTranslation();
  const [scanning, setScanning] = useState(false);

  const scan = async () => {
    setScanning(true);
    try {
      const list = await api.detectJavaRuntimes();
      onSetRuntimes(list);
    } catch (e) {
      onError(String(e));
    } finally {
      setScanning(false);
    }
  };

  const handleSetDefault = async (path: string) => {
    await onUpdateConfig({ ...config, defaultJavaPath: path });
  };

  return (
    <div className="h-full flex flex-col gap-5 overflow-y-auto pr-1">
      <div className="flex items-center justify-between gap-3">
        <div>
          <h1 className="text-2xl font-bold text-white">{t("java.title")}</h1>
          <p className="text-sm text-slate-400">{t("java.subtitle")}</p>
        </div>
        <div className="flex items-center gap-2">
          <button
            onClick={onGoToDownloadJava}
            className="flex items-center gap-2 px-4 py-2 rounded-sm bg-white/5 hover:bg-white/10 border border-white/15 text-slate-200 text-xs font-semibold transition-colors cursor-pointer"
          >
            <Download className="w-3.5 h-3.5" />
            {t("java.goToDownload")}
          </button>
          <button
            onClick={scan}
            disabled={scanning}
            className="flex items-center gap-2 px-4 py-2 rounded-sm bg-emerald-500/15 hover:bg-emerald-500/25 border border-emerald-500/30 text-emerald-300 text-xs font-semibold transition-colors cursor-pointer"
          >
            <RefreshCw className={`w-3.5 h-3.5 ${scanning ? "animate-spin" : ""}`} />
            {scanning ? t("java.scanning") : t("java.scan")}
          </button>
        </div>
      </div>

      {/* Smart Auto-Match Status Banner */}
      <div className="flex items-center gap-3 p-3.5 rounded-sm bg-emerald-500/10 border border-emerald-500/30 text-xs text-emerald-200">
        <Sparkles className="w-4 h-4 text-emerald-400 shrink-0" />
        <span>{t("java.autoMatchBanner")}</span>
      </div>

      {/* Detected Runtimes */}
      <div className="space-y-3">
        <h2 className="text-sm font-semibold text-slate-300">{t("java.detected")}</h2>
        {runtimes.length === 0 ? (
          <div className="p-8 rounded-sm border border-dashed border-white/15 text-center text-sm text-slate-400">
            {t("java.noJava")}
          </div>
        ) : (
          <div className="space-y-2.5">
            {runtimes.map((rt) => {
              const isDefault = config.defaultJavaPath === rt.path;
              return (
                <div
                  key={rt.path}
                  className={`flex items-center justify-between gap-4 p-4 rounded-sm bg-[#11171c] border ${
                    isDefault ? "border-emerald-500/50" : "border-white/10"
                  }`}
                >
                  <div className="flex items-center gap-3 min-w-0">
                    <div className="w-9 h-9 rounded-sm bg-emerald-500/10 border border-emerald-500/25 flex items-center justify-center text-emerald-400 shrink-0">
                      <Coffee className="w-4 h-4" />
                    </div>
                    <div className="min-w-0">
                      <div className="flex items-center gap-2">
                        <span className="font-semibold text-sm text-white">
                          Java {rt.majorVersion} ({rt.version})
                        </span>
                        <span className="text-xs text-slate-400">{rt.vendor}</span>
                        {rt.isManaged && (
                          <span className="px-2 py-0.5 rounded-sm bg-teal-500/15 border border-teal-500/25 text-teal-300 text-[10px] font-medium">
                            {t("java.managedBadge")}
                          </span>
                        )}
                      </div>
                      <div className="text-xs text-slate-500 font-mono truncate mt-0.5">
                        {rt.path}
                      </div>
                    </div>
                  </div>

                  {isDefault ? (
                    <span className="flex items-center gap-1 text-xs font-semibold text-emerald-400 shrink-0">
                      <CheckCircle2 className="w-4 h-4" />
                      {t("java.defaultBadge")}
                    </span>
                  ) : (
                    <button
                      onClick={() => handleSetDefault(rt.path)}
                      className="px-3 py-1.5 rounded-sm bg-white/5 hover:bg-white/10 border border-white/10 text-xs text-slate-200 font-medium shrink-0 cursor-pointer"
                    >
                      {t("java.setDefault")}
                    </button>
                  )}
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}
