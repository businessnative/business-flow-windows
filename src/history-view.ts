export type HistoryStatus = "success" | "failed" | "empty" | "interrupted" | "pasteFailed";

export interface HistoryRow {
  id: string;
  dateMs: number;
  text: string;
  appName: string;
  durationSeconds: number;
  kind?: string;
  status?: HistoryStatus;
  reason?: string;
  audioName?: string | null;
}

export interface HistoryPresentation {
  title: string;
  reason: string;
  label: string;
  tone: "" | "bad" | "warn";
  copy: boolean;
  paste: boolean;
  retry: boolean;
  kind: string;
}

export function presentHistory(item: HistoryRow): HistoryPresentation {
  const status = item.status ?? "success";
  const text = item.text.trim();
  const reason = (item.reason ?? "").trim();
  const title = text || reason || "Nincs szöveg ebből a diktálásból.";
  return {
    title,
    reason: text && reason ? reason : "",
    label: statusLabel(status),
    tone: status === "failed" || status === "pasteFailed" ? "bad" : status === "empty" || status === "interrupted" ? "warn" : "",
    copy: text.length > 0,
    paste: text.length > 0,
    retry: Boolean(item.audioName) && (status === "failed" || status === "interrupted"),
    kind: item.kind === "command" ? "parancs" : item.kind === "polish" ? "polírozás" : "",
  };
}

export function statusLabel(status: HistoryStatus): string {
  switch (status) {
    case "failed":
      return "Sikertelen";
    case "empty":
      return "Üres";
    case "interrupted":
      return "Megszakadt";
    case "pasteFailed":
      return "Nincs beillesztve";
    default:
      return "";
  }
}
