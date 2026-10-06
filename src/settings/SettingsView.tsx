import { useState } from "react";
import { Segmented } from "@/components/Segmented";
import { Toggle } from "@/components/Toggle";
import { ShortcutRecorder } from "@/components/ShortcutRecorder";
import { Button } from "@/components/Button";
import { RefineSection } from "./RefineSection";
import { AboutSection } from "./AboutSection";
import { LocalApiSection } from "./LocalApiSection";
import { DictionarySection } from "./DictionarySection";
import * as commands from "@/lib/commands";
import type { SystemStatus } from "@/lib/types";

export function SettingsView({
  status,
  refresh,
  onOpenModels,
}: {
  status: SystemStatus;
  refresh: () => void;
  onOpenModels: () => void;
}) {
  const [shortcutError, setShortcutError] = useState<string | null>(null);
  const [insertionTest, setInsertionTest] = useState<string | null>(null);
  const [permissionRepair, setPermissionRepair] = useState<string | null>(null);

  return (
    <div className="flex flex-col divide-y divide-line pt-2">
      <Section
        title="Shortcut"
        description="One shortcut, used everywhere. Hold to talk, or press once to start and again to stop."
      >
        <div className="flex flex-wrap items-start gap-6">
          <div>
            <ShortcutRecorder
              value={status.settings.shortcut}
              onChange={async (accelerator) => {
                setShortcutError(null);
                try {
                  await commands.setShortcut(accelerator);
                  refresh();
                } catch (error) {
                  setShortcutError(commands.errorMessage(error));
                }
              }}
            />
            {shortcutError && (
              <p className="mt-2 max-w-[280px] text-[12px] text-stop">
                {shortcutError}
              </p>
            )}
            {!status.shortcutRegistered && !shortcutError && (
              <p className="mt-2 max-w-[280px] text-[12px] text-warn">
                This shortcut isn't active. Another app may already be using it.
              </p>
            )}
          </div>

          <Segmented
            value={status.settings.behavior}
            onChange={async (behavior) => {
              await commands.setDictationBehavior(behavior);
              refresh();
            }}
            segments={[
              { value: "hold", label: "Hold to talk" },
              { value: "toggle", label: "Press to toggle" },
            ]}
          />
        </div>
      </Section>

      <Section
        title="Transcription"
        description="Everything runs on this Mac. Your audio is never uploaded, and no account or key is needed."
      >
        <div className="flex flex-wrap items-center gap-4">
          <div className="min-w-0">
            <p className="display text-[15px] text-ink">{status.providerName}</p>
            <p className="text-[12.5px] text-ink-2">
              {status.modelName}
              {status.providerReady ? "" : " · not downloaded yet"}
            </p>
          </div>
          <Button className="ml-auto" onClick={onOpenModels}>
            Choose engine and models
          </Button>
        </div>
      </Section>

      <Section
        title="Live typing"
        description="Words appear in the app you are speaking to as you say them, instead of all at once when you stop."
      >
        <label className="flex items-center gap-3">
          <Toggle
            checked={status.settings.liveTyping}
            label="Type words as you speak"
            onChange={async (next) => {
              await commands.setLiveTyping(next);
              refresh();
            }}
          />
          <span className="text-[13px] text-ink">
            {status.settings.liveTyping ? "On" : "Off"}
          </span>
        </label>
        <p className="mt-3 text-[12px] leading-relaxed text-ink-3">
          {liveTypingNote(status)}
        </p>
      </Section>

      <Section
        title="Music and other audio"
        description="Music playing while you speak makes clide hear you worse. This turns the Mac's volume down while you record."
      >
        <label className="flex items-start gap-3">
          <Toggle
            checked={status.settings.lowerAudioWhileDictating}
            label="Lower other audio while dictating"
            onChange={async (next) => {
              await commands.setLowerAudioWhileDictating(next);
              refresh();
            }}
          />
          <span className="text-[13px] leading-snug text-ink">
            Lower other audio while dictating
            <span className="mt-1 block text-[12px] text-ink-3">
              {status.settings.lowerAudioWhileDictating
                ? "The Mac's volume drops to a quarter while the microphone is open and is put back as soon as you stop. Nothing is paused."
                : "Your music keeps playing at full volume while you speak."}
            </span>
          </span>
        </label>
      </Section>

      <Section
        title="Dictionary"
        description="Words clide should spell your way: names, products, jargon. It also remembers the words you say."
      >
        <DictionarySection status={status} refresh={refresh} />
      </Section>

      <Section
        title="Spoken punctuation"
        description="Say &ldquo;comma&rdquo;, &ldquo;new line&rdquo; or &ldquo;question mark&rdquo; and clide types the punctuation instead of the word. Works in every mode and adds no delay."
      >
        <label className="flex items-center gap-3">
          <Toggle
            checked={status.settings.spokenPunctuation}
            label="Turn spoken punctuation into punctuation marks"
            onChange={async (next) => {
              await commands.setSpokenPunctuation(next);
              refresh();
            }}
          />
          <span className="text-[13px] text-ink">
            {status.settings.spokenPunctuation ? "On" : "Off"}
          </span>
        </label>
        <p className="mt-3 text-[12px] leading-relaxed text-ink-3">
          {status.settings.spokenPunctuation
            ? "\u201cready comma set\u201d becomes \u201cready, set\u201d."
            : "The words \u201ccomma\u201d and \u201cperiod\u201d are typed out as you said them."}
        </p>
      </Section>

      <Section
        title="Developer dictation"
        description="Helps clide hear technical words in editors, terminals and browsers, and can write spoken file paths as code."
      >
        <div className="flex flex-col gap-4">
          <Segmented
            className="w-full"
            value={status.settings.technicalVocabulary}
            onChange={async (setting) => {
              await commands.setTechnicalVocabulary(setting);
              refresh();
            }}
            segments={[
              { value: "auto", label: "Dev apps", hint: "Editors, terminals and browsers" },
              { value: "always", label: "Everywhere", hint: "Every app" },
              { value: "off", label: "Off", hint: "Never" },
            ]}
          />
          {status.settings.technicalVocabulary !== "off" &&
            !status.providerPrompting && (
              <p className="text-[12px] leading-relaxed text-warn">
                {status.providerName} can't take vocabulary hints. Switch to a
                Whisper model on the Models page and this takes effect.
              </p>
            )}

          <label className="flex items-start gap-3">
            <Toggle
              checked={status.settings.formatTechnicalTerms}
              label="Write spoken file paths as code"
              onChange={async (next) => {
                await commands.setFormatTechnicalTerms(next);
                refresh();
              }}
            />
            <span className="text-[13px] leading-snug text-ink">
              Write spoken file paths as code
              <span className="mt-1 block text-[12px] text-ink-3">
                &ldquo;open src slash app dot tsx&rdquo; becomes{" "}
                <code className="font-mono">`src/app.tsx`</code>.
              </span>
            </span>
          </label>
          {status.settings.formatTechnicalTerms && (
            <p className="text-[12px] leading-relaxed text-warn">
              The backticks are typed as real characters. Leave this off for
              terminals and code editors, where they would break a command or
              clutter your source.
            </p>
          )}
        </div>
      </Section>

      <Section
        title="Local API"
        description="Lets another app on this Mac transcribe audio with Clide or follow its recording state. Off unless you turn it on."
      >
        <LocalApiSection status={status} refresh={refresh} />
      </Section>

      <Section
        title="Rewrite"
        description="Rewrite mode cleans the transcript locally, then asks an on-device model to finish the job. Nothing is sent anywhere."
      >
        <RefineSection status={status} refresh={refresh} />
      </Section>

      <Section
        title="If an engine fails"
        description="clide never switches engines quietly. When a substitute runs, the HUD says which one — so a transcript that reads differently always has an explanation."
      >
        <Segmented
          className="w-full"
          value={status.settings.fallback}
          onChange={async (fallback) => {
            await commands.setFallbackPolicy(fallback);
            refresh();
          }}
          segments={[
            { value: "off", label: "Just tell me", hint: "Report the failure and let me choose" },
            {
              value: "localOnly",
              label: "Try another engine",
              hint: "Another on-device engine takes over",
            },
          ]}
        />
        <p className="mt-3 text-[12px] leading-relaxed text-ink-3">
          {status.settings.fallback === "localOnly"
            ? "Another engine on this Mac takes over, and the HUD says which."
            : "Nothing is substituted. You'll get a Retry button instead."}
        </p>
      </Section>

      <Section
        title="Visual effects"
        description="How much motion the background shader is allowed. macOS Reduce Motion always wins over this setting."
      >
        <Segmented
          value={status.settings.visualIntensity}
          onChange={async (intensity) => {
            await commands.setVisualIntensity(intensity);
            refresh();
          }}
          segments={[
            { value: "reduced", label: "Reduced", hint: "Static background" },
            { value: "normal", label: "Normal", hint: "Slow ambient drift" },
            { value: "high", label: "High", hint: "Reacts to your voice" },
          ]}
        />
      </Section>

      <Section
        title="Permissions"
        description="clide needs the microphone to hear you and Accessibility to type into other applications."
      >
        <div className="flex flex-wrap gap-2">
          <Button onClick={() => commands.openMicrophoneSettings()}>
            Microphone settings
          </Button>
          <Button onClick={() => commands.openAccessibilitySettings()}>
            Accessibility settings
          </Button>
          {status.permissions.accessibility !== "granted" && !status.adHocBuild && (
            <Button
              onClick={async () => {
                setPermissionRepair(null);
                try {
                  await commands.repairAccessibilityPermission();
                  setPermissionRepair(
                    "Clide's stale permission entry was cleared. Grant access in the macOS prompt, then return here.",
                  );
                  refresh();
                } catch (error) {
                  setPermissionRepair(commands.errorMessage(error));
                }
              }}
            >
              Repair accessibility
            </Button>
          )}
          <Button
            variant="ghost"
            onClick={async () => {
              await commands.resetOnboarding();
              refresh();
            }}
          >
            Run setup again
          </Button>
          <Button
            variant="ghost"
            onClick={async () => {
              setInsertionTest("Focus an editable field — testing in 2 seconds…");
              try {
                const result = await commands.testInsertion();
                setInsertionTest(`Inserted into ${result}.`);
              } catch (error) {
                setInsertionTest(commands.errorMessage(error));
              }
            }}
          >
            Test insertion
          </Button>
        </div>
        {insertionTest && (
          <p className="mt-3 text-[12px] leading-relaxed text-ink-3">
            {insertionTest}
          </p>
        )}
        {permissionRepair && (
          <p className="mt-2 text-[12px] leading-relaxed text-ink-3">
            {permissionRepair}
          </p>
        )}
      </Section>

      <Section
        title="About"
        description="clide is free and open source. If something is broken, the issue tracker is the fastest way to reach us — the build number above tells us exactly what you are running."
      >
        <AboutSection />
      </Section>
    </div>
  );
}

