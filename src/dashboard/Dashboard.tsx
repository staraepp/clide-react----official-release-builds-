import { DictationCard } from "./cards/DictationCard";
import { ProviderCard } from "./cards/ProviderCard";
import { ModeCard } from "./cards/ModeCard";
import { RecentCard } from "./cards/RecentCard";
import { UsageCard } from "./cards/UsageCard";
import { SystemCard } from "./cards/SystemCard";
import { StatusDot } from "@/components/StatusDot";
import { useDictationState } from "@/dictation/useDictationState";
import { useMicLevel } from "@/dictation/useMicLevel";
import type { SystemStatus } from "@/lib/types";

/**
 * Home: one column, one document.
 *
 * Dictation first, then the two choices that shape it, then what you have
 * said. Setup only takes space when something needs fixing.
 */
export function Dashboard({
  status,
  refresh,
  onNavigate,
}: {
  status: SystemStatus;
  refresh: () => void;
  onNavigate: (view: "models" | "history" | "settings") => void;
}) {
  const state = useDictationState();
  const level = useMicLevel();

  return (
    <div className="flex flex-col divide-y divide-line pt-2">
      <DictationCard state={state} status={status} levelRef={level} />

      <div className="grid grid-cols-1 gap-x-12 sm:grid-cols-2">
        <ProviderCard status={status} onConfigure={() => onNavigate("models")} />
        <ModeCard mode={status.settings.mode} onChange={refresh} />
      </div>

      <RecentCard onOpenHistory={() => onNavigate("history")} />

      {status.ready ? (
        <p className="flex items-center gap-2 py-5 text-[12.5px] text-ink-3">
          <StatusDot tone="ready" />
          Microphone, Accessibility and your shortcut are all set.
        </p>
      ) : (
        <SystemCard status={status} onRefresh={refresh} />
      )}

      <UsageCard />
    </div>
  );
}
