import { useEffect, useState } from "react";
import { Cpu } from "lucide-react";

import { Segmented } from "@/components/Segmented";
import { StatusDot } from "@/components/StatusDot";
import { Toggle } from "@/components/Toggle";
import { cn } from "@/lib/cn";
import * as commands from "@/lib/commands";
import type { RefinerDescriptor, SystemStatus } from "@/lib/types";

/**
 * Rewrite mode's engine and how far it may go.
 *
 * Lists every engine whether or not it is usable: an option that silently
 * disappears when Apple Intelligence is switched off leaves the user with no
 * way to understand why Rewrite stopped working.
 */
export function RefineSection({
  status,
  refresh,
}: {
  status: SystemStatus;
  refresh: () => void;
}) {
  const [refiners, setRefiners] = useState<RefinerDescriptor[]>([]);

  useEffect(() => {
    commands
      .listRefiners()
      .then(setRefiners)
      .catch((error) => console.error("could not list refiners", error));
  }, [status.settings.mode, status.settings.refineEngines]);

  // Only one engine ever runs: the first one that is both switched on and
  // able to. With several toggled on, nothing previously said which — so the
  // list showed three equal-looking options and no answer.
  const active = refiners.find(
    (refiner) =>
      status.settings.refineEngines.includes(refiner.id) && refiner.available,
  );

  return (
    <div className="flex flex-col gap-4">
      <p className="rounded-ctl border border-line bg-sunken/60 px-3 py-2.5 text-[12.5px] leading-relaxed text-ink-2">
        {active ? (
          <>
            Rewrite will use <strong className="text-ink">{active.name}</strong>.
            Your transcript stays on this Mac.
          </>
        ) : (
          <>
            No engine is switched on, so Rewrite returns your{" "}
            <strong className="text-ink">Polished</strong> transcript — spacing,
            punctuation and filler words cleaned up locally, with no delay.
          </>
        )}
      </p>
      <Segmented
        className="w-full"
        value={status.settings.refineStyle}
        onChange={async (style) => {
          await commands.setRefineStyle(style);
          refresh();
        }}
        segments={[
          {
            value: "tidy",
            label: "Tidy it up",
            hint: "Punctuation and slips only — your wording is kept",
          },
          {
            value: "written",
            label: "Make it written",
            hint: "Spoken phrasing becomes written prose",
          },
        ]}
      />

      <ul className="grid gap-2 sm:grid-cols-2">
        {refiners.map((refiner) => {
          const enabled = status.settings.refineEngines.includes(refiner.id);
          return (
            <li
              key={refiner.id}
              className={cn(
                "flex flex-col gap-2 rounded-ctl border bg-card px-3 py-3 transition-colors",
                enabled ? "border-voice bg-voice-tint/30" : "border-line",
              )}
            >
              <span className="flex items-center gap-2">
                <Cpu size={13} className="shrink-0 text-ink-3" />
                <span className="truncate text-[13px] text-ink">{refiner.name}</span>
                <Toggle
                  checked={enabled}
                  disabled={!refiner.available}
                  label={`Use ${refiner.name} for Rewrite`}
                  onChange={async (next) => {
                    await commands.setRefineEngineEnabled(refiner.id, next);
                    refresh();
                  }}
                />
              </span>

              <p className="text-[11.5px] leading-relaxed text-ink-3">
                {refiner.available ? refiner.description : refiner.unavailableReason}
              </p>

              <span className="mt-auto flex items-center gap-1.5">
                <StatusDot
                  tone={!refiner.available ? "problem" : enabled ? "ready" : "idle"}
                />
                <span className="text-[11px] text-ink-3">
                  {!refiner.available
                    ? "Unavailable"
                    : !enabled
                      ? "Off"
                      : refiner.id === active?.id
                        ? "In use · stays on this Mac"
                        : "On · used if the one above cannot run"}
                </span>
              </span>
            </li>
          );
        })}
      </ul>

      <p className="text-[12px] leading-relaxed text-ink-3">
        Every engine is off until you switch it on. With none on, Rewrite still
        returns your polished transcript — refinement never costs you your
        words.
      </p>
    </div>
  );
}
