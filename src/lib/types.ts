/**
 * The shapes Rust sends over the Tauri bridge.
 *
 * These mirror the `serde` output of the corresponding Rust types. The
 * dictation state in particular is a discriminated union rather than a bag of
 * booleans, so the UI cannot render a contradictory combination.
 */

export type DictationState =
  | { kind: "idle" }
  | { kind: "capturing" }
  | { kind: "finalizingAudio" }
  | { kind: "transcribing"; attempt: number }
  | { kind: "processing" }
  | { kind: "inserting" }
  | { kind: "complete"; transcript: string; method: InsertionMethod }
  | { kind: "captureFailed"; message: string }
  | { kind: "transcriptionFailed"; message: string; retryable: boolean }
  | { kind: "processingFailed"; message: string; transcript: string }
  | {
      kind: "insertionFailed";
      message: string;
      transcript: string;
      onClipboard: boolean;
    };

export type DictationStateKind = DictationState["kind"];

export type InsertionMethod = "accessibility" | "typed" | "clipboardPaste";

export type DictationBehavior = "hold" | "toggle";

export type ProcessingMode = "verbatim" | "polished" | "rewrite";

export type VisualIntensity = "reduced" | "normal" | "high";

export type PermissionStatus =
  | "notDetermined"
  | "granted"
  | "denied"
  | "restricted";

export interface PermissionSnapshot {
  microphone: PermissionStatus;
  accessibility: PermissionStatus;
  speechRecognition: PermissionStatus;
}

export type FallbackPolicy = "off" | "localOnly";

export type RefineStyle = "tidy" | "written";

export type TechnicalVocabulary = "auto" | "always" | "off";

export interface AppSettings {
  fallback: FallbackPolicy;
  refineStyle: RefineStyle;
  refineEngines: string[];
  refineModel: string | null;
  spokenPunctuation: boolean;
  technicalVocabulary: TechnicalVocabulary;
  formatTechnicalTerms: boolean;
  liveTyping: boolean;
  shortcut: string;
  behavior: DictationBehavior;
  mode: ProcessingMode;
  providerId: string;
  modelId: string;
  language: string | null;
  visualIntensity: VisualIntensity;
  onboardingComplete: boolean;
}

export interface SystemStatus {
  permissions: PermissionSnapshot;
  settings: AppSettings;
  registeredShortcut: string | null;
  shortcutRegistered: boolean;
  providerName: string;
  modelName: string;
  /** The selected engine can run the selected model right now. */
  providerReady: boolean;
  /** The selected engine can take a vocabulary hint (local Whisper). */
  providerPrompting: boolean;
  /** The selected engine can recognise speech as it arrives. */
  providerStreaming: boolean;
  adHocBuild: boolean;
  ready: boolean;
}

export interface Capabilities {
  local: boolean;
  batch: boolean;
  streaming: boolean;
  timestamps: boolean;
  wordTimestamps: boolean;
  diarization: boolean;
  languageDetection: boolean;
  translation: boolean;
  prompting: boolean;
}

export interface ModelInfo {
  id: string;
  name: string;
  description: string;
  speed: "fast" | "balanced" | "thorough";
  quality: "good" | "high" | "veryHigh";
  multilingual: boolean;
}

export interface ProviderDescriptor {
  id: string;
  name: string;
  capabilities: Capabilities;
  models: ModelInfo[];
  defaultModel: string;
}

export interface ProviderStatus {
  id: string;
  name: string;
  /** The engine has at least one model it can run right now. */
  ready: boolean;
  modelId: string;
  modelName: string;
  selected: boolean;
}

export type TranscriptSource = "dictation" | "import";

export interface Transcript {
  id: string;
  text: string;
  /** Unix milliseconds. */
  createdAt: number;
  source: TranscriptSource;
  sourceApp: string | null;
}

export interface HistoryQuery {
  search?: string;
  source?: TranscriptSource;
  sourceApp?: string;
  since?: number;
  until?: number;
  limit?: number;
  offset?: number;
}

/** True while the transaction is mid-flight and cannot be restarted. */
export function isBusy(state: DictationState): boolean {
  return (
    state.kind === "capturing" ||
    state.kind === "finalizingAudio" ||
    state.kind === "transcribing" ||
    state.kind === "processing" ||
    state.kind === "inserting"
  );
}

/** True once the transaction has settled, successfully or not. */
export function isFailure(state: DictationState): boolean {
  return state.kind.endsWith("Failed");
}

/** The transcript a state is holding, if any. */
export function transcriptOf(state: DictationState): string | null {
  switch (state.kind) {
    case "complete":
    case "processingFailed":
    case "insertionFailed":
      return state.transcript;
    default:
      return null;
  }
}

/* --- Models page ---------------------------------------------------------- */

export type Fit = "great" | "good" | "tight" | "tooLarge";

/** Derived from this Mac's measured hardware and the model's declared class. */
export interface Rating {
  accuracy: number;
  speed: number;
  overall: number;
  fit: Fit;
  requiredMemoryBytes: number;
}

export interface ModelFile {
  name: string;
  url: string;
  bytes: number;
  sha256: string | null;
}

export interface ModelStatus {
  id: string;
  name: string;
  engine: "whisper" | "parakeet";
  description: string;
  speed: ModelInfo["speed"];
  quality: ModelInfo["quality"];
  multilingual: boolean;
  files: ModelFile[];
  installed: boolean;
  bytesOnDisk: number;
  downloadBytes: number;
  sizeLabel: string;
  rating: Rating;
}

export interface Hardware {
  chip: string;
  totalMemoryBytes: number;
  performanceCores: number;
  appleSilicon: boolean;
}

export interface ModelsPage {
  models: ModelStatus[];
  providers: ProviderDescriptor[];
  hardware: Hardware;
  memoryLabel: string;
  selectedProvider: string;
  selectedModel: string;
}

export interface DownloadProgress {
  modelId: string;
  receivedBytes: number;
  totalBytes: number;
  fraction: number;
}

export const FIT_LABEL: Record<Fit, string> = {
  great: "Runs great here",
  good: "Runs well here",
  tight: "Will be slow here",
  tooLarge: "Not enough memory",
};

/** Counts over real transcripts. Nothing here is estimated. */
export interface Usage {
  totalTranscripts: number;
  transcriptsThisWeek: number;
  wordsThisWeek: number;
  appsThisWeek: number;
  dayStreak: number;
}

/** Emitted when a substitute engine served the transcription. Never silent. */
export interface FallbackPayload {
  failedProvider: string;
  usedProvider: string;
  usedModel: string;
}

/** A text-refinement engine, with live availability. */
export interface RefinerDescriptor {
  id: string;
  name: string;
  description: string;
  local: boolean;
  available: boolean;
  unavailableReason: string | null;
  /** Models the user can choose between. Empty when the engine has one. */
  models: string[];
}

/** Who this build is, and where to go with it. */
export interface About {
  version: string;
  commit: string;
  buildDate: string;
  repository: string;
  website: string;
  issues: string;
  license: string;
  tauriVersion: string;
}

/** The newest signed release, checked by the native app at most once a day. */
export interface UpdateStatus {
  currentVersion: string;
  latestVersion: string | null;
  updateAvailable: boolean;
  /** Unix milliseconds. */
  checkedAt: number | null;
}
