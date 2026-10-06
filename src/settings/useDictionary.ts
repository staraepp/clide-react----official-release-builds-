import { useCallback, useEffect, useState } from "react";
import * as commands from "@/lib/commands";
import { EVENTS, on } from "@/lib/events";
import type { DictionaryPage, DictionaryQuery } from "@/lib/types";

const EMPTY: DictionaryPage = { entries: [], counts: { manual: 0, learned: 0 } };

/**
 * The dictionary for a given search and filter.
 *
 * Reads go to SQLite on every change, and the list refreshes itself when a
 * dictation teaches it a new word while Settings is open.
 */
export function useDictionary(query: DictionaryQuery) {
  const [page, setPage] = useState<DictionaryPage>(EMPTY);
  const [loading, setLoading] = useState(true);

  const key = JSON.stringify(query);

  const refresh = useCallback(async () => {
    try {
      setPage(await commands.getDictionary(JSON.parse(key)));
    } catch (error) {
      console.error("could not read the dictionary", error);
    } finally {
      setLoading(false);
    }
  }, [key]);

  useEffect(() => {
    refresh();
    const subscription = on(EVENTS.dictionaryChanged, refresh);
    return () => {
      subscription.then((unlisten) => unlisten());
    };
  }, [refresh]);

  return { ...page, loading, refresh };
}
