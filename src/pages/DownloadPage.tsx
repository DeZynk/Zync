import { motion } from "framer-motion";
import {
  Boxes,
  Check,
  Coffee,
  Download,
  Gamepad2,
  Image,
  Package,
  Search,
  Sparkles,
} from "lucide-react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { CustomSelect } from "../components/CustomSelect";
import { api } from "../lib/api";
import type {
  ContentSearchResult,
  ContentSource,
  ContentType,
  DownloadProgress,
  GameInstance,
  JavaRuntime,
  LauncherConfig,
  LoaderType,
  LoaderVersionInfo,
  McVersionSummary,
} from "../types/launcher";

export type DownloadCategory =
  | "game"
  | "java"
  | "mod"
  | "resourcepack"
  | "modpack"
  | "shader";

interface DownloadPageProps {
  initialCategory?: DownloadCategory;
  instances: GameInstance[];
  activeInstance?: GameInstance;
  config: LauncherConfig;
  progress: DownloadProgress | null;
  enableAnimations: boolean;
  onSelectInstance: (id: string) => void;
  onRefreshInstances: () => Promise<void>;
  onJavaInstalled: (rt: JavaRuntime) => Promise<void>;
  onError: (msg: string) => void;
}

const CATEGORIES: { id: DownloadCategory; icon: typeof Gamepad2; labelKey: string }[] = [
  { id: "game", icon: Gamepad2, labelKey: "download.tabs.game" },
  { id: "java", icon: Coffee, labelKey: "download.tabs.java" },
  { id: "mod", icon: Package, labelKey: "download.tabs.mod" },
  { id: "resourcepack", icon: Image, labelKey: "download.tabs.resourcepack" },
  { id: "modpack", icon: Boxes, labelKey: "download.tabs.modpack" },
  { id: "shader", icon: Sparkles, labelKey: "download.tabs.shader" },
];

const LOADERS: LoaderType[] = ["vanilla", "fabric", "forge", "neoforge", "quilt"];

const ADOPTIUM_VERSIONS = [
  { major: 21, note: "Recommended for MC 1.20.5 – 1.21+ & 26.x" },
  { major: 17, note: "Required for MC 1.17 – 1.20.4" },
  { major: 8, note: "Required for MC 1.16.5 & older (especially Forge)" },
];

