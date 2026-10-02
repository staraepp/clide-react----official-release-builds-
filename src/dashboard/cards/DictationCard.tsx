import { Copy, Mic, RotateCw, Square } from "lucide-react";

import { Card, CardHeader } from "@/components/Card";
import { Button } from "@/components/Button";
import { Keys } from "@/components/Keys";
import { Ribbon } from "../Ribbon";
import * as commands from "@/lib/commands";
import { shortcutKeys } from "@/lib/format";
import { isBusy, transcriptOf, type DictationState, type SystemStatus } from "@/lib/types";

/**
 * The primary tile. Answers "can I dictate right now, and what is clide doing?"
 * without the user reading anything else on screen.
 */
export function DictationCard({
  state,
  status,
  levelRef,
}: {
  state: DictationState;
  status: SystemStatus;
  levelRef: React.RefObject<number>;
}) {
  const transcript = transcriptOf(state);
  const capturing = state.kind === "capturing";
  const busy = isBusy(state);
  const micReady = status.permissions.microphone === "granted";

  return (
    <Card
      className="flex flex-col py-6"
    >
      <CardHeader
        label="Dictation"
        action={<Keys accelerator={status.settings.shortcut} />}
      />

      <Ribbon
        state={state}
        levelRef={levelRef}
        shortcut={shortcutKeys(status.settings.shortcut).join(" ")}
        behavior={status.settings.behavior}
      />

      <div className="mt-auto flex flex-wrap items-center gap-2 pt-4">
        {capturing ? (
          <Button
            variant="primary"
            icon={<Square size={13} fill="currentColor" />}
            onClick={() => commands.stopDictation()}
          >
            Stop and transcribe
          </Button>
        ) : (
          <Button
            variant="primary"
            icon={<Mic size={14} />}
            disabled={busy || !micReady}
            onClick={() => commands.startDictation()}
          >
            Start dictation
          </Button>
        )}

        {capturing && (
          <Button variant="ghost" onClick={() => commands.cancelDictation()}>
            Cancel
          </Button>
        )}

        {state.kind === "transcriptionFailed" && state.retryable && (
          <Button icon={<RotateCw size={13} />} onClick={() => commands.retryDictation()}>
            Retry
          </Button>
        )}

        {!capturing && transcript && (
          <Button icon={<Copy size={13} />} onClick={() => commands.copyText(transcript)}>
            Copy
          </Button>
        )}

        <span className="ml-auto flex items-center gap-1.5 text-[12.5px] text-ink-2">
          {status.settings.behavior === "hold" ? "Hold" : "Press"}
          <Keys accelerator={status.settings.shortcut} />
          anywhere in macOS
        </span>
      </div>
    </Card>
  );
}
