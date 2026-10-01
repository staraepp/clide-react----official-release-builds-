import { useCallback, useEffect, useState } from "react";
import { AnimatePresence, motion } from "motion/react";
import { Cpu, HardDrive, Info } from "lucide-react";

import { Card } from "@/components/Card";
import { StatusDot } from "@/components/StatusDot";
import { ModelCard, BuiltInModelRow } from "./ModelCard";
import * as commands from "@/lib/commands";
import { EVENTS, on } from "@/lib/events";
import type { DownloadProgress, ModelsPage, ProviderDescriptor } from "@/lib/types";
import { cn } from "@/lib/cn";

/**
 * Providers and models as one decision.
 *
 * Picking an engine and picking its model are the same choice made twice, so
 * they share a screen: providers across the top, then a feed of that provider's
 * models. Local models are ranked by how well they run on *this* Mac, which is
 * measured in Rust rather than guessed at here.
 */
export function ModelsView() {
  const [page, setPage] = useState<ModelsPage | null>(null);
  const [progress, setProgress] = useState<Record<string, DownloadProgress>>({});
  const [failures, setFailures] = useState<Record<string, string>>({});
  const [activeProvider, setActiveProvider] = useState<string | null>(null);
  const [languageFilter, setLanguageFilter] = useState<LanguageFilter>("all");

  const refresh = useCallback(async () => {
    try {
      const next = await commands.getModelsPage();
      setPage(next);
      setActiveProvider((current) => current ?? next.selectedProvider);
    } catch (error) {
      console.error("could not read the models page", error);
    }
  }, []);

  useEffect(() => {
    refresh();

    const subscriptions = [
      on(EVENTS.modelProgress, (event) => {
        setProgress((current) => ({ ...current, [event.modelId]: event }));
      }),
      on(EVENTS.modelComplete, (modelId) => {
        setProgress((current) => {
          const next = { ...current };
          delete next[modelId];
          return next;
        });
        refresh();
      }),
      on(EVENTS.modelFailed, (event) => {
        setProgress((current) => {
          const next = { ...current };
          delete next[event.modelId];
          return next;
        });
        setFailures((current) => ({ ...current, [event.modelId]: event.message }));
      }),
    ];

    return () => {
      subscriptions.forEach((unsubscribe) => unsubscribe.then((fn) => fn()));
    };
  }, [refresh]);

  if (!page) {
    return <div className="p-3" />;
  }

  const provider =
    page.providers.find((candidate) => candidate.id === activeProvider) ??
    page.providers[0];
  const catalogEngine = engineOf(provider.id);
  const localModels = catalogEngine
    ? page.models.filter((model) => model.engine === catalogEngine)
    : [];
  const visibleLocalModels = localModels.filter((model) => {
    if (languageFilter === "multilingual") return model.multilingual;
    if (languageFilter === "english") return !model.multilingual;
    return true;
  });

  return (
    <div className="flex flex-col gap-3 py-3">
      <Hardware page={page} />

      <section className="flex flex-col gap-2">
        <h2 className="label px-1">Engines</h2>
        <div className="grid grid-cols-1 gap-2 sm:grid-cols-3">
          {page.providers.map((candidate, index) => (
            <ProviderTile
              key={candidate.id}
              provider={candidate}
              active={candidate.id === provider.id}
              inUse={candidate.id === page.selectedProvider}
              index={index}
              onClick={() => setActiveProvider(candidate.id)}
            />
          ))}
        </div>
      </section>

      <section className="flex flex-col gap-2">
        <div className="flex flex-wrap items-center gap-2 px-1">
          <h2 className="label">{provider.name} models</h2>
          {catalogEngine && (
            <>
              <span className="text-[11.5px] text-ink-3">
                {localModels.length} available · ranked for your {page.hardware.chip}
              </span>
              {provider.id === "local-whisper" && (
                <LanguageFilters value={languageFilter} onChange={setLanguageFilter} />
              )}
            </>
          )}
        </div>

        <AnimatePresence mode="wait">
          <motion.div
            key={provider.id}
            initial={{ opacity: 0, y: 6 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.2 }}
            className={cn(
              "grid grid-cols-1 gap-2 lg:grid-cols-2",
            )}
          >
            {catalogEngine
              ? visibleLocalModels.map((model, index) => (
                    <div key={model.id} className="flex flex-col gap-1">
                      <ModelCard
                        model={model}
                        index={index}
                        progress={progress[model.id]}
                        selected={
                          page.selectedProvider === provider.id &&
                          page.selectedModel === model.id
                        }
                        onDownload={() => {
                          setFailures((current) => {
                            const next = { ...current };
                            delete next[model.id];
                            return next;
                          });
                          commands.downloadModel(model.id);
                        }}
                        onRemove={async () => {
                          await commands.removeModel(model.id);
                          refresh();
                        }}
                        onSelect={async () => {
                          await commands.selectProvider(provider.id, model.id);
                          refresh();
                        }}
                      />
                      {failures[model.id] && (
                        <p className="px-1 text-[11.5px] text-stop">
                          {failures[model.id]}
                        </p>
                      )}
                    </div>
                  ))
              : provider.models.map((model) => (
                  <BuiltInModelRow
                    key={model.id}
                    name={model.name}
                    description={model.description}
                    selected={
                      page.selectedProvider === provider.id &&
                      page.selectedModel === model.id
                    }
                    onSelect={async () => {
                      await commands.selectProvider(provider.id, model.id);
                      refresh();
                    }}
                  />
                ))}
          </motion.div>
        </AnimatePresence>

        {catalogEngine && visibleLocalModels.length === 0 && (
            <p className="px-1 py-6 text-[13px] text-ink-3">
              No models match this filter.
            </p>
          )}
      </section>
    </div>
  );
}