export function DownloadPage({
  initialCategory = "game",
  instances,
  activeInstance,
  progress,
  enableAnimations,
  onSelectInstance,
  onRefreshInstances,
  onJavaInstalled,
  onError,
}: DownloadPageProps) {
  const { t } = useTranslation();
  const [category, setCategory] = useState<DownloadCategory>(initialCategory);

  useEffect(() => {
    setCategory(initialCategory);
  }, [initialCategory]);

  // --- 1. GAME INSTANCE DOWNLOAD STATE ---
  const [instName, setInstName] = useState("");
  const [mcVersions, setMcVersions] = useState<McVersionSummary[]>([]);
  const [includeSnapshots, setIncludeSnapshots] = useState(false);
  const [selectedMc, setSelectedMc] = useState("");
  const [selectedLoader, setSelectedLoader] = useState<LoaderType>("vanilla");
  const [loaderVersions, setLoaderVersions] = useState<LoaderVersionInfo[]>([]);
  const [selectedLoaderVer, setSelectedLoaderVer] = useState("");
  const [loadingVersions, setLoadingVersions] = useState(false);
  const [creatingInst, setCreatingInst] = useState(false);
  const [createdSuccess, setCreatedSuccess] = useState(false);

  useEffect(() => {
    if (category !== "game") return;
    setLoadingVersions(true);
    api
      .listMinecraftVersions(includeSnapshots)
      .then((list) => {
        setMcVersions(list);
        if (list.length > 0 && !list.some((v) => v.id === selectedMc)) {
          const defaultVer = list.find((v) => v.id.startsWith("1.21")) ?? list[0];
          setSelectedMc(defaultVer.id);
        }
      })
      .catch((e) => onError(String(e)))
      .finally(() => setLoadingVersions(false));
  }, [category, includeSnapshots]);

  useEffect(() => {
    if (category !== "game" || !selectedMc || selectedLoader === "vanilla") {
      setLoaderVersions([]);
      setSelectedLoaderVer("");
      return;
    }
    api
      .listLoaderVersions(selectedLoader, selectedMc)
      .then((list) => {
        setLoaderVersions(list);
        setSelectedLoaderVer(list[0]?.version ?? "");
      })
      .catch((e) => {
        setLoaderVersions([]);
        setSelectedLoaderVer("");
        onError(String(e));
      });
  }, [category, selectedMc, selectedLoader]);

  const handleCreateGameInstance = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!instName.trim() || !selectedMc) return;
    setCreatingInst(true);
    try {
      const created = await api.createInstance(
        instName.trim(),
        selectedMc,
        selectedLoader,
        selectedLoader === "vanilla" ? null : selectedLoaderVer || null
      );
      setInstName("");
      await onRefreshInstances();
      onSelectInstance(created.id);
      setCreatedSuccess(true);
      setTimeout(() => setCreatedSuccess(false), 2500);
    } catch (err) {
      onError(String(err));
    } finally {
      setCreatingInst(false);
    }
  };

  // --- 2. JAVA DOWNLOAD STATE ---
  const [downloadingJavaMajor, setDownloadingJavaMajor] = useState<number | null>(null);

  const handleInstallJava = async (major: number) => {
    setDownloadingJavaMajor(major);
    try {
      const rt = await api.downloadJavaRuntime(major);
      await onJavaInstalled(rt);
    } catch (e) {
      onError(String(e));
    } finally {
      setDownloadingJavaMajor(null);
    }
  };

  // --- 3. CONTENT (MODS / RESOURCEPACKS / MODPACKS / SHADERS) STATE ---
  const [source, setSource] = useState<ContentSource>("modrinth");
  const [query, setQuery] = useState("");
  const [matchInstanceVersion, setMatchInstanceVersion] = useState(true);
  const [results, setResults] = useState<ContentSearchResult[]>([]);
  const [searching, setSearching] = useState(false);
  const [installingId, setInstallingId] = useState<string | null>(null);
  const [installedIds, setInstalledIds] = useState<Record<string, boolean>>({});

  const isContentTab =
    category === "mod" ||
    category === "resourcepack" ||
    category === "modpack" ||
    category === "shader";

  // Clear results when switching content category or source
  useEffect(() => {
    setResults([]);
  }, [category, source]);

  const handleSearchContent = async (e?: React.FormEvent) => {
    e?.preventDefault();
    if (!isContentTab) return;
    setSearching(true);
    try {
      const contentType = category as ContentType;
      const mcFilter =
        matchInstanceVersion && category !== "modpack"
          ? activeInstance?.mcVersion ?? null
          : null;
      const loaderFilter =
        matchInstanceVersion && category === "mod"
          ? activeInstance?.loader ?? null
          : null;

      const list = await api.searchContent(
        source,
        contentType,
        query,
        mcFilter,
        loaderFilter,
        24
      );
      setResults(list);
    } catch (err) {
      onError(String(err));
    } finally {
      setSearching(false);
    }
  };

  const handleInstallContent = async (item: ContentSearchResult) => {
    setInstallingId(item.projectId);
    try {
      if (category === "modpack") {
        const inst = await api.installModpack(source, item.projectId, item.title);
        await onRefreshInstances();
        onSelectInstance(inst.id);
      } else {
        if (!activeInstance) {
          onError(t("download.content.noInstanceWarn"));
          return;
        }
        await api.installContentItem(
          source,
          category as ContentType,
          activeInstance.id,
          item.projectId,
          matchInstanceVersion ? activeInstance.mcVersion : "",
          matchInstanceVersion ? activeInstance.loader : "vanilla"
        );
      }
      setInstalledIds((prev) => ({ ...prev, [item.projectId]: true }));
    } catch (err) {
      onError(String(err));
    } finally {
      setInstallingId(null);
    }
  };

  return (
    <div className="h-full flex flex-col gap-4">
      {/* Header */}
      <div>
        <h1 className="text-2xl font-bold text-white">{t("download.title")}</h1>
        <p className="text-sm text-slate-400">{t("download.subtitle")}</p>
      </div>

      {/* Top Horizontal Selector Bar */}
      <div className="grid grid-cols-6 gap-1.5 p-1.5 rounded-sm bg-[#11171c] border border-white/15 select-none">
        {CATEGORIES.map((cat) => {
          const Icon = cat.icon;
          const isActive = category === cat.id;
          return (
            <button
              key={cat.id}
              onClick={() => setCategory(cat.id)}
              className={`relative flex items-center justify-center gap-2 py-2.5 px-3 rounded-sm text-xs font-semibold transition-colors cursor-pointer ${
                isActive
                  ? "text-slate-950"
                  : "text-slate-300 hover:text-white hover:bg-white/[0.04]"
              }`}
            >
              {isActive && (
                <motion.div
                  layoutId="downloadCategoryBar"
                  transition={
                    enableAnimations
                      ? { type: "spring", stiffness: 450, damping: 34 }
                      : { duration: 0 }
                  }
                  className="absolute inset-0 rounded-sm bg-emerald-500 shadow-[0_0_20px_-4px_rgba(16,185,129,0.5)]"
                />
              )}
              <Icon className="w-3.5 h-3.5 relative z-10 shrink-0" />
              <span className="relative z-10 truncate">{t(cat.labelKey)}</span>
            </button>
          );
        })}
      </div>

      {/* Main Content Area */}
      <div className="flex-1 min-h-0 flex flex-col">
        {/* 1. GAME TAB */}
        {category === "game" && (
          <form
            onSubmit={handleCreateGameInstance}
            className="rounded-sm bg-[#11171c] border border-white/15 p-6 space-y-5 max-w-2xl"
          >
            <h2 className="text-base font-bold text-white">{t("download.game.title")}</h2>

            <div className="space-y-1.5">
              <label className="text-xs font-medium text-slate-300">
                {t("download.game.name")}
              </label>
              <input
                required
                value={instName}
                onChange={(e) => setInstName(e.target.value)}
                placeholder={t("download.game.namePlaceholder")}
                className="w-full px-3.5 py-2.5 rounded-sm bg-[#0a0e11] border border-white/15 text-sm text-white focus:outline-none focus:border-emerald-500/60"
              />
            </div>

            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
              <div className="space-y-1.5">
                <div className="flex items-center justify-between">
                  <label className="text-xs font-medium text-slate-300">
                    {t("download.game.mcVersion")}
                  </label>
                  <label className="flex items-center gap-1.5 text-xs text-slate-400 cursor-pointer">
                    <input
                      type="checkbox"
                      checked={includeSnapshots}
                      onChange={(e) => setIncludeSnapshots(e.target.checked)}
                      className="accent-emerald-500"
                    />
                    {t("download.game.showSnapshots")}
                  </label>
                </div>
                <CustomSelect
                  value={selectedMc}
                  onChange={setSelectedMc}
                  disabled={loadingVersions}
                  searchable
                  searchPlaceholder="Search version (e.g. 1.20.1, 1.8.9)..."
                  placeholder="Loading versions..."
                  options={mcVersions.map((v) => ({
                    value: v.id,
                    label: `${v.id} (${v.versionType})`,
                  }))}
                />
              </div>

              {selectedLoader !== "vanilla" && (
                <div className="space-y-1.5">
                  <label className="text-xs font-medium text-slate-300">
                    {t("download.game.loaderVersion")}
                  </label>
                  <CustomSelect
                    value={selectedLoaderVer}
                    onChange={setSelectedLoaderVer}
                    searchable
                    searchPlaceholder="Search loader version..."
                    placeholder={`No loader versions for ${selectedMc}`}
                    options={loaderVersions.map((lv) => ({
                      value: lv.version,
                      label: `${lv.version}${lv.stable ? "" : " (beta)"}`,
                    }))}
                  />
                </div>
              )}
            </div>

            <div className="space-y-1.5">
              <label className="text-xs font-medium text-slate-300">
                {t("download.game.loader")}
              </label>
              <div className="grid grid-cols-5 gap-2">
                {LOADERS.map((l) => (
                  <button
                    type="button"
                    key={l}
                    onClick={() => setSelectedLoader(l)}
                    className={`py-2.5 rounded-sm text-xs font-semibold capitalize border transition-colors cursor-pointer ${
                      selectedLoader === l
                        ? "bg-emerald-500/20 border-emerald-500/60 text-emerald-300"
                        : "bg-[#0a0e11] border-white/15 text-slate-400 hover:text-white"
                    }`}
                  >
                    {l}
                  </button>
                ))}
              </div>
            </div>

            <div className="pt-2 flex items-center gap-3">
              <button
                type="submit"
                disabled={creatingInst || !selectedMc}
                className="flex items-center gap-2 px-6 py-2.5 rounded-sm bg-emerald-500 hover:bg-emerald-400 disabled:opacity-50 text-slate-950 font-bold text-sm transition-colors cursor-pointer"
              >
                {createdSuccess ? (
                  <>
                    <Check className="w-4 h-4" />
                    Instance Created!
                  </>
                ) : (
                  <>
                    <Download className="w-4 h-4" />
                    {creatingInst ? t("download.game.creating") : t("download.game.createBtn")}
                  </>
                )}
              </button>
            </div>
          </form>
        )}

        {/* 2. JAVA TAB */}
        {category === "java" && (
          <div className="rounded-sm bg-[#11171c] border border-white/15 p-6 space-y-5">
            <div>
              <h2 className="text-base font-bold text-white">{t("download.java.title")}</h2>
              <p className="text-xs text-slate-400 mt-1">{t("download.java.desc")}</p>
            </div>

            <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
              {ADOPTIUM_VERSIONS.map((item) => {
                const isBusy = downloadingJavaMajor === item.major;
                return (
                  <div
                    key={item.major}
                    className="p-5 rounded-sm bg-[#0a0e11] border border-white/15 flex flex-col justify-between gap-4"
                  >
                    <div>
                      <div className="text-lg font-bold text-white">
                        Java {item.major} (Temurin)
                      </div>
                      <div className="text-xs text-slate-400 mt-1">{item.note}</div>
                    </div>
                    <button
                      disabled={downloadingJavaMajor !== null}
                      onClick={() => handleInstallJava(item.major)}
                      className="flex items-center justify-center gap-2 py-2.5 rounded-sm bg-emerald-500 hover:bg-emerald-400 disabled:opacity-50 text-slate-950 text-xs font-bold transition-colors cursor-pointer"
                    >
                      <Download className="w-4 h-4" />
                      {isBusy
                        ? t("download.java.installing")
                        : t("download.java.installBtn", { version: item.major })}
                    </button>
                  </div>
                );
              })}
            </div>

            {downloadingJavaMajor !== null && progress && progress.task === "java" && (
              <div className="p-3 rounded-sm bg-emerald-500/10 border border-emerald-500/30 text-xs text-emerald-300 font-mono">
                {progress.message}
              </div>
            )}
          </div>
        )}

        {/* 3. CONTENT TABS: MODS / RESOURCEPACKS / MODPACKS / SHADERS */}
        {isContentTab && (
          <div className="flex-1 min-h-0 flex flex-col gap-3">
            {/* Filter & Search Bar */}
            <div className="flex flex-wrap items-center justify-between gap-3 p-3 rounded-sm bg-[#11171c] border border-white/15">
              {/* Source Switcher: Modrinth vs CurseForge */}
              <div className="flex items-center gap-2">
                <span className="text-xs text-slate-400 font-medium">
                  {t("download.content.source")}:
                </span>
                <div className="inline-flex rounded-sm bg-[#0a0e11] border border-white/15 p-0.5">
                  <button
                    type="button"
                    onClick={() => setSource("modrinth")}
                    className={`px-3 py-1 rounded-sm text-xs font-semibold transition-colors cursor-pointer ${
                      source === "modrinth"
                        ? "bg-emerald-500 text-slate-950"
                        : "text-slate-400 hover:text-white"
                    }`}
                  >
                    Modrinth
                  </button>
                  <button
                    type="button"
                    onClick={() => setSource("curseforge")}
                    className={`px-3 py-1 rounded-sm text-xs font-semibold transition-colors cursor-pointer ${
                      source === "curseforge"
                        ? "bg-amber-500 text-slate-950"
                        : "text-slate-400 hover:text-white"
                    }`}
                  >
                    CurseForge
                  </button>
                </div>
              </div>

              {/* Target Instance selector (for Mods, Resource Packs, Shaders) */}
              {category !== "modpack" && instances.length > 0 && (
                <div className="flex items-center gap-3">
                  <CustomSelect
                    value={activeInstance?.id ?? ""}
                    onChange={(val) => onSelectInstance(val)}
                    className="w-56"
                    options={instances.map((inst) => ({
                      value: inst.id,
                      label: `${inst.name} (${inst.mcVersion} • ${inst.loader})`,
                    }))}
                  />

                  <label className="flex items-center gap-1.5 text-xs text-slate-300 cursor-pointer">
                    <input
                      type="checkbox"
                      checked={matchInstanceVersion}
                      onChange={(e) => setMatchInstanceVersion(e.target.checked)}
                      className="accent-emerald-500"
                    />
                    {t("download.content.filterByVersion")}
                  </label>
                </div>
              )}

              {/* Search Input */}
              <form onSubmit={handleSearchContent} className="flex-1 min-w-[220px] flex gap-2">
                <div className="relative flex-1">
                  <Search className="w-3.5 h-3.5 text-slate-500 absolute left-3 top-1/2 -translate-y-1/2" />
                  <input
                    value={query}
                    onChange={(e) => setQuery(e.target.value)}
                    placeholder={t("download.content.searchPlaceholder", {
                      type: t(`download.tabs.${category}`),
                    })}
                    className="w-full pl-8 pr-3 py-1.5 rounded-sm bg-[#0a0e11] border border-white/15 text-xs text-white focus:outline-none focus:border-emerald-500/60"
                  />
                </div>
                <button
                  type="submit"
                  disabled={searching}
                  className="px-4 py-1.5 rounded-sm bg-emerald-500 hover:bg-emerald-400 text-slate-950 text-xs font-bold cursor-pointer"
                >
                  {searching ? "..." : t("download.content.search")}
                </button>
              </form>
            </div>

            {progress && progress.task === "modpack" && category === "modpack" && (
              <div className="p-2.5 rounded-sm bg-emerald-500/10 border border-emerald-500/30 text-xs text-emerald-300 font-mono">
                {progress.message}
              </div>
            )}

            {/* Results List */}
            <div className="flex-1 min-h-0 overflow-y-auto pr-1 space-y-2">
              {results.length === 0 ? (
                <div className="h-48 flex items-center justify-center rounded-sm border border-dashed border-white/15 text-sm text-slate-400">
                  Click "{t("download.content.search")}" to browse{" "}
                  {source === "modrinth" ? "Modrinth" : "CurseForge"}.
                </div>
              ) : (
                results.map((item) => {
                  const isBusy = installingId === item.projectId;
                  const isDone = installedIds[item.projectId];
                  return (
                    <div
                      key={`${item.source}-${item.projectId}`}
                      className="flex items-center justify-between gap-4 p-3.5 rounded-sm bg-[#11171c] border border-white/10 hover:border-white/25 transition-colors"
                    >
                      <div className="flex items-center gap-3.5 min-w-0">
                        {item.iconUrl ? (
                          <img
                            src={item.iconUrl}
                            alt={item.title}
                            className="w-11 h-11 rounded-sm object-cover bg-white/5 shrink-0"
                          />
                        ) : (
                          <div className="w-11 h-11 rounded-sm bg-white/5 flex items-center justify-center shrink-0">
                            <Package className="w-5 h-5 text-slate-500" />
                          </div>
                        )}
                        <div className="min-w-0">
                          <div className="flex items-center gap-2">
                            <h3 className="font-semibold text-sm text-white truncate">
                              {item.title}
                            </h3>
                            <span className="text-[11px] text-slate-400 shrink-0">
                              {t("download.content.by")} {item.author}
                            </span>
                            <span
                              className={`px-1.5 py-0.5 rounded-sm text-[10px] font-semibold uppercase ${
                                item.source === "modrinth"
                                  ? "bg-emerald-500/15 text-emerald-300"
                                  : "bg-amber-500/15 text-amber-300"
                              }`}
                            >
                              {item.source}
                            </span>
                          </div>
                          <p className="text-xs text-slate-400 truncate mt-0.5">
                            {item.description}
                          </p>
                          <div className="text-[11px] text-slate-500 mt-1">
                            {item.downloads.toLocaleString()} {t("download.content.downloads")}
                          </div>
                        </div>
                      </div>

                      <button
                        disabled={isBusy}
                        onClick={() => handleInstallContent(item)}
                        className={`shrink-0 flex items-center gap-1.5 px-3.5 py-2 rounded-sm text-xs font-semibold transition-colors cursor-pointer ${
                          isDone
                            ? "bg-white/5 text-emerald-300 border border-emerald-500/40"
                            : "bg-emerald-500 hover:bg-emerald-400 text-slate-950"
                        }`}
                      >
                        {isDone ? (
                          <>
                            <Check className="w-3.5 h-3.5" />
                            {t("download.content.installed")}
                          </>
                        ) : (
                          <>
                            <Download className="w-3.5 h-3.5" />
                            {isBusy
                              ? t("download.content.installing")
                              : category === "modpack"
                              ? t("download.content.installModpack")
                              : t("download.content.install")}
                          </>
                        )}
                      </button>
                    </div>
                  );
                })
              )}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
