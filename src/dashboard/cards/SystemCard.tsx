import { useState } from "react";
import { Card, CardHeader } from "@/components/Card";
import { StatusDot, type Tone } from "@/components/StatusDot";
import { Button } from "@/components/Button";
import { Keys } from "@/components/Keys";
import * as commands from "@/lib/commands";
import { cn } from "@/lib/cn";
import type { PermissionStatus, SystemStatus } from "@/lib/types";

function permissionTone(status: PermissionStatus): Tone {
  if (status === "granted") return "ready";
  if (status === "denied" || status === "restricted") return "problem";
  return "pending";
}

function permissionLabel(status: PermissionStatus): string {
  switch (status) {
    case "granted":
      return "Granted";
    case "denied":
      return "Denied";
    case "restricted":
      return "Blocked by policy";
    case "notDetermined":
      return "Not granted";
  }
}

/**
 * Readiness at a glance. Every row is a real system fact read from macOS, not a
 * value clide is remembering from the last time it asked.
 */
export function SystemCard({
  status,
  onRefresh,
}: {
  status: SystemStatus;
  onRefresh: () => void;
}) {
  const [repairError, setRepairError] = useState<string | null>(null);
  const { microphone, accessibility, speechRecognition } = status.permissions;
  // Only Apple Speech needs it, so it only appears when Apple Speech is chosen.
  const usingAppleSpeech = status.settings.providerId === "apple";

  return (
    <Card index={4} className="flex flex-col py-6">
      <CardHeader
        label="Setup"
        action={
          <Button size="sm" variant="ghost" onClick={onRefresh}>
            Re-check
          </Button>
        }
      />

      <ul className="mt-3 grid grid-cols-2 gap-2">
        <Row
          tone={permissionTone(microphone)}
          name="Microphone"
          value={permissionLabel(microphone)}
          action={
            microphone !== "granted" && (
              <Button
                size="sm"
                onClick={async () => {
                  await commands.requestMicrophonePermission();
                  onRefresh();
                }}
              >
                Grant
              </Button>
            )
          }
        />

        <Row
          tone={permissionTone(accessibility)}
          name="Accessibility"
          value={
            accessibility === "granted"
              ? "Granted"
              : status.adHocBuild
                ? "Lost on rebuild"
                : "Not granted"
          }
          action={
            accessibility !== "granted" && (
              <Button
                size="sm"
                onClick={async () => {
                  setRepairError(null);
                  try {
                    if (status.adHocBuild) {
                      await commands.requestAccessibilityPermission();
                      await commands.openAccessibilitySettings();
                    } else {
                      await commands.repairAccessibilityPermission();
                    }
                    onRefresh();
                  } catch (error) {
                    setRepairError(commands.errorMessage(error));
                  }
                }}
              >
                {status.adHocBuild ? "Open Settings" : "Repair access"}
              </Button>
            )
          }
        />

        {usingAppleSpeech && (
          <Row
            tone={permissionTone(speechRecognition)}
            name="Speech"
            value={permissionLabel(speechRecognition)}
            action={
              speechRecognition !== "granted" && (
                <Button
                  size="sm"
                  onClick={async () => {
                    await commands.requestSpeechPermission();
                    onRefresh();
                  }}
                >
                  Grant
                </Button>
              )
            }
          />
        )}

        <Row
          tone={status.shortcutRegistered ? "ready" : "problem"}
          name="Shortcut"
          value={
            status.shortcutRegistered ? (
              <Keys accelerator={status.settings.shortcut} />
            ) : (
              "In use by another app"
            )
          }
          wide={!usingAppleSpeech}
        />
      </ul>

      {/* The one case where System Settings and clide disagree, and both are
          right: macOS keys the grant to the code signature, and an ad-hoc
          build gets a new one every rebuild. Saying so beats prompting again. */}
      {status.adHocBuild && accessibility !== "granted" && (
        <p className="mt-3 rounded-ctl border border-warn/25 bg-warn/8 px-3 py-2.5 text-[11.5px] leading-relaxed text-ink-2">
          System Settings may still show clide switched on. This is a
          development build, so macOS sees each rebuild as a different app and
          drops the grant. Remove clide from Accessibility, then add it back.
        </p>
      )}

      {!status.adHocBuild && accessibility !== "granted" && (
        <p className="mt-3 rounded-ctl border border-warn/25 bg-warn/8 px-3 py-2.5 text-[11.5px] leading-relaxed text-ink-2">
          If System Settings already shows clide switched on, that entry
          belongs to an older build. Repair access clears only clide's stale
          record and asks macOS to register this signed app again.
        </p>
      )}

      {repairError && (
        <p className="mt-2 text-[11.5px] leading-relaxed text-stop">
          {repairError}
        </p>
      )}

      <div className="mt-auto flex items-center gap-2 pt-4 text-[12.5px] text-ink-2">
        <StatusDot tone={status.ready ? "ready" : "pending"} />
        {status.ready ? "clide is ready to dictate." : "Setup isn't finished yet."}
      </div>
    </Card>
  );
}

function Row({
  tone,
  name,
  value,
  action,
  wide,
}: {
  tone: Tone;
  name: string;
  value: React.ReactNode;
  action?: React.ReactNode;
  /** Span both columns, for the tile whose value is widest. */
  wide?: boolean;
}) {
  return (
    <li
      className={cn(
        "flex flex-col gap-1.5 rounded-ctl border border-line bg-sunken/60 px-3 py-2.5",
        wide && "",
      )}
    >
      <span className="flex items-center gap-2">
        <StatusDot tone={tone} />
        <span className="truncate text-[12.5px] text-ink">{name}</span>
      </span>
      <span className="truncate text-[11.5px] text-ink-3">{value}</span>
      {action}
    </li>
  );
}
