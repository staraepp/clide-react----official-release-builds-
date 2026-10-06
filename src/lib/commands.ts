/**
 * Typed wrappers for every Tauri command.
 *
 * The UI never calls `invoke` directly: keeping the surface in one file means
 * the contract with Rust is reviewable in a single place, and a renamed
 * command breaks the build rather than a button.
 */
import { invoke } from "@tauri-apps/api/core";
import type {
  About,
  PermissionStatus,
  FallbackPolicy,
  RefineStyle,
  TechnicalVocabulary,
  RefinerDescriptor,
  ModelsPage,
  Usage,
  AppSettings,
  DictationBehavior,
  DictationState,
  DictionaryEntry,
  DictionaryPage,
  DictionaryQuery,
  HistoryQuery,
  LocalApiStatus,
  PermissionSnapshot,
  ProcessingMode,
  ProviderDescriptor,
  ProviderStatus,
  SystemStatus,
  Transcript,
  VisualIntensity,
  UpdateStatus,
} from "./types";

// --- dictation -------------------------------------------------------------

export const startDictation = () => invoke<void>("start_dictation");
export const stopDictation = () => invoke<void>("stop_dictation");
export const cancelDictation = () => invoke<void>("cancel_dictation");
export const retryDictation = () => invoke<void>("retry_dictation");
export const beginTranscriptDrag = () => invoke<void>("begin_transcript_drag");
export const dismissDictation = () => invoke<void>("dismiss_dictation");
export const getDictationState = () =>
  invoke<DictationState>("get_dictation_state");
export const testInsertion = () => invoke<string>("test_insertion");

// --- permissions -----------------------------------------------------------

export const getPermissions = () =>
  invoke<PermissionSnapshot>("get_permissions");
export const requestMicrophonePermission = () =>
  invoke<PermissionStatus>("request_microphone_permission");
export const requestAccessibilityPermission = () =>
  invoke<PermissionStatus>("request_accessibility_permission");
export const repairAccessibilityPermission = () =>
  invoke<PermissionStatus>("repair_accessibility_permission");
export const openAccessibilitySettings = () =>
  invoke<void>("open_accessibility_settings");
export const openMicrophoneSettings = () =>
  invoke<void>("open_microphone_settings");

// --- providers -------------------------------------------------------------

export const listProviders = () =>
  invoke<ProviderDescriptor[]>("list_providers");
export const getProviderStatus = () =>
  invoke<ProviderStatus[]>("get_provider_status");

export const selectProvider = (providerId: string, modelId?: string) =>
  invoke<void>("select_provider", { providerId, modelId: modelId ?? null });

// --- history ---------------------------------------------------------------

export const getHistory = (query?: HistoryQuery) =>
  invoke<Transcript[]>("get_history", { query: query ?? null });

export const searchHistory = (search: string, limit?: number) =>
  invoke<Transcript[]>("search_history", { search, limit: limit ?? null });

export const deleteTranscript = (id: string) =>
  invoke<boolean>("delete_transcript", { id });

export const getSourceApps = () => invoke<string[]>("get_source_apps");

export const copyText = (text: string) => invoke<void>("copy_text", { text });

// --- settings --------------------------------------------------------------

export const getSettings = () => invoke<AppSettings>("get_settings");
export const getSystemStatus = () => invoke<SystemStatus>("get_system_status");

export const setShortcut = (accelerator: string) =>
  invoke<void>("set_shortcut", { accelerator });

export const setDictationBehavior = (behavior: DictationBehavior) =>
  invoke<void>("set_dictation_behavior", { behavior });

export const setProcessingMode = (mode: ProcessingMode) =>
  invoke<void>("set_processing_mode", { mode });

export const setVisualIntensity = (intensity: VisualIntensity) =>
  invoke<void>("set_visual_intensity", { intensity });

export const setLanguage = (language: string | null) =>
  invoke<void>("set_language", { language });

export const completeOnboarding = () => invoke<void>("complete_onboarding");
export const resetOnboarding = () => invoke<void>("reset_onboarding");

