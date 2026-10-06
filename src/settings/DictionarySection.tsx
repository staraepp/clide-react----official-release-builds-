import { useState } from "react";
import { Pin, X } from "lucide-react";

import { Button } from "@/components/Button";
import { Segmented } from "@/components/Segmented";
import { TextField } from "@/components/TextField";
import { Toggle } from "@/components/Toggle";
import * as commands from "@/lib/commands";
import { cn } from "@/lib/cn";
import type { DictionaryEntry, SystemStatus, WordSource } from "@/lib/types";
import { useDictionary } from "./useDictionary";

type Filter = "all" | WordSource;

/**
 * Words clide knows. Ones you add are spelled your way and hinted to the
 * engine; ones it learns are only a record until you keep them.
 */
export function DictionarySection({
  status,
  refresh,
}: {
  status: SystemStatus;
  refresh: () => void;
}) {
  const [draft, setDraft] = useState("");
  const [search, setSearch] = useState("");
  const [filter, setFilter] = useState<Filter>("all");
  const [error, setError] = useState<string | null>(null);
  const [confirmingClear, setConfirmingClear] = useState(false);

  const { entries, counts } = useDictionary({
    search: search.trim() || undefined,
    source: filter === "all" ? undefined : filter,
  });
  const total = counts.manual + counts.learned;

  async function run(action: () => Promise<unknown>) {
    setError(null);
    try {
      await action();
    } catch (cause) {
      setError(commands.errorMessage(cause));
    }
  }

  async function add() {
    if (!draft.trim()) return;
    await run(async () => {
      await commands.addDictionaryWord(draft);
      setDraft("");
    });
  }

  return (
    <div className="flex flex-col gap-5">
      <label className="flex items-start gap-3">
        <Toggle
          checked={status.settings.dictionaryAutoLearn}
          label="Learn words as you dictate"
          onChange={async (next) => {
            await commands.setDictionaryAutoLearn(next);
            refresh();
          }}
        />
        <span className="text-[13px] leading-snug text-ink">
          Learn words as you dictate
          <span className="mt-1 block text-[12px] text-ink-3">
            Every word of a finished dictation is added to the list below, on
            this Mac only. Learned words don't change your text until you keep
            them.
          </span>
        </span>
      </label>

      <form
        className="flex flex-col gap-1.5"
        onSubmit={(event) => {
          event.preventDefault();
          void add();
        }}
      >
        <label htmlFor="dictionary-word" className="text-[12px] text-ink-2">
          Add a word or phrase
        </label>
        <div className="flex items-center gap-2">
          <TextField
            id="dictionary-word"
            value={draft}
            maxLength={60}
            spellCheck={false}
            autoComplete="off"
            placeholder="Kubernetes, T3 Code, Breno…"
            onChange={(event) => setDraft(event.target.value)}
          />
          <Button type="submit" variant="primary" disabled={!draft.trim()}>
            Add
          </Button>
        </div>
        <p className="text-[12px] leading-relaxed text-ink-3">
          clide spells it exactly like this whenever it hears it, in any case,
          and hints it to engines that accept vocabulary. Adding a common word
          such as &ldquo;Will&rdquo; respells every &ldquo;will&rdquo;. Spelling
          isn't corrected while live typing is on, because the text is already
          on screen.
        </p>
      </form>

      {error && (
        <p role="alert" className="text-[12.5px] leading-relaxed text-warn">
          {error}
        </p>
      )}

      <div className="flex flex-col gap-3">
        <div className="flex flex-wrap items-center gap-3">
          <Segmented
            value={filter}
            onChange={setFilter}
            segments={[
              { value: "all", label: "All" },
              { value: "manual", label: "Yours" },
              { value: "auto", label: "Learned" },
            ]}
          />
          <TextField
            aria-label="Search the dictionary"
            type="search"
            value={search}
            spellCheck={false}
            placeholder="Search"
            className="h-8 min-w-0 flex-1 basis-32"
            onChange={(event) => setSearch(event.target.value)}
          />
        </div>

        {entries.length === 0 ? (
          <p className="text-[12.5px] leading-relaxed text-ink-3">
            {total === 0
              ? "Nothing here yet. Words appear as you dictate, or add your own above."
              : "No words match."}
          </p>
        ) : (
          <ul
            aria-label="Dictionary"
            className="flex max-h-64 flex-wrap content-start gap-1.5 overflow-y-auto pr-1"
          >
            {entries.map((entry) => (
              <Chip key={entry.word} entry={entry} onError={setError} />
            ))}
          </ul>
        )}

        {total > 0 && (
          <p className="text-[12px] text-ink-3">
            {counts.manual} yours, {counts.learned} learned
          </p>
        )}

        {counts.learned > 0 && (
          <div className="flex items-center gap-2">
            {confirmingClear ? (
              <>
                <span className="text-[12px] text-ink-2">
                  Forget {counts.learned} learned{" "}
                  {counts.learned === 1 ? "word" : "words"}? Yours stay.
                </span>
                <Button
                  size="sm"
                  variant="danger"
                  onClick={() => {
                    setConfirmingClear(false);
                    void run(() => commands.clearLearnedWords());
                  }}
                >
                  Forget them
                </Button>
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() => setConfirmingClear(false)}
                >
                  Cancel
                </Button>
              </>
            ) : (
              <Button
                size="sm"
                variant="ghost"
                onClick={() => setConfirmingClear(true)}
              >
                Clear learned words
              </Button>
            )}
          </div>
        )}
      </div>
    </div>
  );
}

function Chip({
  entry,
  onError,
}: {
  entry: DictionaryEntry;
  onError: (message: string | null) => void;
}) {
  const yours = entry.source === "manual";

  async function run(action: () => Promise<unknown>) {
    onError(null);
    try {
      await action();
    } catch (cause) {
      onError(commands.errorMessage(cause));
    }
  }

  return (
    <li
      className={cn(
        "no-drag inline-flex h-7 max-w-full items-center gap-1 rounded-full border pl-3 pr-1 text-[12.5px]",
        yours
          ? "border-line-2 bg-card text-ink"
          : "border-line bg-sunken text-ink-2",
      )}
    >
      <span className="truncate">{entry.word}</span>
      {entry.uses > 0 && (
        <span className="font-mono text-[10.5px] text-ink-3" title="Times dictated">
          {entry.uses}
        </span>
      )}
      {!yours && (
        <button
          type="button"
          aria-label={`Keep ${entry.word} and spell it this way`}
          title="Keep: spell it like this"
          className="grid size-5 place-items-center rounded-full text-ink-3 transition-colors hover:bg-card hover:text-ink"
          onClick={() => run(() => commands.addDictionaryWord(entry.word))}
        >
          <Pin size={11} />
        </button>
      )}
      <button
        type="button"
        aria-label={`Remove ${entry.word}`}
        title="Remove"
        className="grid size-5 place-items-center rounded-full text-ink-3 transition-colors hover:bg-card hover:text-stop"
        onClick={() => run(() => commands.removeDictionaryWord(entry.word))}
      >
        <X size={12} />
      </button>
    </li>
  );
}
