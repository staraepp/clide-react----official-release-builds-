import { useMemo, useState } from "react";
import { Copy, Search, Trash2 } from "lucide-react";
import { motion } from "motion/react";

import { Button } from "@/components/Button";
import { useHistory } from "./useHistory";
import * as commands from "@/lib/commands";
import { relativeTime, wordCount } from "@/lib/format";
import type { HistoryQuery } from "@/lib/types";
import { cn } from "@/lib/cn";

/**
 * Transcript history.
 *
 * Search is served by SQLite FTS in Rust, so it stays fast as history grows
 * and matches on prefixes while the user is still typing.
 */
export function HistoryView() {
  const [search, setSearch] = useState("");
  const [sourceApp, setSourceApp] = useState<string | null>(null);

  const query = useMemo<HistoryQuery>(
    () => ({
      search: search.trim() || undefined,
      sourceApp: sourceApp ?? undefined,
      limit: 200,
    }),
    [search, sourceApp],
  );

  const { transcripts, loading } = useHistory(query);

  const apps = useMemo(() => {
    const seen = new Set<string>();
    for (const transcript of transcripts) {
      if (transcript.sourceApp) seen.add(transcript.sourceApp);
    }
    return [...seen].slice(0, 8);
  }, [transcripts]);

  return (
    <div className="flex h-full flex-col gap-4 pb-8">
      <div className="flex flex-wrap items-center gap-2">
        <div className="relative flex-1 min-w-[220px]">
          <Search
            size={14}
            className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-ink-2"
          />
          <input
            value={search}
            onChange={(event) => setSearch(event.target.value)}
            placeholder="Search transcripts"
            className={cn(
              "no-drag h-10 w-full rounded-xl border border-line bg-sunken pl-9 pr-3",
              "text-[13px] text-ink placeholder:text-ink-3",
              "outline-none transition-colors focus:border-voice",
            )}
          />
        </div>

        {sourceApp && (
          <Button size="sm" onClick={() => setSourceApp(null)}>
            {sourceApp} ✕
          </Button>
        )}
      </div>

      {apps.length > 1 && !sourceApp && (
        <div className="flex flex-wrap gap-1.5">
          {apps.map((app) => (
            <Button key={app} size="sm" variant="ghost" onClick={() => setSourceApp(app)}>
              {app}
            </Button>
          ))}
        </div>
      )}

      <div className="scroll-area -mr-2 flex-1 pr-2">
        {transcripts.length === 0 ? (
          <p className="mt-16 text-center text-[13px] text-ink-2">
            {loading
              ? "Loading…"
              : search
                ? `Nothing matches “${search}”.`
                : "No transcripts yet."}
          </p>
        ) : (
          <ul className="flex flex-col divide-y divide-line">
            {transcripts.map((transcript, index) => (
              <motion.li
                key={transcript.id}
                initial={{ opacity: 0, y: 8 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ delay: Math.min(index, 10) * 0.018 }}
                className="group py-4"
              >
                <p className="text-[13.5px] leading-relaxed text-ink">
                  {transcript.text}
                </p>

                <div className="mt-3 flex items-center gap-3">
                  <span className="numeral text-[11px] text-ink-2">
                    {relativeTime(transcript.createdAt)}
                  </span>
                  {transcript.sourceApp && (
                    <span className="text-[11px] text-ink-2">
                      {transcript.sourceApp}
                    </span>
                  )}
                  <span className="numeral text-[11px] text-ink-3">
                    {wordCount(transcript.text)} words
                  </span>

                  <div className="ml-auto flex gap-1 opacity-0 transition-opacity group-hover:opacity-100 focus-within:opacity-100">
                    <Button
                      size="sm"
                      variant="ghost"
                      aria-label="Copy"
                      onClick={() => commands.copyText(transcript.text)}
                    >
                      <Copy size={13} />
                    </Button>
                    <Button
                      size="sm"
                      variant="ghost"
                      aria-label="Delete"
                      onClick={() => commands.deleteTranscript(transcript.id)}
                    >
                      <Trash2 size={13} />
                    </Button>
                  </div>
                </div>
              </motion.li>
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}
