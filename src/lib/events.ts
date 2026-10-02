/**
 * Event names emitted by the Rust core, and a typed `listen` helper.
 *
 * `DICTATION_STATE` is the authoritative one the UI renders from; the
 * lifecycle events exist for anything that needs a specific moment.
 */
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  FallbackPayload,
  DownloadProgress, DictationState } from "./types";

export const EVENTS = {
  dictationState: "dictation:state",
  dictationLevel: "dictation:level",
  dictationStarted: "dictation:started",
  dictationStopped: "dictation:stopped",
  transcriptionStarted: "transcription:started",
  transcriptionComplete: "transcription:complete",
  transcriptionFailed: "transcription:failed",
  transcriptionFellBack: "transcription:fell-back",
  correctionApplied: "dictation:correction-applied",
  processingStarted: "processing:started",
  processingComplete: "processing:complete",
  insertionStarted: "insertion:started",
  insertionComplete: "insertion:complete",
  insertionFailed: "insertion:failed",
  historyChanged: "history:changed",
  settingsChanged: "settings:changed",
  navigate: "navigate",
  modelProgress: "model:progress",
  modelComplete: "model:complete",
  modelFailed: "model:failed",
} as const;

export interface LevelPayload {
  level: number;
}

export interface StartedPayload {
  targetApp: string | null;
}

export interface FailurePayload {
  message: string;
  retryable: boolean;
}

export interface TextPayload {
  text: string;
}

interface EventMap {
  [EVENTS.dictationState]: DictationState;
  [EVENTS.dictationLevel]: LevelPayload;
  [EVENTS.dictationStarted]: StartedPayload;
  [EVENTS.dictationStopped]: null;
  [EVENTS.transcriptionStarted]: null;
  [EVENTS.transcriptionComplete]: TextPayload;
  [EVENTS.transcriptionFailed]: FailurePayload;
  [EVENTS.transcriptionFellBack]: FallbackPayload;
  [EVENTS.correctionApplied]: { count: number };
  [EVENTS.processingStarted]: null;
  [EVENTS.processingComplete]: TextPayload;
  [EVENTS.insertionStarted]: null;
  [EVENTS.insertionComplete]: null;
  [EVENTS.insertionFailed]: FailurePayload;
  [EVENTS.historyChanged]: null;
  [EVENTS.settingsChanged]: null;
  [EVENTS.navigate]: string;
  [EVENTS.modelProgress]: DownloadProgress;
  /** The model id that finished. */
  [EVENTS.modelComplete]: string;
  [EVENTS.modelFailed]: { modelId: string; message: string };
}

export function on<K extends keyof EventMap>(
  name: K,
  handler: (payload: EventMap[K]) => void,
): Promise<UnlistenFn> {
  return listen<EventMap[K]>(name, (event) => handler(event.payload));
}
