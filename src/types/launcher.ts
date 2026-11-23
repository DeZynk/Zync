export type LoaderType = "vanilla" | "fabric" | "quilt" | "forge" | "neoforge";
export type AccountType = "offline" | "microsoft";
export type ContentSource = "modrinth" | "curseforge";
export type ContentType = "mod" | "resourcepack" | "shader" | "modpack";

export interface Account {
  id: string;
  username: string;
  uuid: string;
  accountType: AccountType;
  accessToken?: string | null;
  refreshToken?: string | null;
  expiresAt?: number | null;
}

export interface LauncherConfig {
  language: string;
  dataDir: string;
  defaultJavaPath?: string | null;
  minMemoryMb: number;
  maxMemoryMb: number;
  jvmArgs: string;
  maxConcurrentDownloads: number;
  windowWidth: number;
  windowHeight: number;
  closeOnLaunch: boolean;
  enableAnimations: boolean;
  instancesViewStyle: "grid" | "list";
  sidebarCollapsed: boolean;
  activeInstanceId?: string | null;
  activeAccountId?: string | null;
  accounts: Account[];
  msClientId?: string | null;
}

export interface JavaRuntime {
  path: string;
  version: string;
  majorVersion: number;
  vendor: string;
  isManaged: boolean;
}

export interface GameInstance {
  id: string;
  name: string;
  mcVersion: string;
  loader: LoaderType;
  loaderVersion?: string | null;
  javaPath?: string | null;
  minMemoryMb?: number | null;
  maxMemoryMb?: number | null;
  jvmArgs?: string | null;
  createdAt: string;
  lastPlayed?: string | null;
}

export interface McVersionSummary {
  id: string;
  versionType: string;
  releaseTime: string;
  url: string;
}

export interface LoaderVersionInfo {
  version: string;
  stable: boolean;
}

export interface ContentSearchResult {
  projectId: string;
  slug: string;
  title: string;
  description: string;
  iconUrl?: string | null;
  downloads: number;
  author: string;
  source: ContentSource;
  contentType: ContentType;
}

export interface InstalledMod {
  filename: string;
  name: string;
  enabled: boolean;
  sizeBytes: number;
}

export interface DownloadProgress {
  task: string;
  current: number;
  total: number;
  message: string;
}

export interface DeviceCodeResponse {
  userCode: string;
  deviceCode: string;
  verificationUri: string;
  expiresIn: number;
  interval: number;
  message: string;
}

export interface ExportOptions {
  instanceId: string;
  packageAllFiles: boolean;
  keepSettings: boolean;
  includeKeybindsAndVolume: boolean;
  includeMultiplayerServers: boolean;
  includeJeiData: boolean;
  includeModConfigs: boolean;
  includeResourcePacks: boolean;
  includeShaderPacks: boolean;
}

export interface ExportScanSummary {
  totalMods: number;
  modrinthMatched: number;
  curseforgeMatched: number;
  unlistedMods: string[];
}

export interface ExportResult {
  outputPath: string;
  totalMods: number;
  modrinthMatched: number;
  curseforgeMatched: number;
  bundledDirectly: number;
  unlistedMods: string[];
}
