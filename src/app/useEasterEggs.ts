import { useEffect, useState } from "react";

const KONAMI = [
  "ArrowUp",
  "ArrowUp",
  "ArrowDown",
  "ArrowDown",
  "ArrowLeft",
  "ArrowRight",
  "ArrowLeft",
  "ArrowRight",
  "b",
  "a",
];

/**
 * Hidden things.
 *
 * Deliberately small and deliberately harmless: an easter egg in a utility
 * should never change what the app *does*. These only affect how it looks, and
 * every one of them wears off on its own.
 */
export function useEasterEggs() {
  const [surge, setSurge] = useState(false);

  useEffect(() => {
    let progress = 0;

    const onKeyDown = (event: KeyboardEvent) => {
      // Never while the user is typing into something.
      const target = event.target as HTMLElement | null;
      if (
        target &&
        (target.tagName === "INPUT" ||
          target.tagName === "TEXTAREA" ||
          target.isContentEditable)
      ) {
        progress = 0;
        return;
      }

      const expected = KONAMI[progress];
      const pressed =
        expected.length === 1 ? event.key.toLowerCase() : event.key;

      if (pressed === expected) {
        progress += 1;
        if (progress === KONAMI.length) {
          progress = 0;
          setSurge(true);
          window.setTimeout(() => setSurge(false), 9000);
        }
      } else {
        // Allow a wrong key to be the start of a fresh attempt.
        progress = pressed === KONAMI[0] ? 1 : 0;
      }
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  return { surge };
}

/**
 * A note for whoever opens the console.
 *
 * clide is open source; someone poking around here is a potential contributor,
 * so this points them at the repository rather than being a joke alone.
 */
export function greetTheCurious() {
  const style = "color:#111112;font-weight:600";
  console.log(
    "%c▁▃▆▄▂  clide\n%cYour voice. Your models. Your words.\nSource: https://github.com/staraepp/clide_stt",
    style,
    "color:#4A6076",
  );
}
