import { useCallback, useEffect, useState } from "react";
import { Eye, EyeOff } from "lucide-react";

import { Button } from "@/components/Button";
import { CopyButton } from "@/components/CopyButton";
import { TextArea, TextField } from "@/components/TextField";
import { Toggle } from "@/components/Toggle";
import * as commands from "@/lib/commands";
import type { LocalApiStatus, SystemStatus } from "@/lib/types";

/**
 * The optional local HTTP API: off by default, loopback only, bearer token on
 * every request. Everything here only changes what other apps on this Mac may
 * ask of Clide; nothing leaves the machine.
 */
export function LocalApiSection({
  status,
  refresh,
}: {
  status: SystemStatus;
  refresh: () => void;
}) {
  const { settings } = status;
  const enabled = settings.localApiEnabled;

  const [api, setApi] = useState<LocalApiStatus | null>(null);
  const [token, setToken] = useState<string | null>(null);
  const [revealed, setRevealed] = useState(false);
  const [port, setPort] = useState(String(settings.localApiPort));
  const [origins, setOrigins] = useState(settings.localApiAllowedOrigins.join("\n"));
  const [error, setError] = useState<string | null>(null);

  const reload = useCallback(() => {
    commands
      .getLocalApiStatus()
      .then(setApi)
      .catch((cause) => console.error("could not read API status", cause));
  }, []);

  useEffect(reload, [reload, settings.localApiEnabled, settings.localApiPort]);

  useEffect(() => {
    if (!enabled) {
      setToken(null);
      setRevealed(false);
      return;
    }
    commands
      .getLocalApiToken()
      .then(setToken)
      .catch((cause) => setError(commands.errorMessage(cause)));
  }, [enabled]);

  useEffect(() => {
    setPort(String(settings.localApiPort));
  }, [settings.localApiPort]);

  async function run(action: () => Promise<unknown>) {
    setError(null);
    try {
      await action();
      refresh();
      reload();
    } catch (cause) {
      setError(commands.errorMessage(cause));
    }
  }

  const masked = token ? "•".repeat(24) : "";

  return (
    <div className="flex flex-col gap-5">
      <label className="flex items-start gap-3">
        <Toggle
          checked={enabled}
          label="Enable the local API"
          onChange={(next) => run(() => commands.setLocalApiEnabled(next))}
        />
        <span className="text-[13px] leading-snug text-ink">
          Let other apps on this Mac use Clide
          <span className="mt-1 block text-[12px] text-ink-3">
            Off by default. Listens on 127.0.0.1 only, so nothing on your
            network can reach it, and every request needs the token below.
          </span>
        </span>
      </label>

      {enabled && (
        <>
          <p
            role="status"
            className={
              api?.error
                ? "text-[12.5px] leading-relaxed text-warn"
                : "text-[12.5px] leading-relaxed text-ink-2"
            }
          >
            {api?.error
              ? api.error
              : api?.running
                ? `Listening on ${api.address}`
                : "Starting…"}
          </p>

          <div className="flex flex-col gap-1.5">
            <label htmlFor="api-token" className="text-[12px] text-ink-2">
              Token
            </label>
            <TextField
              id="api-token"
              readOnly
              spellCheck={false}
              value={revealed ? (token ?? "") : masked}
              className="font-mono text-[10.5px]"
              onFocus={(event) => event.currentTarget.select()}
            />
            <div className="flex items-center gap-2">
              <Button
                size="sm"
                variant="ghost"
                aria-label={revealed ? "Hide token" : "Show token"}
                onClick={() => setRevealed((shown) => !shown)}
              >
                {revealed ? <EyeOff size={13} /> : <Eye size={13} />}
              </Button>
              {token && <CopyButton text={token} label="Copy" variant="surface" />}
              <Button
                size="sm"
                onClick={() => run(() => commands.regenerateLocalApiToken().then(setToken))}
              >
                Regenerate
              </Button>
            </div>
            <p className="text-[12px] leading-relaxed text-ink-3">
              Send it as <code className="font-mono">Authorization: Bearer …</code>.
              Regenerating signs out every app using the old one.
            </p>
          </div>

          <div className="flex flex-col gap-1.5">
            <label htmlFor="api-port" className="text-[12px] text-ink-2">
              Port
            </label>
            <TextField
              id="api-port"
              inputMode="numeric"
              value={port}
              className="w-28 font-mono"
              onChange={(event) => setPort(event.target.value.replace(/\D/g, ""))}
              onBlur={() => {
                if (port === String(settings.localApiPort)) return;
                const next = Number(port);
                if (!Number.isInteger(next)) {
                  setPort(String(settings.localApiPort));
                  return;
                }
                run(() => commands.setLocalApiPort(next));
              }}
            />
          </div>

          <div className="flex flex-col gap-3">
            <label className="flex items-start gap-3">
              <Toggle
                checked={settings.localApiTranscription}
                label="Transcription endpoint"
                onChange={(next) =>
                  run(() => commands.setLocalApiEndpoints(next, settings.localApiEvents))
                }
              />
              <span className="text-[13px] leading-snug text-ink">
                Transcription
                <span className="mt-1 block text-[12px] text-ink-3">
                  <code className="font-mono">POST /v1/audio/transcriptions</code>{" "}
                  turns an uploaded file into text. It never types anything.
                </span>
              </span>
            </label>
            <label className="flex items-start gap-3">
              <Toggle
                checked={settings.localApiEvents}
                label="Event stream"
                onChange={(next) =>
                  run(() =>
                    commands.setLocalApiEndpoints(settings.localApiTranscription, next),
                  )
                }
              />
              <span className="text-[13px] leading-snug text-ink">
                Event stream
                <span className="mt-1 block text-[12px] text-ink-3">
                  <code className="font-mono">GET /v1/events</code> reports when
                  you record, a live mic level, and each finished transcript.
                </span>
              </span>
            </label>
          </div>

          <div className="flex flex-col gap-1.5">
            <label htmlFor="api-origins" className="text-[12px] text-ink-2">
              Allowed web origins
            </label>
            <TextArea
              id="api-origins"
              rows={2}
              spellCheck={false}
              placeholder="None. Browser pages are refused."
              value={origins}
              className="font-mono"
              onChange={(event) => setOrigins(event.target.value)}
              onBlur={() => {
                const next = origins
                  .split("\n")
                  .map((line) => line.trim())
                  .filter(Boolean);
                if (next.join("\n") === settings.localApiAllowedOrigins.join("\n")) return;
                run(() => commands.setLocalApiOrigins(next));
              }}
            />
            <p className="text-[12px] leading-relaxed text-ink-3">
              One per line, like <code className="font-mono">http://localhost:3000</code>.
              Native apps and scripts need nothing here.
            </p>
          </div>
        </>
      )}

      {error && (
        <p role="alert" className="text-[12.5px] leading-relaxed text-warn">
          {error}
        </p>
      )}
    </div>
  );
}
