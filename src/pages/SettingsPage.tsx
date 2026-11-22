import { Check, Copy, FolderOpen, LogIn, Plus, Save, Trash2, User } from "lucide-react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { CustomSelect } from "../components/CustomSelect";
import { SUPPORTED_LANGUAGES } from "../i18n";
import { api } from "../lib/api";
import type { DeviceCodeResponse, LauncherConfig } from "../types/launcher";

interface SettingsPageProps {
  config: LauncherConfig;
  onUpdateConfig: (cfg: LauncherConfig) => Promise<void>;
  onReloadConfig: () => Promise<void>;
  onError: (msg: string) => void;
}

export function SettingsPage({
  config,
  onUpdateConfig,
  onReloadConfig,
  onError,
}: SettingsPageProps) {
  const { t, i18n } = useTranslation();
  const [draft, setDraft] = useState<LauncherConfig>(config);
  const [configPath, setConfigPath] = useState("");
  const [savedBanner, setSavedBanner] = useState(false);
  const [offlineName, setOfflineName] = useState("");
  const [deviceCode, setDeviceCode] = useState<DeviceCodeResponse | null>(null);
  const [copiedCode, setCopiedCode] = useState(false);

  useEffect(() => {
    setDraft(config);
  }, [config]);

  useEffect(() => {
    api.getConfigPath().then(setConfigPath).catch(() => {});
  }, []);

  const handleSave = async (e: React.FormEvent) => {
    e.preventDefault();
    try {
      await onUpdateConfig(draft);
      if (draft.language !== i18n.language) {
        await i18n.changeLanguage(draft.language);
      }
      setSavedBanner(true);
      setTimeout(() => setSavedBanner(false), 2000);
    } catch (err) {
      onError(String(err));
    }
  };

  const handleAddOffline = async () => {
    if (!offlineName.trim()) return;
    try {
      await api.addOfflineAccount(offlineName.trim());
      setOfflineName("");
      await onReloadConfig();
    } catch (err) {
      onError(String(err));
    }
  };

  const handleCopyCode = async (code: string) => {
    try {
      await navigator.clipboard.writeText(code);
      setCopiedCode(true);
      setTimeout(() => setCopiedCode(false), 2000);
    } catch {
      // ignore clipboard errors
    }
  };

  const handleStartMsLogin = async () => {
    try {
      const res = await api.startMicrosoftLogin(draft.msClientId);
      setDeviceCode(res);
      handleCopyCode(res.userCode);

      const timer = setInterval(async () => {
        try {
          const acc = await api.completeMicrosoftLogin(res.deviceCode, draft.msClientId);
          if (acc) {
            clearInterval(timer);
            setDeviceCode(null);
            await onReloadConfig();
          }
        } catch (err) {
          clearInterval(timer);
          setDeviceCode(null);
          onError(String(err));
        }
      }, (res.interval || 5) * 1000);
    } catch (err) {
      onError(String(err));
    }
  };

  return (
    <form onSubmit={handleSave} className="h-full flex flex-col gap-5 overflow-y-auto pr-1">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-white">{t("settings.title")}</h1>
          <p className="text-sm text-slate-400">{t("settings.subtitle")}</p>
        </div>
        <button
          type="submit"
          className="flex items-center gap-2 px-5 py-2.5 rounded-sm bg-emerald-500 hover:bg-emerald-400 text-slate-950 font-semibold text-sm transition-colors cursor-pointer"
        >
          {savedBanner ? <Check className="w-4 h-4" /> : <Save className="w-4 h-4" />}
          {savedBanner ? t("settings.saved") : t("settings.save")}
        </button>
      </div>

      {/* General & i18n */}
      <section className="rounded-sm bg-[#11171c] border border-white/15 p-5 space-y-4">
        <h2 className="text-sm font-semibold text-emerald-300">{t("settings.general")}</h2>
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          <div className="space-y-1.5">
            <label className="text-xs font-medium text-slate-300">{t("settings.language")}</label>
            <CustomSelect
              value={draft.language}
              onChange={(val) => setDraft({ ...draft, language: val })}
              options={SUPPORTED_LANGUAGES.map((lang) => ({
                value: lang.code,
                label: lang.label,
              }))}
            />
          </div>

          <div className="space-y-1.5">
            <label className="text-xs font-medium text-slate-300">{t("settings.dataDir")}</label>
            <input
              value={draft.dataDir}
              onChange={(e) => setDraft({ ...draft, dataDir: e.target.value })}
              className="w-full px-3 py-2 rounded-sm bg-[#0a0e11] border border-white/15 text-sm text-white font-mono focus:outline-none focus:border-emerald-500/60"
            />
          </div>
        </div>

        <div className="flex flex-wrap gap-6 pt-1">
          <label className="flex items-center gap-2.5 text-xs text-slate-300 cursor-pointer">
            <input
              type="checkbox"
              checked={draft.enableAnimations}
              onChange={(e) => setDraft({ ...draft, enableAnimations: e.target.checked })}
              className="accent-emerald-500 w-4 h-4"
            />
            {t("settings.animations")}
          </label>
          <label className="flex items-center gap-2.5 text-xs text-slate-300 cursor-pointer">
            <input
              type="checkbox"
              checked={draft.closeOnLaunch}
              onChange={(e) => setDraft({ ...draft, closeOnLaunch: e.target.checked })}
              className="accent-emerald-500 w-4 h-4"
            />
            {t("settings.closeOnLaunch")}
          </label>
        </div>
      </section>

      {/* Java & Performance */}
      <section className="rounded-sm bg-[#11171c] border border-white/15 p-5 space-y-4">
        <h2 className="text-sm font-semibold text-emerald-300">{t("settings.javaMemory")}</h2>

        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          <div className="space-y-1.5">
            <div className="flex justify-between text-xs text-slate-300">
              <span>{t("settings.minMemory")}</span>
              <span className="font-mono text-emerald-300">{draft.minMemoryMb} MB</span>
            </div>
            <input
              type="range"
              min={512}
              max={8192}
              step={256}
              value={draft.minMemoryMb}
              onChange={(e) => setDraft({ ...draft, minMemoryMb: Number(e.target.value) })}
              className="w-full accent-emerald-500"
            />
          </div>

          <div className="space-y-1.5">
            <div className="flex justify-between text-xs text-slate-300">
              <span>{t("settings.maxMemory")}</span>
              <span className="font-mono text-emerald-300">{draft.maxMemoryMb} MB</span>
            </div>
            <input
              type="range"
              min={1024}
              max={16384}
              step={512}
              value={draft.maxMemoryMb}
              onChange={(e) => setDraft({ ...draft, maxMemoryMb: Number(e.target.value) })}
              className="w-full accent-emerald-500"
            />
          </div>
        </div>

        <div className="space-y-1.5">
          <label className="text-xs font-medium text-slate-300">{t("settings.jvmArgs")}</label>
          <input
            value={draft.jvmArgs}
            onChange={(e) => setDraft({ ...draft, jvmArgs: e.target.value })}
            className="w-full px-3 py-2 rounded-sm bg-[#0a0e11] border border-white/15 text-xs text-white font-mono focus:outline-none focus:border-emerald-500/60"
          />
        </div>

        <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
          <div className="space-y-1.5">
            <label className="text-xs font-medium text-slate-300">
              {t("settings.concurrentDownloads")}
            </label>
            <input
              type="number"
              min={1}
              max={64}
              value={draft.maxConcurrentDownloads}
              onChange={(e) =>
                setDraft({ ...draft, maxConcurrentDownloads: Number(e.target.value) || 8 })
              }
              className="w-full px-3 py-2 rounded-sm bg-[#0a0e11] border border-white/15 text-xs text-white font-mono focus:outline-none focus:border-emerald-500/60"
            />
          </div>

          <div className="space-y-1.5">
            <label className="text-xs font-medium text-slate-300">
              {t("settings.windowSize")} (W)
            </label>
            <input
              type="number"
              value={draft.windowWidth}
              onChange={(e) => setDraft({ ...draft, windowWidth: Number(e.target.value) || 854 })}
              className="w-full px-3 py-2 rounded-sm bg-[#0a0e11] border border-white/15 text-xs text-white font-mono focus:outline-none focus:border-emerald-500/60"
            />
          </div>

          <div className="space-y-1.5">
            <label className="text-xs font-medium text-slate-300">
              {t("settings.windowSize")} (H)
            </label>
            <input
              type="number"
              value={draft.windowHeight}
              onChange={(e) => setDraft({ ...draft, windowHeight: Number(e.target.value) || 480 })}
              className="w-full px-3 py-2 rounded-sm bg-[#0a0e11] border border-white/15 text-xs text-white font-mono focus:outline-none focus:border-emerald-500/60"
            />
          </div>
        </div>
      </section>

      {/* Accounts */}
      <section className="rounded-sm bg-[#11171c] border border-white/15 p-5 space-y-4">
        <h2 className="text-sm font-semibold text-emerald-300">{t("settings.accounts")}</h2>

        <div className="flex flex-wrap items-center gap-3">
          <button
            type="button"
            onClick={handleStartMsLogin}
            className="flex items-center gap-2 px-4 py-2.5 rounded-sm bg-emerald-500 hover:bg-emerald-400 text-slate-950 text-xs font-bold cursor-pointer transition-colors"
          >
            <LogIn className="w-4 h-4" />
            {t("settings.msLogin")}
          </button>

          <div className="h-5 w-px bg-white/10 hidden sm:block" />

          <div className="flex items-center gap-2">
            <input
              value={offlineName}
              onChange={(e) => setOfflineName(e.target.value)}
              placeholder={t("settings.offlinePlaceholder")}
              className="px-3 py-2 rounded-sm bg-[#0a0e11] border border-white/15 text-xs text-white focus:outline-none focus:border-emerald-500/60"
            />
            <button
              type="button"
              onClick={handleAddOffline}
              className="flex items-center gap-1.5 px-3.5 py-2 rounded-sm bg-white/5 hover:bg-white/10 border border-white/10 text-xs text-slate-200 font-medium cursor-pointer"
            >
              <Plus className="w-3.5 h-3.5" />
              {t("settings.addOffline")}
            </button>
          </div>
        </div>

        {deviceCode && (
          <div className="p-4 rounded-sm bg-emerald-500/10 border border-emerald-500/40 space-y-3">
            <div className="text-xs text-emerald-200">{deviceCode.message}</div>
            <div className="flex items-center gap-3">
              <div className="px-4 py-2 rounded-sm bg-[#0a0e11] border border-emerald-500/40 font-mono text-lg font-bold tracking-widest text-white select-all">
                {deviceCode.userCode}
              </div>
              <button
                type="button"
                onClick={() => handleCopyCode(deviceCode.userCode)}
                className="flex items-center gap-1.5 px-3.5 py-2 rounded-sm bg-emerald-500 hover:bg-emerald-400 text-slate-950 text-xs font-bold cursor-pointer"
              >
                {copiedCode ? <Check className="w-3.5 h-3.5" /> : <Copy className="w-3.5 h-3.5" />}
                {copiedCode ? t("settings.copied") : t("settings.copyCode")}
              </button>
            </div>
            <div className="text-[11px] text-slate-400">{t("settings.waitingAuth")}</div>
          </div>
        )}

        {config.accounts.length > 0 && (
          <div className="space-y-2 pt-1">
            {config.accounts.map((acc) => {
              const active = acc.id === config.activeAccountId;
              return (
                <div
                  key={acc.id}
                  className={`flex items-center justify-between p-3 rounded-sm border ${
                    active
                      ? "bg-emerald-500/10 border-emerald-500/40"
                      : "bg-[#0a0e11] border-white/10"
                  }`}
                >
                  <button
                    type="button"
                    onClick={() => api.setActiveAccount(acc.id).then(onReloadConfig)}
                    className="flex items-center gap-2.5 text-left cursor-pointer"
                  >
                    <User className="w-4 h-4 text-emerald-400" />
                    <div>
                      <div className="text-xs font-semibold text-white">{acc.username}</div>
                      <div className="text-[11px] text-slate-400 capitalize">{acc.accountType}</div>
                    </div>
                  </button>
                  <button
                    type="button"
                    onClick={() => api.removeAccount(acc.id).then(onReloadConfig)}
                    className="p-1.5 rounded-sm text-slate-400 hover:text-rose-400 hover:bg-rose-500/10 cursor-pointer"
                  >
                    <Trash2 className="w-3.5 h-3.5" />
                  </button>
                </div>
              );
            })}
          </div>
        )}
      </section>

      {/* External Config File */}
      <section className="rounded-sm bg-[#11171c] border border-white/15 p-5 flex items-center justify-between gap-4">
        <div className="min-w-0">
          <h2 className="text-sm font-semibold text-white">{t("settings.configSection")}</h2>
          <div className="text-xs text-slate-400 font-mono truncate mt-0.5">{configPath}</div>
        </div>
        <button
          type="button"
          onClick={() => api.openConfigFolder()}
          className="shrink-0 flex items-center gap-2 px-3.5 py-2 rounded-sm bg-white/5 hover:bg-white/10 border border-white/10 text-xs text-slate-200 font-medium cursor-pointer"
        >
          <FolderOpen className="w-4 h-4" />
          {t("settings.openConfigFolder")}
        </button>
      </section>
    </form>
  );
}
