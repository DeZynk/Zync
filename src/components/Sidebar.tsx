import { motion } from "framer-motion";
import { Coffee, Download, Layers, Play, Settings, User } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { Account } from "../types/launcher";

export type NavTab = "play" | "instances" | "download" | "java" | "settings";

interface SidebarProps {
  activeTab: NavTab;
  onSelectTab: (tab: NavTab) => void;
  activeAccount?: Account;
  enableAnimations: boolean;
}

const NAV_ITEMS: { id: NavTab; icon: typeof Play; labelKey: string }[] = [
  { id: "play", icon: Play, labelKey: "nav.play" },
  { id: "instances", icon: Layers, labelKey: "nav.instances" },
  { id: "download", icon: Download, labelKey: "nav.download" },
  { id: "java", icon: Coffee, labelKey: "nav.java" },
  { id: "settings", icon: Settings, labelKey: "nav.settings" },
];

export function Sidebar({
  activeTab,
  onSelectTab,
  activeAccount,
  enableAnimations,
}: SidebarProps) {
  const { t } = useTranslation();

  return (
    <aside className="w-56 shrink-0 bg-[#0f1418]/90 border-r border-white/10 flex flex-col justify-between p-3 select-none">
      <nav className="space-y-1 mt-1">
        {NAV_ITEMS.map((item) => {
          const Icon = item.icon;
          const isActive = activeTab === item.id;
          return (
            <button
              key={item.id}
              onClick={() => onSelectTab(item.id)}
              className={`relative w-full flex items-center gap-3 px-3.5 py-2.5 rounded-sm text-sm font-medium transition-colors cursor-pointer ${
                isActive ? "text-emerald-300" : "text-slate-400 hover:text-slate-100 hover:bg-white/[0.03]"
              }`}
            >
              {isActive && (
                <motion.div
                  layoutId="activeNavPill"
                  transition={
                    enableAnimations
                      ? { type: "spring", stiffness: 420, damping: 32 }
                      : { duration: 0 }
                  }
                  className="absolute inset-0 rounded-sm bg-emerald-500/12 border-l-2 border-emerald-400 border-y border-r border-y-emerald-500/20 border-r-emerald-500/20 shadow-[0_0_20px_-6px_rgba(16,185,129,0.35)]"
                />
              )}
              <Icon className="w-4 h-4 relative z-10" />
              <span className="relative z-10">{t(item.labelKey)}</span>
            </button>
          );
        })}
      </nav>

      <button
        onClick={() => onSelectTab("settings")}
        className="flex items-center gap-3 p-2.5 rounded-sm bg-white/[0.03] border border-white/10 hover:border-emerald-500/40 transition-colors text-left cursor-pointer"
      >
        <div className="w-8 h-8 rounded-sm bg-emerald-500/15 border border-emerald-500/30 flex items-center justify-center text-emerald-400 shrink-0">
          <User className="w-4 h-4" />
        </div>
        <div className="min-w-0">
          <div className="text-xs font-semibold text-slate-200 truncate">
            {activeAccount ? activeAccount.username : "Player"}
          </div>
          <div className="text-[11px] text-slate-400 capitalize">
            {activeAccount ? activeAccount.accountType : "offline"}
          </div>
        </div>
      </button>
    </aside>
  );
}