/** Tauri rejects with a plain string; normalise it for display. */
export function errorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return "Something went wrong.";
}

/* --- Models --------------------------------------------------------------- */

/** Providers, models, and this Mac's hardware in one round trip. */
export const getModelsPage = () => invoke<ModelsPage>("get_models_page");

/**
 * Start a download. Returns as soon as it is under way — progress arrives as
 * `model:progress` and settles on `model:complete` or `model:failed`.
 */
export const downloadModel = (modelId: string) =>
  invoke<void>("download_model", { modelId });

export const removeModel = (modelId: string) =>
  invoke<void>("remove_model", { modelId });

export const getUsage = () => invoke<Usage>("get_usage");

export const setFallbackPolicy = (fallback: FallbackPolicy) =>
  invoke<void>("set_fallback_policy", { fallback });

export const listRefiners = () => invoke<RefinerDescriptor[]>("list_refiners");

export const setRefineStyle = (style: RefineStyle) =>
  invoke<void>("set_refine_style", { style });

export const getAbout = () => invoke<About>("get_about");
/** Download, verify and install the newer version, then relaunch. */
export const installUpdate = () => invoke<void>("install_update");

export const checkForUpdates = (force = false) =>
  invoke<UpdateStatus>("check_for_updates", { force });

/** Apple Speech only. Separate from the microphone, even though it is on-device. */
export const requestSpeechPermission = () =>
  invoke<PermissionStatus>("request_speech_permission");

/** Switch a refinement engine on or off. Cloud engines send text off the Mac. */
export const setRefineEngineEnabled = (engineId: string, enabled: boolean) =>
  invoke<void>("set_refine_engine_enabled", { engineId, enabled });

/** Spoken "comma" and "new line" become punctuation. Applies in every mode. */
export const setSpokenPunctuation = (enabled: boolean) =>
  invoke<void>("set_spoken_punctuation", { enabled });

export const setTechnicalVocabulary = (setting: TechnicalVocabulary) =>
  invoke<void>("set_technical_vocabulary", { setting });

export const setRefineModel = (model: string | null) =>
  invoke<void>("set_refine_model", { model });

export const setLocalApiEnabled = (enabled: boolean) =>
  invoke<void>("set_local_api_enabled", { enabled });
export const setLocalApiPort = (port: number) =>
  invoke<void>("set_local_api_port", { port });
export const setLocalApiOrigins = (origins: string[]) =>
  invoke<void>("set_local_api_origins", { origins });
export const setLocalApiEndpoints = (
  transcription: boolean,
  eventsEnabled: boolean,
) => invoke<void>("set_local_api_endpoints", { transcription, eventsEnabled });
export const getLocalApiStatus = () =>
  invoke<LocalApiStatus>("get_local_api_status");
export const getLocalApiToken = () => invoke<string>("get_local_api_token");
export const regenerateLocalApiToken = () =>
  invoke<string>("regenerate_local_api_token");

export const setLiveTyping = (enabled: boolean) =>
  invoke<void>("set_live_typing", { enabled });

export const setFormatTechnicalTerms = (enabled: boolean) =>
  invoke<void>("set_format_technical_terms", { enabled });

export const setLowerAudioWhileDictating = (enabled: boolean) =>
  invoke<void>("set_lower_audio_while_dictating", { enabled });

// --- dictionary ------------------------------------------------------------

export const setDictionaryAutoLearn = (enabled: boolean) =>
  invoke<void>("set_dictionary_auto_learn", { enabled });

export const getDictionary = (query?: DictionaryQuery) =>
  invoke<DictionaryPage>("get_dictionary", { query: query ?? null });

/** Add a word, or turn a learned one into your own. Rejects with a reason. */
export const addDictionaryWord = (word: string) =>
  invoke<DictionaryEntry>("add_dictionary_word", { word });

export const removeDictionaryWord = (word: string) =>
  invoke<boolean>("remove_dictionary_word", { word });

/** Forget every learned word. Words you added stay. */
export const clearLearnedWords = () => invoke<number>("clear_learned_words");
