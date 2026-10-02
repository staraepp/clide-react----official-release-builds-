import { useEffect, useState } from "react";
import { AnimatePresence, motion } from "motion/react";
import { Check, Copy, GripVertical, RotateCw, X } from "lucide-react";

import { Waveform } from "@/components/Waveform";
import { useDictationState } from "@/dictation/useDictationState";
import { useMicLevel } from "@/dictation/useMicLevel";
import { failureDetail, stateLabel } from "@/dictation/labels";
import * as commands from "@/lib/commands";
import { EVENTS, on } from "@/lib/events";
import {
  transcriptOf,
  type DictationState,
  type FallbackPayload,
} from "@/lib/types";
import { cn } from "@/lib/cn";

/**
 * The recording HUD.
 *
 * A pill, not a window: no title bar, no settings, no engine menu. It shows one
 * line of state and, when something has gone wrong, a card holding the
 * transcript so it can be dragged straight into a text field. The window never
 * takes focus, so the caret stays where the user left it.
 */
export function Hud() {
  const state = useDictationState();
  const level = useMicLevel();
  const fellBack = useFallbackNotice(state);
  const corrected = useCorrectionNotice(state);

  const failure = failureDetail(state);
  const transcript = transcriptOf(state);
  const expanded = failure !== null;

  return (
    <div className="flex h-full w-full items-end justify-center">
      <AnimatePresence>
        {state.kind !== "idle" && (
          <motion.div
            key="hud"
            initial={{ opacity: 0, y: 18, scale: 0.8 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            // Leaving swells a touch, then draws in to nothing: opacity only
            // drops at the very end, so it never reads as a blink.
            exit={{
              opacity: [1, 1, 0],
              scale: [1, 1.07, 0.5],
              y: [0, 0, 6],
              transition: { duration: 0.42, times: [0, 0.35, 1], ease: "easeInOut" },
            }}
            transition={{ type: "spring", stiffness: 520, damping: 26, mass: 0.6 }}
            className={cn(
              "pointer-events-auto relative flex flex-col overflow-hidden",
              "bg-[#0c0c0d] text-white shadow-[0_8px_28px_-10px_rgba(0,0,0,0.55)]",
              "ring-1 ring-white/10",
              expanded ? "w-[360px] rounded-[22px]" : "w-auto rounded-full",
            )}
          >
            <div
              className={cn(
                "relative flex items-center gap-2.5",
                expanded ? "px-4 pt-3.5" : "px-4 py-2",
              )}
            >
              <Visual state={state} levelRef={level} />
              <span className="whitespace-nowrap text-[12.5px] text-white/90">
                {stateLabel(state)}
              </span>

              {fellBack && (
                <span className="whitespace-nowrap text-[11px] text-white/50">
                  via {fellBack.usedProvider}
                </span>
              )}

              {corrected > 0 && (
                <span className="whitespace-nowrap text-[11px] text-white/50">
                  {corrected === 1 ? "corrected" : `${corrected} corrections`}
                </span>
              )}
            </div>

            {failure && (
              <div className="relative flex flex-col gap-2.5 px-4 pb-3.5 pt-2">
                {transcript ? (
                  <>
                    <p className="text-[11.5px] leading-relaxed text-white/60">
                      Drag this into any text field, or copy it.
                    </p>
                    <TranscriptChip text={transcript} />
                  </>
                ) : (
                  <p className="text-[11.5px] leading-relaxed text-white/60">
                    {failure}
                  </p>
                )}
                <div className="flex items-center gap-1.5">
                  {state.kind === "transcriptionFailed" && state.retryable && (
                    <HudAction
                      icon={<RotateCw size={11} />}
                      label="Retry"
                      primary
                      onClick={() => commands.retryDictation()}
                    />
                  )}
                  {transcript && (
                    <HudAction
                      icon={<Copy size={11} />}
                      label="Copy"
                      primary
                      onClick={() => commands.copyText(transcript)}
                    />
                  )}
                  <HudAction
                    icon={<X size={11} />}
                    label="Dismiss"
                    onClick={() => commands.dismissDictation()}
                  />
                </div>
              </div>
            )}
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}

/**
 * The transcript as a chip that can be dragged out of the HUD.
 *
 * Dropped on any text field it types the words in, with no Accessibility
 * permission needed — the drop is the user's own gesture, so macOS lets it
 * through. Once it lands, the HUD closes.
 */
function TranscriptChip({ text }: { text: string }) {
  return (
    <div
      draggable
      onDragStart={(event) => {
        event.dataTransfer.setData("text/plain", text);
        event.dataTransfer.effectAllowed = "copy";
      }}
      onDragEnd={(event) => {
        if (event.dataTransfer.dropEffect !== "none") {
          commands.dismissDictation();
        }
      }}
      className="flex cursor-grab items-start gap-2 rounded-xl bg-white/10 px-3 py-2.5 active:cursor-grabbing"
    >
      <GripVertical size={14} className="mt-0.5 shrink-0 text-white/40" />
      <p className="line-clamp-4 select-none text-[12.5px] leading-snug text-white">
        {text}
      </p>
    </div>
  );
}

/**
 * Remember which engine rescued the current dictation.
 *
 * Cleared when the next one starts, so the notice belongs to the transcript it
 * describes rather than lingering.
 */
function useFallbackNotice(state: DictationState) {
  const [notice, setNotice] = useState<FallbackPayload | null>(null);

  useEffect(() => {
    const subscription = on(EVENTS.transcriptionFellBack, setNotice);
    return () => {
      subscription.then((unsubscribe) => unsubscribe());
    };
  }, []);

  useEffect(() => {
    if (state.kind === "capturing") setNotice(null);
  }, [state.kind]);

  return notice;
}

/**
 * How many spoken corrections ("scratch that") shaped the current dictation.
 *
 * A correction removes words the user said, so it is announced rather than
 * silent. Cleared when the next dictation starts.
 */
function useCorrectionNotice(state: DictationState) {
  const [count, setCount] = useState(0);

  useEffect(() => {
    const subscription = on(EVENTS.correctionApplied, (payload) =>
      setCount(payload.count),
    );
    return () => {
      subscription.then((unsubscribe) => unsubscribe());
    };
  }, []);

  useEffect(() => {
    if (state.kind === "capturing") setCount(0);
  }, [state.kind]);

  return count;
}

/** The left-hand glyph: waveform, activity shimmer, tick, or warning. */
function Visual({
  state,
  levelRef,
}: {
  state: DictationState;
  levelRef: React.RefObject<number>;
}) {
  if (state.kind === "capturing") {
    return (
      <div className="h-5 w-[52px]">
        <Waveform levelRef={levelRef} bars={7} color="#ffffff" />
      </div>
    );
  }

  if (state.kind === "complete") {
    return (
      <motion.span
        initial={{ scale: 0.4, opacity: 0 }}
        animate={{ scale: 1, opacity: 1 }}
        transition={{ type: "spring", stiffness: 600, damping: 20 }}
        className="flex size-4 items-center justify-center rounded-full bg-white text-black"
      >
        <Check size={10} strokeWidth={3} />
      </motion.span>
    );
  }

  if (failureDetail(state)) {
    return <span className="size-2 shrink-0 rounded-full bg-white/70" />;
  }

  // Transcribing / inserting: the waveform settles into a travelling pulse.
  return (
    <div className="flex h-5 w-[52px] items-center justify-between">
      {Array.from({ length: 7 }).map((_, index) => (
        <motion.span
          key={index}
          className="size-[5px] rounded-full bg-white"
          animate={{ scale: [0.7, 1.35, 0.7], opacity: [0.35, 1, 0.35] }}
          transition={{
            duration: 1,
            repeat: Infinity,
            ease: "easeInOut",
            delay: index * 0.08,
          }}
        />
      ))}
    </div>
  );
}

function HudAction({
  icon,
  label,
  onClick,
  primary,
}: {
  icon: React.ReactNode;
  label: string;
  onClick: () => void;
  primary?: boolean;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        "inline-flex h-6 items-center gap-1.5 rounded-md px-2 text-[11px] transition-colors",
        primary
          ? "bg-white text-black hover:bg-white/85"
          : "text-white/60 hover:bg-white/10 hover:text-white",
      )}
    >
      {icon}
      {label}
    </button>
  );
}
