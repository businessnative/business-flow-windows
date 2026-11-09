/** Model ids match the Mac app's AIProvider lists. The Rust core is the runtime source of truth. */
export const transcriptionProviders = [
  {
    id: "openai",
    label: "OpenAI",
    models: ["gpt-transcribe", "gpt-4o-mini-transcribe", "gpt-4o-transcribe"],
  },
  {
    id: "gemini",
    label: "Google Gemini",
    models: ["gemini-3.5-flash-lite", "gemini-3.8-flash", "gemini-2.5-flash"],
  },
] as const;

export const cleanupProviders = [
  { id: "openai", label: "OpenAI", models: ["gpt-5.6-luna", "gpt-4.1-mini", "gpt-4.1-nano"] },
  { id: "anthropic", label: "Anthropic Claude", models: ["claude-haiku-5-5", "claude-sonnet-5-5"] },
  { id: "gemini", label: "Google Gemini", models: ["gemini-3.5-flash-lite", "gemini-3.8-flash", "gemini-2.5-flash-lite"] },
] as const;

export const holdKeys = [
  { id: "rightControl", label: "Jobb Ctrl" },
  { id: "leftControl", label: "Bal Ctrl" },
  { id: "rightOption", label: "Jobb Alt" },
  { id: "leftOption", label: "Bal Alt" },
  { id: "rightShift", label: "Jobb Shift" },
  { id: "fn", label: "Fn" },
] as const;

export const toggleCombos = [
  { id: "ctrl-space", label: "Ctrl + Szóköz", modifiers: ["control"] },
  { id: "ctrl-shift-space", label: "Ctrl + Shift + Szóköz", modifiers: ["control", "shift"] },
  { id: "alt-space", label: "Alt + Szóköz", modifiers: ["option"] },
  { id: "fn-space", label: "Fn + Szóköz", modifiers: ["fn"] },
] as const;

export function requiredModelIds(): string[] {
  return [
    "gpt-transcribe",
    "gpt-4o-mini-transcribe",
    "gpt-4o-transcribe",
    "gemini-3.5-flash-lite",
    "gpt-5.6-luna",
    "gpt-4.1-mini",
    "claude-haiku-5-5",
  ];
}
