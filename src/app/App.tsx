import { useEffect, useState } from "react";
import { AnimatePresence, motion } from "motion/react";

import { Dashboard } from "@/dashboard/Dashboard";
import { HistoryView } from "@/history/HistoryView";
import { ModelsView } from "@/models/ModelsView";
import { SettingsView } from "@/settings/SettingsView";
import { Onboarding } from "@/onboarding/Onboarding";
import { ShaderBackground } from "@/shaders/ShaderBackground";
import { useSystemStatus } from "./useSystemStatus";
import { useDictationState } from "@/dictation/useDictationState";
import { useMicLevel } from "@/dictation/useMicLevel";
import { EVENTS, on } from "@/lib/events";
import { useEasterEggs } from "./useEasterEggs";
import { isBusy } from "@/lib/types";
import { Sidebar, type View } from "./Sidebar";
import * as commands from "@/lib/commands";

export function App() {
  const { status, refresh } = useSystemStatus();
  const { surge } = useEasterEggs();
  const [view, setView] = useState<View>("dashboard");
  const state = useDictationState();
  const level = useMicLevel();

  // Native persistence makes this a real once-per-day check across launches,
  // rather than one request every time React remounts.
  useEffect(() => {
    commands
      .checkForUpdates(false)
      .catch((error) => console.debug("background update check skipped", error));
  }, []);

  // The tray's "Settings…" item navigates the already-open window.
  useEffect(() => {
    const subscription = on(EVENTS.navigate, (route) => {
      if (route === "settings" || route === "history" || route === "dashboard") {
        setView(route);
      }
    });
    return () => {
      subscription.then((unlisten) => unlisten());
    };
  }, []);

  if (!status) {
    return <div className="h-full w-full bg-paper" />;
  }

  if (!status.settings.onboardingComplete) {
    return (
      <Onboarding
        status={status}
        refresh={refresh}
        onDone={() => {
          refresh();
          setView("dashboard");
        }}
      />
    );
  }

  return (
    <div className="relative h-full w-full overflow-hidden">
      <ShaderBackground
        intensity={surge ? "high" : status.settings.visualIntensity}
        // The wash gathers only while clide is handling speech — the same
        // "blue means voice" rule the rest of the palette follows.
        active={isBusy(state) || surge}
        // Only High reacts to the microphone; the shader ignores it otherwise.
        energy={state.kind === "capturing" ? (level.current ?? 0) : 0}
      />

      <div className="relative flex h-full">
        <Sidebar
          view={view}
          onChange={setView}
          status={status}
          state={state}
          levelRef={level}
        />

        <main className="relative flex min-w-0 flex-1 flex-col border-l border-line bg-card">
          <div data-tauri-drag-region className="drag-region h-[38px] shrink-0" />

          <div className="scroll-area min-h-0 flex-1">
            <AnimatePresence mode="wait" initial={false}>
              <motion.div
                key={view}
                initial={{ opacity: 0, y: 6 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0 }}
                transition={{ duration: 0.18, ease: [0.22, 1, 0.36, 1] }}
                className="mx-auto w-full max-w-[820px] px-9 pb-14"
              >
                {view === "dashboard" && (
                  <Dashboard
                    status={status}
                    refresh={refresh}
                    onNavigate={setView}
                  />
                )}
                {view === "models" && <ModelsView />}
                {view === "history" && <HistoryView />}
                {view === "settings" && (
                  <SettingsView
                    status={status}
                    refresh={refresh}
                    onOpenModels={() => setView("models")}
                  />
                )}
              </motion.div>
            </AnimatePresence>
          </div>
        </main>
      </div>
    </div>
  );
}
