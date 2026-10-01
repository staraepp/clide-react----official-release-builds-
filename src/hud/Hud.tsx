import { useEffect, useState } from "react";
import { AnimatePresence, motion } from "motion/react";
import { Check, Copy, RotateCw, X } from "lucide-react";

import { Waveform } from "@/components/Waveform";
import { ShaderBackground } from "@/shaders/ShaderBackground";
import { useDictationState } from "@/dictation/useDictationState";
import { useMicLevel } from "@/dictation/useMicLevel";
import { useSystemStatus } from "@/app/useSystemStatus";
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
 * A chip, not a window: no title bar, no settings, no engine menu. It shows one
 * line of state and, when something has gone wrong, the two or three controls
 * needed to recover. The window never takes focus, so the caret stays where the
 * user left it.
 */
export function Hud() {
  const state = useDictationState();
  const level = useMicLevel();
  const fellBack = useFallbackNotice(state);
  const corrected = useCorrectionNotice(state);
  // The HUD never takes focus, so this only refetches when settings change —
  // which is the one thing that can alter how the chip should render.
  const { status } = useSystemStatus();

  const failure = failureDetail(state);
  const transcript = transcriptOf(state);
  const expanded = failure !== null;

  return (
    <div className="flex h-full w-full items-end justify-center pb-1">
      <AnimatePresence>
        {state.kind !== "idle" && (
          <motion.div
            key="hud"
            initial={{ opacity: 0, y: 26, scale: 0.72 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={{
              opacity: 0,
              y: 14,
              scale: 0.78,
              // Leaving is quicker than arriving, so the HUD gets out of the
              // way rather than lingering over the text it just inserted.
              transition: { duration: 0.16, ease: [0.4, 0, 1, 1] },
            }}
            transition={{
              // Underdamped, so it overshoots and settles: the HUD appears
              // over whatever the user is typing in and should announce itself.
              type: "spring",
              stiffness: 560,
              damping: 22,
              mass: 0.62,
            }}
            className={cn(
              "pointer-events-auto relative flex flex-col overflow-hidden rounded-[13px]",
              "border border-line-2 bg-card/92 backdrop-blur-xl",
              "shadow-[0_6px_22px_-8px_rgba(10,35,56,0.28)]",
              expanded ? "w-[300px]" : "w-auto",
            )}
          >
            {/* The same field as the dashboard, at chip scale. It is the only
                thing the user sees while dictating into another app, so the
                voice blue lives here rather than in a static fill. Suppressed
                on failure, where blue would read as "still working". */}
            {!failure && (
              <ShaderBackground
                intensity={status?.settings.visualIntensity ?? "normal"}
                active
                energy={state.kind === "capturing" ? (level.current ?? 0) : 0}
                className="opacity-70"
              />
            )}

            <div className="relative flex items-center gap-2.5 px-3.5 py-2.5">
              <Visual state={state} levelRef={level} />
              <span className="whitespace-nowrap text-[12.5px] text-ink">
                {stateLabel(state)}
              </span>

              {fellBack && (
                <span className="whitespace-nowrap text-[11px] text-warn">
                  via {fellBack.usedProvider}
                </span>
              )}

              {corrected > 0 && (
                <span className="whitespace-nowrap text-[11px] text-ink-3">
                  {corrected === 1 ? "corrected" : `${corrected} corrections`}
                </span>
              )}
            </div>

            {failure && (
              <div className="relative border-t border-line px-3.5 py-2.5">
                <p className="text-[11.5px] leading-relaxed text-ink-2">
                  {failure}
                </p>
                <div className="mt-2.5 flex items-center gap-1.5">
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
      <div className="h-4 w-[58px]">
        <Waveform levelRef={levelRef} bars={16} color="#5b9bc9" />
      </div>
    );
  }

  if (state.kind === "complete") {
    return (
      <motion.span
        initial={{ scale: 0.4, opacity: 0 }}
        animate={{ scale: 1, opacity: 1 }}
        transition={{ type: "spring", stiffness: 600, damping: 20 }}
        className="flex size-4 items-center justify-center rounded-full bg-ok/15 text-ok"
      >
        <Check size={10} strokeWidth={3} />
      </motion.span>
    );
  }

  if (failureDetail(state)) {
    return <span className="size-1.5 shrink-0 rounded-full bg-stop" />;
  }

  // Transcribing / inserting: the waveform settles into a travelling shimmer.
  return (
    <div className="flex h-4 w-[58px] items-center gap-[3px]">
      {Array.from({ length: 10 }).map((_, index) => (
        <motion.span
          key={index}
          className="h-full w-[2.5px] rounded-full bg-voice"
          animate={{ scaleY: [0.24, 0.92, 0.24], opacity: [0.35, 1, 0.35] }}
          transition={{
            duration: 1.1,
            repeat: Infinity,
            ease: "easeInOut",
            delay: index * 0.075,
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
          ? "bg-ink text-white hover:bg-[#12314a]"
          : "text-ink-3 hover:bg-sunken hover:text-ink",
      )}
    >
      {icon}
      {label}
    </button>
  );
}
