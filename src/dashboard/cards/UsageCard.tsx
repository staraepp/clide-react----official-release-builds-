import { useEffect, useState } from "react";
import { motion } from "motion/react";

import { Card, CardHeader } from "@/components/Card";
import { useCountUp } from "@/lib/useCountUp";
import * as commands from "@/lib/commands";
import { EVENTS, on } from "@/lib/events";
import type { Usage } from "@/lib/types";

/**
 * What you've actually dictated.
 *
 * Every figure is a count over rows in the database — no estimates, no invented
 * metrics. When there is nothing yet, the card says so rather than showing a
 * row of confident zeroes.
 */
export function UsageCard() {
  const [usage, setUsage] = useState<Usage | null>(null);

  useEffect(() => {
    const refresh = () => {
      commands
        .getUsage()
        .then(setUsage)
        .catch((error) => console.error("could not read usage", error));
    };

    refresh();
    const subscription = on(EVENTS.historyChanged, refresh);
    return () => {
      subscription.then((unsubscribe) => unsubscribe());
    };
  }, []);

  if (!usage) {
    // Hold the slot while loading so the grid does not reflow under the user.
    return (
      <Card index={5} className="py-6">
        <CardHeader label="This week" />
      </Card>
    );
  }

  if (usage.totalTranscripts === 0) {
    return (
      <Card index={5} className="flex flex-col py-6">
        <CardHeader label="This week" />
        <p className="flex flex-1 items-center py-4 text-[13px] text-ink-3">
          Nothing dictated yet. Your first transcript starts the count.
        </p>
      </Card>
    );
  }

  return (
    <Card index={5} className="flex flex-col py-6">
      <CardHeader label="This week" />

      <div className="mt-3 grid grid-cols-2 gap-x-6 gap-y-4 sm:grid-cols-4 lg:grid-cols-4">
        <Figure value={usage.wordsThisWeek} caption="words spoken" />
        <Figure value={usage.transcriptsThisWeek} caption="dictations" />
        <Figure value={usage.appsThisWeek} caption="apps used" />
        <Figure
          value={usage.dayStreak}
          caption={usage.dayStreak === 1 ? "day in a row" : "days in a row"}
          highlight={usage.dayStreak >= 2}
        />
      </div>
    </Card>
  );
}

function Figure({
  value,
  caption,
  highlight,
}: {
  value: number;
  caption: string;
  highlight?: boolean;
}) {
  const shown = useCountUp(value);

  return (
    <div className="flex flex-col gap-0.5">
      <motion.span
        initial={{ opacity: 0, y: 4 }}
        animate={{ opacity: 1, y: 0 }}
        className="display text-[26px] leading-none tabular-nums"
      >
        {shown.toLocaleString()}
        {highlight && <span className="ml-1 text-voice">·</span>}
      </motion.span>
      <span className="text-[11.5px] text-ink-3">{caption}</span>
    </div>
  );
}
