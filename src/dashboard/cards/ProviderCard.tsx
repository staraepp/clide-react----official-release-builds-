import { ArrowUpRight } from "lucide-react";
import { Card, CardHeader } from "@/components/Card";
import { StatusDot } from "@/components/StatusDot";
import { Button } from "@/components/Button";
import type { SystemStatus } from "@/lib/types";

/**
 * Which engine is doing the transcribing, and whether it can run right now.
 * Every engine runs on this Mac; "not ready" means the chosen model has not
 * been downloaded.
 */
export function ProviderCard({
  status,
  onConfigure,
}: {
  status: SystemStatus;
  onConfigure: () => void;
}) {
  return (
    <Card index={1} className="flex flex-col py-6">
      <CardHeader
        label="Engine"
        action={
          <Button size="sm" variant="ghost" onClick={onConfigure}>
            Change engine
            <ArrowUpRight size={12} />
          </Button>
        }
      />

      <p className="display mt-3 text-[19px]">{status.providerName}</p>
      <p className="text-[13px] text-ink-2">{status.modelName}</p>

      <div className="mt-auto flex items-center gap-2 pt-4 text-[12.5px] text-ink-2">
        <StatusDot tone={status.providerReady ? "ready" : "pending"} />
        {status.providerReady ? (
          "Runs on this Mac"
        ) : (
          <span className="text-warn">Model not downloaded</span>
        )}
      </div>
    </Card>
  );
}
