import { invoke } from "@tauri-apps/api/core";
import type {
  Account,
  ContentSearchResult,
  ContentSource,
  ContentType,
  DeviceCodeResponse,
  GameInstance,
  InstalledMod,
  JavaRuntime,
  LauncherConfig,
  LoaderType,
  LoaderVersionInfo,
  McVersionSummary,
} from "../types/launcher";

export const api = {
  // Config
  loadConfig: () => invoke<LauncherConfig>("load_config"),
  saveConfig: (config: LauncherConfig) => invoke<LauncherConfig>("save_config", { config }),
  getConfigPath: () => invoke<string>("get_config_path"),
  openConfigFolder: () => invoke<void>("open_config_folder"),

  // Java
  detectJavaRuntimes: () => invoke<JavaRuntime[]>("detect_java_runtimes"),
  testJavaRuntime: (path: string) => invoke<JavaRuntime>("test_java_runtime", { path }),
  downloadJavaRuntime: (majorVersion: number) =>
    invoke<JavaRuntime>("download_java_runtime", { majorVersion }),

  // Instances
  listInstances: () => invoke<GameInstance[]>("list_instances"),
  createInstance: (
    name: string,
    mcVersion: string,
    loader: LoaderType,
    loaderVersion?: string | null
  ) => invoke<GameInstance>("create_instance", { name, mcVersion, loader, loaderVersion }),
  updateInstance: (instance: GameInstance) =>
    invoke<GameInstance>("update_instance", { instance }),
  deleteInstance: (id: string) => invoke<void>("delete_instance", { id }),
  openInstanceFolder: (id: string) => invoke<void>("open_instance_folder", { id }),

  // Versions & Loaders
  listMinecraftVersions: (includeSnapshots: boolean) =>
    invoke<McVersionSummary[]>("list_minecraft_versions", { includeSnapshots }),
  listLoaderVersions: (loader: LoaderType, mcVersion: string) =>
    invoke<LoaderVersionInfo[]>("list_loader_versions", { loader, mcVersion }),
  launchInstance: (instanceId: string) => invoke<void>("launch_instance", { instanceId }),
  killInstance: () => invoke<void>("kill_instance"),

  // Content Download (Modrinth & CurseForge)
  searchContent: (
    source: ContentSource,
    contentType: ContentType,
    query: string,
    mcVersion?: string | null,
    loader?: LoaderType | null,
    limit?: number
  ) =>
    invoke<ContentSearchResult[]>("search_content", {
      source,
      contentType,
      query,
      mcVersion,
      loader,
      limit,
    }),
  installContentItem: (
    source: ContentSource,
    contentType: ContentType,
    instanceId: string,
    projectId: string,
    mcVersion?: string | null,
    loader?: LoaderType | null
  ) =>
    invoke<string>("install_content_item", {
      source,
      contentType,
      instanceId,
      projectId,
      mcVersion,
      loader,
    }),
  installModpack: (source: ContentSource, projectId: string, title: string) =>
    invoke<GameInstance>("install_modpack", { source, projectId, title }),

  // Installed Mods Management
  listInstalledMods: (instanceId: string) =>
    invoke<InstalledMod[]>("list_installed_mods", { instanceId }),
  toggleMod: (instanceId: string, filename: string) =>
    invoke<InstalledMod>("toggle_mod", { instanceId, filename }),
  deleteMod: (instanceId: string, filename: string) =>
    invoke<void>("delete_mod", { instanceId, filename }),

  // Auth
  addOfflineAccount: (username: string) => invoke<Account>("add_offline_account", { username }),
  removeAccount: (accountId: string) => invoke<void>("remove_account", { accountId }),
  setActiveAccount: (accountId: string) => invoke<void>("set_active_account", { accountId }),
  startMicrosoftLogin: (clientId?: string | null) =>
    invoke<DeviceCodeResponse>("start_microsoft_login", { clientId }),
  completeMicrosoftLogin: (deviceCode: string, clientId?: string | null) =>
    invoke<Account | null>("complete_microsoft_login", { clientId, deviceCode }),
};
