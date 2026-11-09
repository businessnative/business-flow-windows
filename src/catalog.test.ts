import { cleanupProviders, requiredModelIds, transcriptionProviders } from "./catalog";
import { describe, expect, it } from "vitest";

describe("provider catalog", () => {
  it("keeps the Mac app model names", () => {
    const names = [
      ...transcriptionProviders.flatMap((p) => p.models),
      ...cleanupProviders.flatMap((p) => p.models),
    ];
    for (const id of requiredModelIds()) {
      expect(names).toContain(id);
    }
    expect(transcriptionProviders.map((p) => p.id)).toEqual(["openai", "gemini"]);
    expect(cleanupProviders.map((p) => p.id)).toEqual(["openai", "anthropic", "gemini"]);
    expect(cleanupProviders.find((p) => p.id === "anthropic")?.models[0]).toBe("claude-haiku-5-5");
    expect(transcriptionProviders.find((p) => p.id === "openai")?.models[0]).toBe("gpt-transcribe");
  });

  it("does not offer Claude for transcription", () => {
    const ids: string[] = transcriptionProviders.map((p) => p.id);
    expect(ids.includes("anthropic")).toBe(false);
  });
});
