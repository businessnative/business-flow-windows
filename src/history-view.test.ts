import { describe, expect, it } from "vitest";
import { presentHistory, statusLabel } from "./history-view";

const base = {
  id: "1",
  dateMs: 1,
  text: "",
  appName: "Jegyzettömb",
  durationSeconds: 2,
};

describe("history recovery rows", () => {
  it("shows a successful transcript with copy and paste", () => {
    const view = presentHistory({ ...base, text: "Holnap indulunk.", status: "success" });
    expect(view.title).toBe("Holnap indulunk.");
    expect(view.label).toBe("");
    expect(view.copy).toBe(true);
    expect(view.paste).toBe(true);
    expect(view.retry).toBe(false);
  });

  it("keeps a paste failure copyable", () => {
    const view = presentHistory({
      ...base,
      text: "Szia, ez megvan.",
      status: "pasteFailed",
      reason: "A szöveg megvan, a beillesztés nem sikerült.",
    });
    expect(view.copy).toBe(true);
    expect(view.paste).toBe(true);
    expect(view.retry).toBe(false);
    expect(view.label).toBe("Nincs beillesztve");
    expect(view.tone).toBe("bad");
    expect(view.reason).toContain("beillesztés");
  });

  it("offers retry only when audio remains after a failed or interrupted transcription", () => {
    const failed = presentHistory({ ...base, status: "failed", reason: "Elfogyott az átírás kerete.", audioName: "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee.wav" });
    expect(failed.retry).toBe(true);
    expect(failed.copy).toBe(false);
    expect(failed.title).toContain("kerete");
    const interrupted = presentHistory({ ...base, status: "interrupted", reason: "A kapcsolat megszakadt.", audioName: "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee.wav" });
    expect(interrupted.retry).toBe(true);
    expect(statusLabel("interrupted")).toBe("Megszakadt");
    const empty = presentHistory({ ...base, status: "empty", reason: "Nem érkezett szöveg.", audioName: "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee.wav" });
    expect(empty.retry).toBe(false);
    const noAudio = presentHistory({ ...base, status: "failed", reason: "Az API-kulcs érvénytelen." });
    expect(noAudio.retry).toBe(false);
  });

  it("treats older rows without a status as success", () => {
    const view = presentHistory({ ...base, text: "Régi sor." });
    expect(view.copy).toBe(true);
    expect(view.retry).toBe(false);
    expect(view.label).toBe("");
  });
});
