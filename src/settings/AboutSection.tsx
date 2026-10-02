import { useEffect, useState } from "react";
import { motion } from "motion/react";
import { ArrowUpRight, Bug, Download, Globe, RefreshCw, Scale } from "lucide-react";

import { Wordmark } from "@/components/Wordmark";
import { CopyButton } from "@/components/CopyButton";
import * as commands from "@/lib/commands";
import { PRESS } from "@/lib/motion";
import type { About, UpdateStatus } from "@/lib/types";

/**
 * Which build this is, and where to take it.
 *
 * The commit is here so a bug report can name the exact build rather than "the
 * latest one", and it is copyable for the same reason.
 */
export function AboutSection() {
  const [about, setAbout] = useState<About | null>(null);
  const [update, setUpdate] = useState<UpdateStatus | null>(null);
  const [checking, setChecking] = useState(false);
  const [updateError, setUpdateError] = useState<string | null>(null);

  useEffect(() => {
    commands
      .getAbout()
      .then(setAbout)
      .catch((error) => console.error("could not read build info", error));
    commands
      .checkForUpdates(false)
      .then(setUpdate)
      .catch((error) => setUpdateError(commands.errorMessage(error)));
  }, []);

  const checkNow = async () => {
    setChecking(true);
    setUpdateError(null);
    try {
      setUpdate(await commands.checkForUpdates(true));
    } catch (error) {
      setUpdateError(commands.errorMessage(error));
    } finally {
      setChecking(false);
    }
  };

  if (!about) return null;

  const built =
    about.buildDate && /^\d+$/.test(about.buildDate)
      ? new Date(Number(about.buildDate) * 1000).toLocaleDateString(undefined, {
          year: "numeric",
          month: "short",
          day: "numeric",
        })
      : null;

  return (
    <div className="flex flex-col gap-4">
      <div className="flex items-center gap-3">
        <span className="flex size-10 items-center justify-center rounded-xl border border-line bg-sunken">
          <Wordmark />
        </span>
        <div>
          <p className="display text-[15px]">clide {about.version}</p>
          <p className="numeral text-[11.5px] text-ink-3">
            {about.commit}
            {built ? ` · built ${built}` : ""}
          </p>
        </div>
        <CopyButton
          text={`clide ${about.version} (${about.commit})`}
          label="Copy build"
          variant="surface"
          className="ml-auto"
        />
      </div>

      <div className="flex items-center gap-3 rounded-ctl border border-line bg-card px-3 py-2.5">
        <span className="flex size-7 shrink-0 items-center justify-center rounded-lg bg-voice-tint text-voice-deep">
          {update?.updateAvailable ? <Download size={13} /> : <RefreshCw size={13} />}
        </span>
        <span className="min-w-0 flex-1">
          <span className="block text-[13px] text-ink">
            {update?.updateAvailable
              ? `clide ${update.latestVersion} is available`
              : update
                ? "clide is up to date"
                : updateError
                  ? "Update check unavailable"
                  : "Checking for updates…"}
          </span>
          <span className="block truncate text-[11.5px] text-ink-3">
            {updateError ??
              (update?.checkedAt
                ? `Checked ${new Date(update.checkedAt).toLocaleString()} · automatic daily check`
                : "Automatic daily check via GitHub Releases")}
          </span>
        </span>
        <motion.button
          type="button"
          {...PRESS}
          disabled={checking}
          onClick={() =>
            update?.updateAvailable ? commands.openUrl(update.releaseUrl) : checkNow()
          }
          className="rounded-lg border border-line bg-sunken px-2.5 py-1.5 text-[11.5px] text-ink transition-colors hover:border-line-2 disabled:opacity-50"
        >
          {checking ? "Checking…" : update?.updateAvailable ? "View update" : "Check now"}
        </motion.button>
      </div>

      <div className="grid gap-2 sm:grid-cols-2">
        <Link
          label="Source on GitHub"
          detail="staraepp/clide_stt"
          icon={<GitHubMark />}
          url={about.repository}
        />
        <Link
          label="clide.staraep.fun"
          detail="Website and downloads"
          icon={<Globe size={13} />}
          url={about.website}
        />
        <Link
          label="Report a bug"
          detail="Issues on GitHub"
          icon={<Bug size={13} />}
          url={about.issues}
        />
        <Link
          label={`${about.license} licensed`}
          detail="Free and open source, forever"
          icon={<Scale size={13} />}
          url={`${about.repository}/blob/main/LICENSE`}
        />
      </div>
    </div>
  );
}

function Link({
  label,
  detail,
  icon,
  url,
}: {
  label: string;
  detail: string;
  icon: React.ReactNode;
  url: string;
}) {
  return (
    <motion.button
      type="button"
      {...PRESS}
      onClick={() => commands.openUrl(url)}
      className="group flex items-center gap-2.5 rounded-ctl border border-line bg-card px-3 py-2.5 text-left transition-colors hover:border-line-2 hover:bg-sunken"
    >
      <span className="shrink-0 text-ink-3 transition-colors group-hover:text-voice-deep">
        {icon}
      </span>
      <span className="min-w-0">
        <span className="block truncate text-[13px] text-ink">{label}</span>
        <span className="block truncate text-[11.5px] text-ink-3">{detail}</span>
      </span>
      <ArrowUpRight
        size={12}
        className="ml-auto shrink-0 text-line-2 transition-colors group-hover:text-ink-3"
      />
    </motion.button>
  );
}

function GitHubMark() {
  return (
    <svg viewBox="0 0 16 16" width="13" height="13" fill="currentColor" aria-hidden>
      <path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27s1.36.09 2 .27c1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.01 8.01 0 0 0 16 8c0-4.42-3.58-8-8-8Z" />
    </svg>
  );
}
