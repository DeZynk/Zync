import { getCurrentWindow } from "@tauri-apps/api/window";
import { Minus, Square, X } from "lucide-react";

export function Titlebar() {
  const appWindow = getCurrentWindow();

  return (
    <header
      data-tauri-drag-region
      className="h-10 shrink-0 flex items-center justify-between px-4 bg-[#0a0d10]/95 border-b border-white/10 select-none"
    >
      <div data-tauri-drag-region className="flex items-center pointer-events-none">
        <span className="text-xs font-bold tracking-[0.2em] text-emerald-400">
          ZYNC
        </span>
      </div>

      <div className="flex items-center gap-0.5">
        <button
          onClick={() => appWindow.minimize()}
          className="w-8 h-7 flex items-center justify-center rounded-sm text-slate-400 hover:text-white hover:bg-white/10 transition-colors"
          aria-label="Minimize"
        >
          <Minus className="w-3.5 h-3.5" />
        </button>
        <button
          onClick={() => appWindow.toggleMaximize()}
          className="w-8 h-7 flex items-center justify-center rounded-sm text-slate-400 hover:text-white hover:bg-white/10 transition-colors"
          aria-label="Maximize"
        >
          <Square className="w-3 h-3" />
        </button>
        <button
          onClick={() => appWindow.close()}
          className="w-8 h-7 flex items-center justify-center rounded-sm text-slate-400 hover:text-white hover:bg-rose-500/80 transition-colors"
          aria-label="Close"
        >
          <X className="w-3.5 h-3.5" />
        </button>
      </div>
    </header>
  );
}
