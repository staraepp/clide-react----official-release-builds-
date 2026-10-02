import { useEffect, useState } from "react";
import { motion } from "motion/react";
import { Clock, Cpu, House, Settings2, type LucideIcon } from "lucide-react";

import { Keys } from "@/components/Keys";
import { StatusDot } from "@/components/StatusDot";
import { Wordmark } from "@/components/Wordmark";
import { stateLabel, stateTone } from "@/dictation/labels";
import { isBusy, type DictationState, type SystemStatus } from "@/lib/types";
import { cn } from "@/lib/cn";
import * as commands from "@/lib/commands";

export type View = "dashboard" | "models" | "history" | "settings";

const ITEMS: { value: View; label: string; icon: LucideIcon }[] = [
  { value: "dashboard", label: "Home", icon: House },
  { value: "history", label: "History", icon: Clock },
  { value: "models", label: "Models", icon: Cpu },
  { value: "settings", label: "Settings", icon: Settings2 },
];

/**
 * Navigation and status, down the left edge.
 *
 * The window uses macOS's overlay title bar, so the traffic lights float in the
 * top-left corner — hence the empty strip above the wordmark, which is also
 * where the window can be dragged from.
 */
export function Sidebar({
  view,
  onChange,
  status,
  state,
  levelRef,
}: {
  view: View;
  onChange: (view: View) => void;
  status: SystemStatus;
  state: DictationState;
  levelRef: React.RefObject<number>;
}) {
  // Cached natively for a day, so this is cheap after the first launch.
  const [updateVersion, setUpdateVersion] = useState<string | null>(null);
  useEffect(() => {
    commands
      .checkForUpdates(false)
      .then((update) =>
        setUpdateVersion(update.updateAvailable ? update.latestVersion : null),
      )
      .catch(() => setUpdateVersion(null));
  }, []);

  const label =
    state.kind === "idle"
      ? status.ready
        ? "Ready"
        : "Setup needed"
      : stateLabel(state);

  return (
    <aside className="flex w-[208px] shrink-0 flex-col px-3 pb-4">
      {/* Tauri's own handler, not the CSS property: `-webkit-app-region` is
          silently ignored by this webview, which left the window stuck. */}
      <div data-tauri-drag-region className="drag-region h-[52px] shrink-0" />

      <div
        data-tauri-drag-region
        className="display mb-5 flex items-center gap-2 px-2 text-[15px]"
      >
        <Wordmark levelRef={levelRef} live={state.kind === "capturing"} />
        clide
      </div>

      <nav className="flex flex-col gap-0.5">
        {ITEMS.map(({ value, label: text, icon: Icon }) => {
          const active = value === view;
          return (
            <button
              key={value}
              type="button"
              onClick={() => onChange(value)}
              aria-current={active ? "page" : undefined}
              className={cn(
                "relative flex items-center gap-2.5 rounded-lg px-2.5 py-1.5 text-left text-[13.5px] transition-colors",
                active ? "text-ink" : "text-ink-3 hover:text-ink",
              )}
            >
              {active && (
                <motion.span
                  layoutId="nav"
                  transition={{ type: "spring", stiffness: 460, damping: 38 }}
                  className="absolute inset-0 rounded-lg bg-voice-tint"
                />
              )}
              <Icon size={15} strokeWidth={1.75} className="relative z-10" />
              <span className="relative z-10">{text}</span>
            </button>
          );
        })}
      </nav>

      <div className="mt-auto flex flex-col gap-2 px-2.5">
        {updateVersion && (
          <button
            type="button"
            onClick={() => onChange("settings")}
            className="mb-1 rounded-lg bg-voice-tint px-2.5 py-1.5 text-left text-[12px] text-ink transition-opacity hover:opacity-80"
          >
            clide {updateVersion} is available
          </button>
        )}
        <div className="flex items-center gap-2 text-[12.5px] text-ink-2">
          <StatusDot
            tone={
              state.kind === "idle" && !status.ready ? "pending" : stateTone(state)
            }
            pulse={isBusy(state)}
          />
          {label}
        </div>
        <div className="flex items-center gap-1.5 text-[11.5px] text-ink-3">
          <Keys accelerator={status.settings.shortcut} />
        </div>
      </div>
    </aside>
  );
}
