import { motion } from "motion/react";
import { Check, Cpu, Download, Trash2 } from "lucide-react";

import { Button } from "@/components/Button";
import { Stars } from "./Stars";
import { FitBadge } from "./FitBadge";
import type { DownloadProgress, ModelStatus } from "@/lib/types";
import { cn } from "@/lib/cn";

/**
 * One model in the feed.
 *
 * Leads with the verdict for *this* Mac, because that is the decision being
 * made. Accuracy and speed sit underneath as the reasons behind it.
 */
export function ModelCard({
  model,
  progress,
  selected,
  onDownload,
  onRemove,
  onSelect,
  index,
}: {
  model: ModelStatus;
  progress?: DownloadProgress;
  selected: boolean;
  onDownload: () => void;
  onRemove: () => void;
  onSelect: () => void;
  index: number;
}) {
  const downloading = progress !== undefined && !model.installed;
  const unusable = model.rating.fit === "tooLarge";

  return (
    <motion.article
      initial={{ opacity: 0, y: 8 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.36, delay: Math.min(index, 8) * 0.03 }}
      className={cn(
        "card flex flex-col gap-3 p-4 transition-colors",
        selected && "border-voice bg-voice-tint/40",
      )}
    >
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <h3 className="display truncate text-[15px]">{model.name}</h3>
            {model.installed && (
              <span className="flex size-4 shrink-0 items-center justify-center rounded-full bg-ok/15 text-ok">
                <Check size={9} strokeWidth={3} />
              </span>
            )}
          </div>
          <p className="mt-1 text-[12.5px] leading-relaxed text-ink-2">
            {model.description}
          </p>
        </div>
        <FitBadge fit={model.rating.fit} className="shrink-0" />
      </div>

      <div className="flex flex-wrap items-center gap-x-5 gap-y-1.5">
        <Stars value={model.rating.accuracy} label="Accuracy" />
        <span className="text-[11px] text-ink-3">Accuracy</span>
        <Stars value={model.rating.speed} label="Speed on this Mac" />
        <span className="text-[11px] text-ink-3">Speed here</span>
      </div>

      <div className="mt-auto flex items-center gap-2 border-t border-line pt-3">
        <span className="numeral text-[11px] text-ink-3">{model.sizeLabel}</span>
        <span className="text-[11px] text-ink-3">·</span>
        <span className="inline-flex items-center gap-1 text-[11px] text-ink-3">
          <Cpu size={10} />
          {model.engine === "whisper" ? "whisper.cpp" : "ONNX"}
        </span>
        <span className="text-[11px] text-ink-3">·</span>
        <span className="text-[11px] text-ink-3">
          {model.multilingual ? "Multilingual" : "English only"}
        </span>

        <div className="ml-auto flex items-center gap-2">
          {downloading ? (
            <div className="flex items-center gap-2">
              <div className="h-1 w-24 overflow-hidden rounded-full bg-line">
                <motion.div
                  className="h-full rounded-full bg-voice"
                  animate={{ width: `${(progress?.fraction ?? 0) * 100}%` }}
                  transition={{ duration: 0.3 }}
                />
              </div>
              <span className="numeral text-[11px] text-ink-3">
                {Math.round((progress?.fraction ?? 0) * 100)}%
              </span>
            </div>
          ) : model.installed ? (
            <>
              <Button
                size="sm"
                variant={selected ? "surface" : "primary"}
                disabled={selected}
                onClick={onSelect}
              >
                {selected ? "In use" : "Use"}
              </Button>
              <Button
                size="sm"
                variant="ghost"
                onClick={onRemove}
                aria-label={`Remove ${model.name}`}
              >
                <Trash2 size={12} />
              </Button>
            </>
          ) : (
            <Button
              size="sm"
              icon={<Download size={12} />}
              onClick={onDownload}
              // A model that cannot fit in memory is still downloadable —
              // the user may be about to upgrade, or know better than the
              // estimate. The badge warns; it does not forbid.
              variant={unusable ? "surface" : "primary"}
            >
              Download
            </Button>
          )}
        </div>
      </div>
    </motion.article>
  );
}

/** A model that ships with macOS — no download, no hardware verdict. */
export function BuiltInModelRow({
  name,
  description,
  selected,
  onSelect,
}: {
  name: string;
  description: string;
  selected: boolean;
  onSelect: () => void;
}) {
  return (
    <motion.div
      initial={{ opacity: 0, y: 8 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.32 }}
      className={cn(
        "card flex flex-col gap-3 p-4 transition-colors",
        selected && "border-voice bg-voice-tint/40",
      )}
    >
      <div className="flex items-start gap-2">
        <Cpu size={13} className="mt-0.5 shrink-0 text-ink-3" />
        <div className="min-w-0">
          <h3 className="display truncate text-[15px]">{name}</h3>
          <p className="mt-1 text-[12.5px] leading-relaxed text-ink-2">
            {description}
          </p>
        </div>
      </div>

      <div className="mt-auto flex items-center gap-2 border-t border-line pt-3">
        <span className="text-[11px] text-ink-3">Built into macOS</span>
        <Button
          size="sm"
          variant={selected ? "surface" : "primary"}
          disabled={selected}
          onClick={onSelect}
          className="ml-auto"
        >
          {selected ? "In use" : "Use"}
        </Button>
      </div>
    </motion.div>
  );
}
