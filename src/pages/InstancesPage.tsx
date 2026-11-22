import { AnimatePresence, motion } from "framer-motion";
import {
  Download,
  FolderOpen,
  Layers,
  LayoutGrid,
  List,
  Package,
  Play,
  Sliders,
  Trash2,
  X,
} from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { api } from "../lib/api";
import type { GameInstance, InstalledMod } from "../types/launcher";

interface InstancesPageProps {
  instances: GameInstance[];
  activeInstanceId?: string | null;
  onSelectAndLaunch: (id: string) => void;
  onGoToDownloadGame: () => void;
  onRefresh: () => Promise<void>;
  onError: (msg: string) => void;
}

export function InstancesPage({
  instances,
  activeInstanceId,
  onSelectAndLaunch,
  onGoToDownloadGame,
  onRefresh,
  onError,
}: InstancesPageProps) {
  const { t } = useTranslation();
  const [viewStyle, setViewStyle] = useState<"grid" | "list">("grid");

  // Configure Instance Modal State
  const [editingInst, setEditingInst] = useState<GameInstance | null>(null);

  // Manage Installed Mods Modal State
  const [modsModalInst, setModsModalInst] = useState<GameInstance | null>(null);
  const [installedMods, setInstalledMods] = useState<InstalledMod[]>([]);

  const handleOpenModsModal = async (inst: GameInstance) => {
    setModsModalInst(inst);
    try {
      const list = await api.listInstalledMods(inst.id);
      setInstalledMods(list);
    } catch (e) {
      onError(String(e));
    }
  };

  const handleToggleMod = async (filename: string) => {
    if (!modsModalInst) return;
    try {
      await api.toggleMod(modsModalInst.id, filename);
      const list = await api.listInstalledMods(modsModalInst.id);
      setInstalledMods(list);
    } catch (e) {
      onError(String(e));
    }
  };

  const handleDeleteMod = async (filename: string) => {
    if (!modsModalInst) return;
    try {
      await api.deleteMod(modsModalInst.id, filename);
      const list = await api.listInstalledMods(modsModalInst.id);
      setInstalledMods(list);
    } catch (e) {
      onError(String(e));
    }
  };

  const handleSaveInstanceConfig = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!editingInst) return;
    try {
      await api.updateInstance(editingInst);
      setEditingInst(null);
      await onRefresh();
    } catch (err) {
      onError(String(err));
    }
  };

  const handleDelete = async (id: string) => {
    try {
      await api.deleteInstance(id);
      await onRefresh();
    } catch (err) {
      onError(String(err));
    }
  };

  return (
    <div className="h-full flex flex-col gap-5">
      <div className="flex items-center justify-between gap-4">
        <div>
          <h1 className="text-2xl font-bold text-white">{t("instances.title")}</h1>
          <p className="text-sm text-slate-400">{t("instances.subtitle")}</p>
        </div>

        {/* View Style Switcher: Grid / List */}
        <div className="inline-flex rounded-sm bg-[#11171c] border border-white/15 p-1">
          <button
            type="button"
            onClick={() => setViewStyle("grid")}
            className={`flex items-center gap-1.5 px-3 py-1.5 rounded-sm text-xs font-semibold transition-colors cursor-pointer ${
              viewStyle === "grid"
                ? "bg-emerald-500 text-slate-950"
                : "text-slate-400 hover:text-white"
            }`}
          >
            <LayoutGrid className="w-3.5 h-3.5" />
            <span>{t("instances.viewGrid")}</span>
          </button>
          <button
            type="button"
            onClick={() => setViewStyle("list")}
            className={`flex items-center gap-1.5 px-3 py-1.5 rounded-sm text-xs font-semibold transition-colors cursor-pointer ${
              viewStyle === "list"
                ? "bg-emerald-500 text-slate-950"
                : "text-slate-400 hover:text-white"
            }`}
          >
            <List className="w-3.5 h-3.5" />
            <span>{t("instances.viewList")}</span>
          </button>
        </div>
      </div>

      {instances.length === 0 ? (
        <div className="flex-1 flex flex-col items-center justify-center rounded-sm border border-dashed border-white/15 text-slate-400 gap-4">
          <Layers className="w-10 h-10 text-slate-600" />
          <p className="text-sm">{t("instances.empty")}</p>
          <button
            onClick={onGoToDownloadGame}
            className="flex items-center gap-2 px-4 py-2 rounded-sm bg-emerald-500 hover:bg-emerald-400 text-slate-950 font-semibold text-xs transition-colors cursor-pointer"
          >
            <Download className="w-3.5 h-3.5" />
            {t("instances.goToDownload")}
          </button>
        </div>
      ) : viewStyle === "grid" ? (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4 overflow-y-auto pr-1">
          {instances.map((inst) => {
            const isActive = inst.id === activeInstanceId;
            return (
              <motion.div
                key={inst.id}
                layout
                className={`rounded-sm p-4 bg-[#11171c] border transition-colors flex flex-col justify-between gap-4 ${
                  isActive
                    ? "border-emerald-500/50 shadow-[0_0_25px_-10px_rgba(16,185,129,0.35)]"
                    : "border-white/10 hover:border-white/25"
                }`}
              >
                <div>
                  <div className="flex items-start justify-between gap-2">
                    <h3 className="font-semibold text-white truncate">{inst.name}</h3>
                    <span className="px-2 py-0.5 rounded-sm bg-emerald-500/15 border border-emerald-500/25 text-emerald-300 text-xs capitalize shrink-0">
                      {inst.loader}
                    </span>
                  </div>
                  <div className="text-xs text-slate-400 mt-1 font-mono">
                    MC {inst.mcVersion}
                    {inst.loaderVersion ? ` • ${inst.loaderVersion}` : ""}
                  </div>
                </div>

                <div className="flex items-center gap-1.5 pt-2 border-t border-white/5">
                  <button
                    onClick={() => onSelectAndLaunch(inst.id)}
                    className="flex-1 flex items-center justify-center gap-1.5 py-2 rounded-sm bg-emerald-500/15 hover:bg-emerald-500 text-emerald-300 hover:text-slate-950 font-medium text-xs transition-colors cursor-pointer"
                  >
                    <Play className="w-3.5 h-3.5 fill-current" />
                    {t("instances.launch")}
                  </button>
                  <button
                    onClick={() => handleOpenModsModal(inst)}
                    className="p-2 rounded-sm bg-white/5 hover:bg-white/10 text-slate-300 transition-colors cursor-pointer"
                    title={t("instances.manageMods")}
                  >
                    <Package className="w-4 h-4" />
                  </button>
                  <button
                    onClick={() => setEditingInst({ ...inst })}
                    className="p-2 rounded-sm bg-white/5 hover:bg-white/10 text-slate-300 transition-colors cursor-pointer"
                    title={t("instances.configure")}
                  >
                    <Sliders className="w-4 h-4" />
                  </button>
                  <button
                    onClick={() => api.openInstanceFolder(inst.id)}
                    className="p-2 rounded-sm bg-white/5 hover:bg-white/10 text-slate-300 transition-colors cursor-pointer"
                    title={t("instances.openFolder")}
                  >
                    <FolderOpen className="w-4 h-4" />
                  </button>
                  <button
                    onClick={() => handleDelete(inst.id)}
                    className="p-2 rounded-sm bg-white/5 hover:bg-rose-500/20 text-slate-400 hover:text-rose-400 transition-colors cursor-pointer"
                    title={t("instances.delete")}
                  >
                    <Trash2 className="w-4 h-4" />
                  </button>
                </div>
              </motion.div>
            );
          })}
        </div>
      ) : (
        <div className="flex flex-col gap-2 overflow-y-auto pr-1">
          {instances.map((inst) => {
            const isActive = inst.id === activeInstanceId;
            return (
              <div
                key={inst.id}
                className={`rounded-sm px-4 py-3 bg-[#11171c] border transition-colors flex items-center justify-between gap-4 ${
                  isActive
                    ? "border-emerald-500/50 bg-emerald-500/[0.04]"
                    : "border-white/10 hover:border-white/25"
                }`}
              >
                <div className="flex items-center gap-4 min-w-0">
                  <div className="w-2 h-8 rounded-sm bg-emerald-500/40 shrink-0" />
                  <div className="min-w-0">
                    <div className="flex items-center gap-2.5">
                      <h3 className="font-semibold text-sm text-white truncate">{inst.name}</h3>
                      <span className="px-2 py-0.5 rounded-sm bg-white/5 border border-white/10 text-slate-300 font-mono text-[11px] shrink-0">
                        MC {inst.mcVersion}
                      </span>
                      <span className="px-2 py-0.5 rounded-sm bg-emerald-500/15 border border-emerald-500/25 text-emerald-300 text-[11px] capitalize shrink-0">
                        {inst.loader}
                        {inst.loaderVersion ? ` ${inst.loaderVersion}` : ""}
                      </span>
                    </div>
                  </div>
                </div>

                <div className="flex items-center gap-1.5 shrink-0">
                  <button
                    onClick={() => onSelectAndLaunch(inst.id)}
                    className="flex items-center gap-1.5 px-3.5 py-1.5 rounded-sm bg-emerald-500/15 hover:bg-emerald-500 text-emerald-300 hover:text-slate-950 font-semibold text-xs transition-colors cursor-pointer"
                  >
                    <Play className="w-3.5 h-3.5 fill-current" />
                    {t("instances.launch")}
                  </button>
                  <button
                    onClick={() => handleOpenModsModal(inst)}
                    className="flex items-center gap-1 px-2.5 py-1.5 rounded-sm bg-white/5 hover:bg-white/10 text-xs text-slate-300 transition-colors cursor-pointer"
                    title={t("instances.manageMods")}
                  >
                    <Package className="w-3.5 h-3.5" />
                    <span>{t("instances.manageMods")}</span>
                  </button>
                  <button
                    onClick={() => setEditingInst({ ...inst })}
                    className="p-1.5 rounded-sm bg-white/5 hover:bg-white/10 text-slate-300 transition-colors cursor-pointer"
                    title={t("instances.configure")}
                  >
                    <Sliders className="w-4 h-4" />
                  </button>
                  <button
                    onClick={() => api.openInstanceFolder(inst.id)}
                    className="p-1.5 rounded-sm bg-white/5 hover:bg-white/10 text-slate-300 transition-colors cursor-pointer"
                    title={t("instances.openFolder")}
                  >
                    <FolderOpen className="w-4 h-4" />
                  </button>
                  <button
                    onClick={() => handleDelete(inst.id)}
                    className="p-1.5 rounded-sm bg-white/5 hover:bg-rose-500/20 text-slate-400 hover:text-rose-400 transition-colors cursor-pointer"
                    title={t("instances.delete")}
                  >
                    <Trash2 className="w-4 h-4" />
                  </button>
                </div>
              </div>
            );
          })}
        </div>
      )}

      {/* Configure Instance Modal */}
      <AnimatePresence>
        {editingInst && (
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            className="fixed inset-0 z-50 bg-black/70 backdrop-blur-sm flex items-center justify-center p-4"
          >
            <motion.form
              initial={{ scale: 0.96, opacity: 0 }}
              animate={{ scale: 1, opacity: 1 }}
              exit={{ scale: 0.96, opacity: 0 }}
              onSubmit={handleSaveInstanceConfig}
              className="w-full max-w-md rounded-sm bg-[#12181d] border border-white/15 p-6 space-y-4 shadow-2xl"
            >
              <div className="flex items-center justify-between">
                <h2 className="text-lg font-bold text-white">
                  {t("instances.configure")}: {editingInst.name}
                </h2>
                <button
                  type="button"
                  onClick={() => setEditingInst(null)}
                  className="text-slate-400 hover:text-white cursor-pointer"
                >
                  <X className="w-5 h-5" />
                </button>
              </div>

              <div className="space-y-1.5">
                <label className="text-xs font-medium text-slate-300">{t("instances.name")}</label>
                <input
                  required
                  value={editingInst.name}
                  onChange={(e) => setEditingInst({ ...editingInst, name: e.target.value })}
                  className="w-full px-3.5 py-2 rounded-sm bg-[#0a0e11] border border-white/15 text-sm text-white focus:outline-none focus:border-emerald-500/60"
                />
              </div>

              <div className="space-y-1.5">
                <label className="text-xs font-medium text-slate-300">
                  {t("instances.javaOverride")}
                </label>
                <input
                  value={editingInst.javaPath ?? ""}
                  onChange={(e) =>
                    setEditingInst({
                      ...editingInst,
                      javaPath: e.target.value.trim() || null,
                    })
                  }
                  placeholder="Auto-match Java version based on Minecraft version"
                  className="w-full px-3.5 py-2 rounded-sm bg-[#0a0e11] border border-white/15 text-xs text-white font-mono focus:outline-none focus:border-emerald-500/60"
                />
              </div>

              <div className="space-y-1.5">
                <label className="text-xs font-medium text-slate-300">
                  {t("instances.maxMemoryOverride")}
                </label>
                <input
                  type="number"
                  min={0}
                  max={32768}
                  value={editingInst.maxMemoryMb ?? 0}
                  onChange={(e) => {
                    const val = Number(e.target.value);
                    setEditingInst({
                      ...editingInst,
                      maxMemoryMb: val > 0 ? val : null,
                    });
                  }}
                  className="w-full px-3.5 py-2 rounded-sm bg-[#0a0e11] border border-white/15 text-xs text-white font-mono focus:outline-none focus:border-emerald-500/60"
                />
              </div>

              <div className="space-y-1.5">
                <label className="text-xs font-medium text-slate-300">
                  {t("instances.jvmArgsOverride")}
                </label>
                <input
                  value={editingInst.jvmArgs ?? ""}
                  onChange={(e) =>
                    setEditingInst({
                      ...editingInst,
                      jvmArgs: e.target.value.trim() || null,
                    })
                  }
                  className="w-full px-3.5 py-2 rounded-sm bg-[#0a0e11] border border-white/15 text-xs text-white font-mono focus:outline-none focus:border-emerald-500/60"
                />
              </div>

              <div className="flex justify-end gap-2 pt-2">
                <button
                  type="button"
                  onClick={() => setEditingInst(null)}
                  className="px-4 py-2 rounded-sm bg-white/5 hover:bg-white/10 text-sm text-slate-300 cursor-pointer"
                >
                  {t("instances.cancel")}
                </button>
                <button
                  type="submit"
                  className="px-5 py-2 rounded-sm bg-emerald-500 hover:bg-emerald-400 text-slate-950 font-semibold text-sm cursor-pointer"
                >
                  {t("instances.save")}
                </button>
              </div>
            </motion.form>
          </motion.div>
        )}
      </AnimatePresence>

      {/* Installed Mods Modal */}
      <AnimatePresence>
        {modsModalInst && (
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            className="fixed inset-0 z-50 bg-black/70 backdrop-blur-sm flex items-center justify-center p-4"
          >
            <motion.div
              initial={{ scale: 0.96, opacity: 0 }}
              animate={{ scale: 1, opacity: 1 }}
              exit={{ scale: 0.96, opacity: 0 }}
              className="w-full max-w-lg max-h-[80vh] flex flex-col rounded-sm bg-[#12181d] border border-white/15 p-6 gap-4 shadow-2xl"
            >
              <div className="flex items-center justify-between">
                <h2 className="text-lg font-bold text-white">
                  {t("instances.installedModsTitle")} — {modsModalInst.name}
                </h2>
                <button
                  type="button"
                  onClick={() => setModsModalInst(null)}
                  className="text-slate-400 hover:text-white cursor-pointer"
                >
                  <X className="w-5 h-5" />
                </button>
              </div>

              <div className="flex-1 min-h-0 overflow-y-auto space-y-2 pr-1">
                {installedMods.length === 0 ? (
                  <div className="py-12 text-center text-sm text-slate-500">
                    {t("instances.noModsInstalled")}
                  </div>
                ) : (
                  installedMods.map((mod) => (
                    <div
                      key={mod.filename}
                      className="flex items-center justify-between gap-4 p-3 rounded-sm bg-[#0a0e11] border border-white/10"
                    >
                      <div className="min-w-0">
                        <div
                          className={`font-medium text-xs truncate ${
                            mod.enabled ? "text-white" : "text-slate-500 line-through"
                          }`}
                        >
                          {mod.name}
                        </div>
                        <div className="text-[11px] text-slate-500 font-mono">
                          {(mod.sizeBytes / 1024).toFixed(1)} KB
                        </div>
                      </div>
                      <div className="flex items-center gap-2 shrink-0">
                        <button
                          onClick={() => handleToggleMod(mod.filename)}
                          className={`px-2.5 py-1 rounded-sm text-xs font-medium cursor-pointer transition-colors ${
                            mod.enabled
                              ? "bg-emerald-500/15 border border-emerald-500/30 text-emerald-300"
                              : "bg-white/5 border border-white/10 text-slate-400"
                          }`}
                        >
                          {mod.enabled ? "Enabled" : "Disabled"}
                        </button>
                        <button
                          onClick={() => handleDeleteMod(mod.filename)}
                          className="p-1.5 rounded-sm bg-white/5 hover:bg-rose-500/20 text-slate-400 hover:text-rose-400 cursor-pointer"
                        >
                          <Trash2 className="w-3.5 h-3.5" />
                        </button>
                      </div>
                    </div>
                  ))
                )}
              </div>
            </motion.div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