/**
 * One settings group: what it is on the left, its controls on the right.
 *
 * Plain rows divided by hairlines. Settings used to be a bento of cards, which
 * made every group look equally important and hid the page's actual order.
 */
function Section({
  title,
  description,
  children,
}: {
  title: string;
  description: string;
  children: React.ReactNode;
}) {
  return (
    <section className="grid grid-cols-1 gap-x-12 gap-y-4 py-7 md:grid-cols-[230px_minmax(0,1fr)]">
      <div>
        <h2 className="display text-[14.5px] text-ink">{title}</h2>
        <p className="mt-1.5 text-[12.5px] leading-relaxed text-ink-2">
          {description}
        </p>
      </div>
      <div className="min-w-0">{children}</div>
    </section>
  );
}

function liveTypingNote(status: SystemStatus): string {
  if (!status.settings.liveTyping) {
    return "Everything is typed in one go when you stop speaking.";
  }
  if (!status.providerStreaming) {
    return `${status.providerName} can only transcribe a finished recording. Choose Apple Speech to type as you speak.`;
  }
  if (status.settings.mode === "rewrite") {
    return "Rewrite needs the whole recording, so this is paused while Rewrite is the style.";
  }
  return "Spoken corrections such as \u201cscratch that\u201d are typed as words while this is on, because the text is already on screen.";
}