type LanguageFilter = "all" | "multilingual" | "english";

function LanguageFilters({
  value,
  onChange,
}: {
  value: LanguageFilter;
  onChange: (filter: LanguageFilter) => void;
}) {
  const options: Array<{ value: LanguageFilter; label: string }> = [
    { value: "all", label: "All" },
    { value: "multilingual", label: "Multilingual" },
    { value: "english", label: "English only" },
  ];

  return (
    <div className="ml-auto inline-flex rounded-lg border border-line bg-sunken p-0.5">
      {options.map((option) => (
        <button
          key={option.value}
          type="button"
          onClick={() => onChange(option.value)}
          aria-pressed={value === option.value}
          className={cn(
            "rounded-md px-2 py-1 text-[11px] transition-colors",
            value === option.value
              ? "bg-ink text-card"
              : "text-ink-3 hover:text-ink",
          )}
        >
          {option.label}
        </button>
      ))}
    </div>
  );
}

/**
 * Which downloadable catalogue an engine draws from, or `null` for an engine
 * that ships with macOS and has nothing to download.
 */
function engineOf(providerId: string): "whisper" | "parakeet" | null {
  if (providerId === "local-parakeet") return "parakeet";
  if (providerId === "local-whisper") return "whisper";
  return null;
}

function ProviderTile({
  provider,
  active,
  inUse,
  index,
  onClick,
}: {
  provider: ProviderDescriptor;
  active: boolean;
  inUse: boolean;
  index: number;
  onClick: () => void;
}) {
  return (
    <motion.button
      type="button"
      onClick={onClick}
      initial={{ opacity: 0, y: 6 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.3, delay: index * 0.03 }}
      className={cn(
        "card flex flex-col items-start gap-1.5 p-3 text-left transition-colors",
        active ? "border-voice bg-voice-tint/40" : "hover:bg-sunken",
      )}
    >
      <span className="flex w-full items-center gap-1.5">
        <Cpu size={12} className="text-ink-3" />
        <span className="display truncate text-[13.5px]">{provider.name}</span>
        {inUse && <StatusDot tone="ready" className="ml-auto" />}
      </span>

      <span className="text-[11px] text-ink-3">
        {engineOf(provider.id) ? "Downloaded models" : "Built into macOS"}
      </span>
    </motion.button>
  );
}

/** What the rankings were measured against, stated plainly. */
function Hardware({ page }: { page: ModelsPage }) {
  return (
    <Card className="flex flex-wrap items-center gap-x-6 gap-y-2 p-4">
      <span className="flex items-center gap-2">
        <Cpu size={13} className="text-ink-3" />
        <span className="text-[13px] text-ink">{page.hardware.chip}</span>
      </span>
      <span className="flex items-center gap-2">
        <HardDrive size={13} className="text-ink-3" />
        <span className="text-[13px] text-ink">{page.memoryLabel} memory</span>
      </span>
      <span className="text-[12.5px] text-ink-3">
        {page.hardware.performanceCores} performance cores
        {page.hardware.appleSilicon ? " · Metal acceleration" : ""}
      </span>
      <span className="ml-auto flex items-center gap-1.5 text-[11.5px] text-ink-3">
        <Info size={11} />
        Ratings come from this hardware, not from a leaderboard.
      </span>
    </Card>
  );
}
