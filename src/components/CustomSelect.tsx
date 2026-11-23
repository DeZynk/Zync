import { Check, ChevronDown } from "lucide-react";
import { useEffect, useRef, useState } from "react";

export interface SelectOption {
  value: string;
  label: string;
}

interface CustomSelectProps {
  value: string;
  onChange: (value: string) => void;
  options: SelectOption[];
  disabled?: boolean;
  placeholder?: string;
  searchable?: boolean;
  searchPlaceholder?: string;
  className?: string;
}

export function CustomSelect({
  value,
  onChange,
  options,
  disabled = false,
  placeholder = "Select...",
  searchable = false,
  searchPlaceholder = "Type to search...",
  className = "",
}: CustomSelectProps) {
  const [open, setOpen] = useState(false);
  const [search, setSearch] = useState("");
  const containerRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  const selectedOption = options.find((o) => o.value === value);

  const filteredOptions =
    searchable && search.trim()
      ? options.filter((o) => {
          const q = search.trim().toLowerCase();
          return o.label.toLowerCase().includes(q) || o.value.toLowerCase().includes(q);
        })
      : options;

  useEffect(() => {
    if (!open) {
      setSearch("");
      return;
    }
    if (searchable) {
      setTimeout(() => inputRef.current?.focus(), 0);
    }
    const handleClickOutside = (e: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", handleClickOutside);
    document.addEventListener("keydown", handleKeyDown);
    return () => {
      document.removeEventListener("mousedown", handleClickOutside);
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [open, searchable]);

  return (
    <div ref={containerRef} className={`relative select-none ${className}`}>
      {searchable && open ? (
        <div className="w-full flex items-center justify-between gap-2 px-3.5 py-2 rounded-sm bg-[#0a0e11] border border-emerald-500/70 text-xs text-white">
          <input
            ref={inputRef}
            type="text"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                if (filteredOptions.length > 0) {
                  onChange(filteredOptions[0].value);
                  setOpen(false);
                }
              }
            }}
            placeholder={selectedOption ? selectedOption.label : searchPlaceholder}
            className="w-full bg-transparent text-xs text-white placeholder:text-slate-400 focus:outline-none"
          />
          <button
            type="button"
            onClick={() => setOpen(false)}
            className="text-emerald-400 shrink-0 cursor-pointer"
          >
            <ChevronDown className="w-3.5 h-3.5 rotate-180" />
          </button>
        </div>
      ) : (
        <button
          type="button"
          disabled={disabled}
          onClick={() => !disabled && setOpen((prev) => !prev)}
          className={`w-full flex items-center justify-between gap-2 px-3.5 py-2 rounded-sm bg-[#0a0e11] border text-left text-xs text-white transition-colors cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed ${
            open ? "border-emerald-500/70" : "border-white/15 hover:border-white/30"
          }`}
        >
          <span className="truncate">
            {selectedOption ? selectedOption.label : placeholder}
          </span>
          <ChevronDown
            className={`w-3.5 h-3.5 text-slate-400 shrink-0 transition-transform duration-150 ${
              open ? "rotate-180 text-emerald-400" : ""
            }`}
          />
        </button>
      )}

      {open && (
        <div className="absolute left-0 right-0 top-full mt-1 z-[100] max-h-52 overflow-y-auto rounded-sm bg-[#0d1318] border border-emerald-500/40 shadow-[0_12px_32px_rgba(0,0,0,0.9)] py-1">
          {filteredOptions.length === 0 ? (
            <div className="px-3.5 py-2 text-xs text-slate-500">No matches found</div>
          ) : (
            filteredOptions.map((opt) => {
              const isSelected = opt.value === value;
              return (
                <button
                  type="button"
                  key={opt.value}
                  onClick={() => {
                    onChange(opt.value);
                    setOpen(false);
                  }}
                  className={`w-full flex items-center justify-between gap-2 px-3.5 py-2 text-left text-xs transition-colors cursor-pointer ${
                    isSelected
                      ? "bg-emerald-500/20 text-emerald-300 font-semibold"
                      : "text-slate-200 hover:bg-white/[0.06] hover:text-white"
                  }`}
                >
                  <span className="truncate">{opt.label}</span>
                  {isSelected && <Check className="w-3.5 h-3.5 text-emerald-400 shrink-0" />}
                </button>
              );
            })
          )}
        </div>
      )}
    </div>
  );
}
