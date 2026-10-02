import { Card, CardHeader } from "@/components/Card";
import { Segmented } from "@/components/Segmented";
import * as commands from "@/lib/commands";
import type { ProcessingMode } from "@/lib/types";

const DESCRIPTIONS: Record<ProcessingMode, string> = {
  verbatim: "Your words exactly as spoken. Only spacing is tidied.",
  polished: "Formats your text: punctuation, capitals, and \u201cum\u201d removed. Local, instant.",
  rewrite: "Formats it, then rephrases it as written prose using the engine you chose.",
};

/**
 * Processing style.
 *
 * All three modes are live. Rewrite runs the same deterministic cleanup first
 * and then hands the result to a refinement engine — so if no engine is
 * available, the user still gets a polished transcript rather than a failure.
 */
export function ModeCard({
  mode,
  onChange,
}: {
  mode: ProcessingMode;
  onChange: () => void;
}) {
  return (
    <Card index={2} className="flex flex-col py-6">
      <CardHeader label="Style" />

      <Segmented
        className="mt-3 w-full"
        value={mode}
        onChange={async (next) => {
          await commands.setProcessingMode(next);
          onChange();
        }}
        segments={[
          { value: "verbatim", label: "Verbatim" },
          { value: "polished", label: "Polished" },
          {
            value: "rewrite",
            label: "Rewrite",
            hint: "Refines with an on-device model",
          },
        ]}
      />

      <p className="mt-3 text-[12.5px] leading-relaxed text-ink-2">
        {DESCRIPTIONS[mode]}
      </p>
    </Card>
  );
}
