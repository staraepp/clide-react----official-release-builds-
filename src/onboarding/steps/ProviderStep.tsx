import { Cpu } from "lucide-react";
import { Button } from "@/components/Button";
import { StatusDot } from "@/components/StatusDot";
import * as commands from "@/lib/commands";
import type { SystemStatus } from "@/lib/types";
import { StepLayout } from "../StepLayout";

const BUILT_IN_ENGINE = "apple";

export function ProviderStep({
  status,
  refresh,
}: {
  status: SystemStatus;
  refresh: () => void;
}) {
  const usingBuiltIn = status.settings.providerId === BUILT_IN_ENGINE;

  return (
    <StepLayout
      icon={<Cpu size={18} />}
      title="Choose how clide listens"
      description="clide runs entirely on this Mac. Your audio is never uploaded, and there is no account or key to set up."
    >
      <div className="flex flex-col gap-3">
        <div className="flex items-center gap-3 rounded-ctl border border-line bg-sunken/60 px-3 py-3">
          <StatusDot tone={status.providerReady ? "ready" : "pending"} />
          <div className="min-w-0">
            <p className="text-[13.5px] text-ink">{status.providerName}</p>
            <p className="text-[12px] text-ink-3">
              {status.modelName}
              {status.providerReady ? "" : " · not downloaded yet"}
            </p>
          </div>
          {!usingBuiltIn && (
            <Button
              className="ml-auto"
              size="sm"
              onClick={async () => {
                await commands.selectProvider(BUILT_IN_ENGINE);
                refresh();
              }}
            >
              Use Apple Speech
            </Button>
          )}
        </div>

        <p className="text-[12px] leading-relaxed text-ink-3">
          Apple Speech is built into macOS and works right now. For higher
          accuracy you can download Whisper or Parakeet models from the Models
          page whenever you like.
        </p>
      </div>
    </StepLayout>
  );
}
