import { useEffect, useState } from "react";

import { Wordmark } from "@/components/Wordmark";
import { Button } from "@/components/Button";
import { CopyButton } from "@/components/CopyButton";
import * as commands from "@/lib/commands";
import type { About, UpdateStatus } from "@/lib/types";

/**
 * Which build this is, and whether a newer one exists.
 *
 * Updates install in place, from a signed package, and only when asked.
 *
 * The commit is here so a bug report can name the exact build rather than "the
 * latest one", and it is copyable for the same reason.
 */
export function AboutSection() {
  const [about, setAbout] = useState<About | null>(null);
  const [update, setUpdate] = useState<UpdateStatus | null>(null);
  const [checking, setChecking] = useState(false);
  const [installing, setInstalling] = useState(false);
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

  const install = async () => {
    setInstalling(true);
    setUpdateError(null);
    try {
      // On success the app relaunches, so there is nothing to do afterwards.
      await commands.installUpdate();
    } catch (error) {
      setUpdateError(commands.errorMessage(error));
      setInstalling(false);
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
    <div className="flex flex-col gap-5">
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

      <div className="flex items-center gap-3">
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
                ? `Checked ${new Date(update.checkedAt).toLocaleString()}`
                : "Checks once a day")}
          </span>
        </span>
        {update?.updateAvailable ? (
          <Button size="sm" variant="primary" disabled={installing} onClick={install}>
            {installing ? "Installing…" : "Install and restart"}
          </Button>
        ) : (
          <Button size="sm" disabled={checking} onClick={checkNow}>
            {checking ? "Checking…" : "Check now"}
          </Button>
        )}
      </div>

      <p className="text-[11.5px] text-ink-3">
        {about.license} licensed. Everything runs on this Mac.
      </p>
    </div>
  );
}
