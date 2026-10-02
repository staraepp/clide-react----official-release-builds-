import { ArrowUpRight } from "lucide-react";
import { Card, CardHeader } from "@/components/Card";
import { Button } from "@/components/Button";
import { CopyButton } from "@/components/CopyButton";
import { useHistory } from "@/history/useHistory";
import { clockTime, preview } from "@/lib/format";

/**
 * The last few transcripts, laid out the way the marketing site's session log
 * is: time, where it landed, what was said. Real rows only — no invented
 * statistics.
 */
export function RecentCard({ onOpenHistory }: { onOpenHistory: () => void }) {
  const { transcripts, loading } = useHistory({ limit: 4 });

  return (
    <Card index={3} className="flex flex-col py-6">
      <CardHeader
        label="Recent"
        action={
          <Button size="sm" variant="ghost" onClick={onOpenHistory}>
            All transcripts
            <ArrowUpRight size={12} />
          </Button>
        }
      />

      {transcripts.length === 0 ? (
        <div className="flex flex-1 items-center py-6">
          <p className="text-[13px] text-ink-3">
            {loading
              ? "Loading…"
              : "Nothing yet. Your first dictation will appear here."}
          </p>
        </div>
      ) : (
        <ul className="mt-2 flex flex-col">
          {transcripts.map((transcript) => (
            <li
              key={transcript.id}
              className="group grid grid-cols-[46px_78px_1fr_auto] items-baseline gap-3 border-t border-line py-2.5 first:border-t-0"
            >
              <time className="numeral text-[11px] text-ink-3">
                {clockTime(transcript.createdAt)}
              </time>
              <span className="numeral truncate text-[11px] text-ink-3">
                {transcript.sourceApp ?? "—"}
              </span>
              <span className="truncate text-[13.5px] text-ink">
                {preview(transcript.text, 96)}
              </span>
              <CopyButton
                text={transcript.text}
                className="opacity-0 transition-opacity group-hover:opacity-100 focus-visible:opacity-100"
              />
            </li>
          ))}
        </ul>
      )}
    </Card>
  );
}
