import { useCallback, useState } from "react";
import { AnimatePresence, motion } from "motion/react";
import { ArrowLeft, ArrowRight } from "lucide-react";

import { Button } from "@/components/Button";
import { ShaderBackground } from "@/shaders/ShaderBackground";
import * as commands from "@/lib/commands";
import type { SystemStatus } from "@/lib/types";
import { cn } from "@/lib/cn";

import { WelcomeStep } from "./steps/WelcomeStep";
import { MicrophoneStep } from "./steps/MicrophoneStep";
import { AccessibilityStep } from "./steps/AccessibilityStep";
import { ShortcutStep } from "./steps/ShortcutStep";
import { ProviderStep } from "./steps/ProviderStep";
import { TestDictationStep } from "./steps/TestDictationStep";
import { FinishStep } from "./steps/FinishStep";

const STEPS = [
  "welcome",
  "microphone",
  "accessibility",
  "shortcut",
  "provider",
  "test",
  "finish",
] as const;

type Step = (typeof STEPS)[number];

/**
 * First-run setup.
 *
 * Each permission is requested by the step that explains it, never at launch,
 * and every step verifies the real system state before letting the user move
 * on. Steps that can't be verified (Accessibility takes effect outside the
 * app) can be skipped deliberately rather than silently.
 */
export function Onboarding({
  status,
  refresh,
  onDone,
}: {
  status: SystemStatus;
  refresh: () => void;
  onDone: () => void;
}) {
  const [index, setIndex] = useState(0);
  const [tested, setTested] = useState(false);
  const step = STEPS[index];

  const onTestSucceeded = useCallback(() => setTested(true), []);

  /** Whether the step's goal is actually met, read from system state. */
  const satisfied: Record<Step, boolean> = {
    welcome: true,
    microphone: status.permissions.microphone === "granted",
    accessibility: status.permissions.accessibility === "granted",
    shortcut: status.shortcutRegistered,
    provider: status.providerReady,
    test: tested,
    finish: true,
  };

  const isLast = index === STEPS.length - 1;

  const next = async () => {
    if (isLast) {
      await commands.completeOnboarding();
      onDone();
      return;
    }
    setIndex((current) => current + 1);
    refresh();
  };

  return (
    <div className="relative h-full w-full overflow-hidden">
      <ShaderBackground intensity={status.settings.visualIntensity} />

      <div className="drag-region relative flex h-full items-center justify-center p-8">
        <motion.div
          layout
          transition={{ type: "spring", stiffness: 320, damping: 34 }}
          className="card no-drag w-full max-w-[540px] p-7"
        >
          <div className="flex justify-center gap-1.5 pb-6">
            {STEPS.map((name, position) => (
              <span
                key={name}
                className={cn(
                  "h-[3px] rounded-full transition-all duration-300",
                  position === index
                    ? "w-6 bg-voice"
                    : position < index
                      ? "w-2.5 bg-voice"
                      : "w-2.5 bg-line",
                )}
              />
            ))}
          </div>

          <AnimatePresence mode="wait" initial={false}>
            <motion.div
              key={step}
              initial={{ opacity: 0, x: 18 }}
              animate={{ opacity: 1, x: 0 }}
              exit={{ opacity: 0, x: -18 }}
              transition={{ duration: 0.26, ease: [0.22, 1, 0.36, 1] }}
            >
              {step === "welcome" && <WelcomeStep />}
              {step === "microphone" && (
                <MicrophoneStep
                  status={status.permissions.microphone}
                  refresh={refresh}
                />
              )}
              {step === "accessibility" && (
                <AccessibilityStep
                  status={status.permissions.accessibility}
                  adHocBuild={status.adHocBuild}
                  refresh={refresh}
                />
              )}
              {step === "shortcut" && (
                <ShortcutStep status={status} refresh={refresh} />
              )}
              {step === "provider" && <ProviderStep status={status} refresh={refresh} />}
              {step === "test" && (
                <TestDictationStep status={status} onSuccess={onTestSucceeded} />
              )}
              {step === "finish" && <FinishStep status={status} />}
            </motion.div>
          </AnimatePresence>

          <footer className="mt-7 flex items-center gap-2">
            {index > 0 && !isLast && (
              <Button
                variant="ghost"
                icon={<ArrowLeft size={14} />}
                onClick={() => setIndex((current) => current - 1)}
              >
                Back
              </Button>
            )}

            <div className="ml-auto flex items-center gap-2">
              {/* Steps that depend on something outside clide can be skipped;
                  the dashboard keeps showing what is still missing. */}
              {!satisfied[step] && !isLast && (
                <Button variant="ghost" onClick={next}>
                  Skip for now
                </Button>
              )}
              <Button
                variant="primary"
                disabled={!satisfied[step] && step !== "welcome"}
                icon={isLast ? undefined : <ArrowRight size={14} />}
                onClick={next}
              >
                {isLast ? "Open Clide" : step === "welcome" ? "Get started" : "Continue"}
              </Button>
            </div>
          </footer>
        </motion.div>
      </div>
    </div>
  );
}
