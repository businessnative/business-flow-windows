import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./styles.css";
import { cleanupProviders, holdKeys, toggleCombos, transcriptionProviders } from "./catalog";

type Shortcut = { type: "hold"; keys: string[] } | { type: "combo"; keyCode: number; modifiers: string[] } | { type: "mouse"; button: number };

interface Settings {
  language: string;
  shortcuts: {
    pushToTalk: Shortcut[];
    handsFree: Shortcut[];
    command: Shortcut[];
    pasteLast: Shortcut[];
    polish: Shortcut[];
  };
  smartFormatting: boolean;
  launchAtLogin: boolean;
  microphoneName: string | null;
  historyEnabled: boolean;
  transcriptionModel: string;
  fallbackTranscriptionModel: string;
  chatModel: string;
  fallbackChatModel: string;
  transcriptionProvider: string;
  cleanupProvider: string;
  geminiTranscriptionModel: string;
  geminiChatModel: string;
  anthropicChatModel: string;
  onboardingDone: boolean;
  dictionary: { id: string; term: string; heardAs: string[]; autoLearned: boolean }[];
  snippets: { id: string; trigger: string; expansion: string }[];
  styles: Record<string, string>;
  meetingsEnabled: boolean;
  meetingBotName: string;
  defaultPolishId: string;
  paidPrefetch: boolean;
}

interface KeyInfo {
  provider: string;
  label: string;
  hasKey: boolean;
  source: string;
  canTranscribe: boolean;
  placeholder: string;
  keyPage: string;
}

interface HistoryItem {
  id: string;
  dateMs: number;
  text: string;
  appName: string;
  durationSeconds: number;
}

interface UiState {
  settings: Settings;
  keys: KeyInfo[];
  mics: string[];
  history: HistoryItem[];
  stats: { totalWords: number; weekWords: number; averageWpm: number; streakDays: number; sessions: number };
  onboardingDoneSteps: number;
  onboardingTotal: number;
  platform: string;
  sttNote: string;
  polishPresets: [string, string][];
}

let state: UiState;

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;

function fillSelect(select: HTMLSelectElement, options: { value: string; label: string }[], current: string) {
  select.replaceChildren();
  for (const option of options) {
    const el = document.createElement("option");
    el.value = option.value;
    el.textContent = option.label;
    select.append(el);
  }
  if (options.some((o) => o.value === current)) select.value = current;
}

function currentHold(): string {
  const first = state.settings.shortcuts.pushToTalk.find((s) => s.type === "hold");
  return first && first.type === "hold" ? first.keys[0] ?? "rightControl" : "rightControl";
}

function currentToggle(): string {
  const combo = state.settings.shortcuts.handsFree.find((s) => s.type === "combo");
  if (!combo || combo.type !== "combo") return "ctrl-space";
  const mods = [...combo.modifiers].sort().join("+");
  const found = toggleCombos.find((c) => [...c.modifiers].sort().join("+") === mods && combo.keyCode === 49);
  return found?.id ?? "ctrl-space";
}

function applyHotkeys() {
  const hold = $<HTMLSelectElement>("hold").value;
  const toggle = toggleCombos.find((c) => c.id === $<HTMLSelectElement>("toggle").value) ?? toggleCombos[0];
  state.settings.shortcuts.pushToTalk = [{ type: "hold", keys: [hold] }];
  state.settings.shortcuts.handsFree = [{ type: "combo", keyCode: 49, modifiers: [...toggle.modifiers] }];
}

function render() {
  const s = state.settings;
  fillSelect($("language"), [
    { value: "hu", label: "Magyar" },
    { value: "en", label: "Angol" },
    { value: "auto", label: "Automatikus (magyar + angol)" },
  ], s.language);
  fillSelect($("mic"), [{ value: "", label: "Rendszer alapértelmezett" }, ...state.mics.map((m) => ({ value: m, label: m }))], s.microphoneName ?? "");
  fillSelect($("hold"), holdKeys.map((k) => ({ value: k.id, label: k.label })), currentHold());
  fillSelect($("toggle"), toggleCombos.map((k) => ({ value: k.id, label: k.label })), currentToggle());
  setSwitch($("cleanup"), s.smartFormatting);
  setSwitch($("login"), s.launchAtLogin);
  setSwitch($("prefetch"), s.paidPrefetch);
  setSwitch($("history-on"), s.historyEnabled);
  $("intro").hidden = s.onboardingDone;

  fillSelect($("stt-provider"), transcriptionProviders.map((p) => ({ value: p.id, label: p.label })), s.transcriptionProvider === "anthropic" ? "openai" : s.transcriptionProvider);
  fillSelect($("clean-provider"), cleanupProviders.map((p) => ({ value: p.id, label: p.label })), s.cleanupProvider);
  renderModels();
  renderKeys();
  renderHistory();
  renderHub();
  renderDictionary();
  renderSnippets();
  renderStyle();
  renderPolish();
  setSwitch($("meet-on"), s.meetingsEnabled);
  $<HTMLInputElement>("meet-name").value = s.meetingBotName || "Business Flow jegyzetelő";
}

function renderHub() {
  $("onboarding-line").textContent = `Első lépések: ${state.onboardingDoneSteps} / ${state.onboardingTotal}`;
  const stats = $("stats");
  stats.replaceChildren();
  for (const [label, value] of [
    ["Szavak", String(state.stats.totalWords)],
    ["Ezen a héten", String(state.stats.weekWords)],
    ["Szó / perc", String(state.stats.averageWpm)],
    ["Sorozat (nap)", String(state.stats.streakDays)],
  ] as const) {
    const card = document.createElement("article");
    card.className = "card";
    card.innerHTML = `<span class="eyebrow">${label}</span><h1>${value}</h1>`;
    stats.append(card);
  }
}

function renderDictionary() {
  const root = $("dict-list");
  root.replaceChildren();
  for (const entry of state.settings.dictionary) {
    const row = document.createElement("article");
    row.className = "item";
    const body = document.createElement("div");
    body.textContent = `${entry.term}${entry.heardAs.length ? " · " + entry.heardAs.join(", ") : ""}`;
    const button = document.createElement("button");
    button.className = "btn secondary";
    button.type = "button";
    button.textContent = "Törlés";
    button.addEventListener("click", () => {
      state.settings.dictionary = state.settings.dictionary.filter((item) => item.id !== entry.id);
      void save("learn-status");
      renderDictionary();
    });
    row.append(body, button);
    root.append(row);
  }
}

function renderSnippets() {
  const root = $("sn-list");
  root.replaceChildren();
  for (const snippet of state.settings.snippets) {
    const row = document.createElement("article");
    row.className = "item";
    const body = document.createElement("div");
    body.textContent = `${snippet.trigger} → ${snippet.expansion}`;
    const button = document.createElement("button");
    button.className = "btn secondary";
    button.type = "button";
    button.textContent = "Törlés";
    button.addEventListener("click", () => {
      state.settings.snippets = state.settings.snippets.filter((item) => item.id !== snippet.id);
      void save("learn-status");
      renderSnippets();
    });
    row.append(body, button);
    root.append(row);
  }
}

function renderStyle() {
  const root = $("style-grid");
  root.replaceChildren();
  const tones = [
    { value: "formal", label: "Formális" },
    { value: "casual", label: "Laza" },
    { value: "veryCasual", label: "Nagyon laza" },
  ];
  for (const [id, label] of [["personal", "Személyes"], ["work", "Munka"], ["email", "E-mail"], ["other", "Egyéb"]] as const) {
    const field = document.createElement("label");
    field.className = "field";
    const span = document.createElement("span");
    span.textContent = label;
    const select = document.createElement("select");
    select.dataset.styleFor = id;
    fillSelect(select, tones, state.settings.styles[id] || "formal");
    field.append(span, select);
    root.append(field);
  }
}

function renderPolish() {
  fillSelect($("polish-preset"), state.polishPresets.map(([value, label]) => ({ value, label })), state.settings.defaultPolishId || "clean");
}

function renderModels() {
  const stt = transcriptionProviders.find((p) => p.id === $<HTMLSelectElement>("stt-provider").value) ?? transcriptionProviders[0];
  const clean = cleanupProviders.find((p) => p.id === $<HTMLSelectElement>("clean-provider").value) ?? cleanupProviders[0];
  const sttCurrent = stt.id === "gemini" ? state.settings.geminiTranscriptionModel : state.settings.transcriptionModel;
  const cleanCurrent = clean.id === "anthropic" ? state.settings.anthropicChatModel : clean.id === "gemini" ? state.settings.geminiChatModel : state.settings.chatModel;
  fillSelect($("stt-model"), stt.models.map((m) => ({ value: m, label: m })), sttCurrent);
  fillSelect($("clean-model"), clean.models.map((m) => ({ value: m, label: m })), cleanCurrent);
  const fallback = stt.id === "openai" ? "Tartalék átíró: gpt-4o-transcribe, ha a választott modell nem elérhető." : "Ha a választott Gemini modell elutasítja a kérést, a gemini-3.5-flash-lite a tartalék.";
  $("model-note").textContent = fallback + " A rövid, tiszta mondatot a helyi szabályok formázzák, modellhívás nélkül.";
}

function renderKeys() {
  const root = $("keys");
  root.replaceChildren();
  for (const key of state.keys) {
    const card = document.createElement("article");
    card.className = "card key-card";
    const header = document.createElement("header");
    const title = document.createElement("strong");
    title.textContent = key.label;
    const pill = document.createElement("span");
    pill.className = key.hasKey ? "pill on" : "pill";
    pill.textContent = key.hasKey ? (key.source === "env" ? "Környezetből" : "Mentve helyben") : "Nincs kulcs";
    header.append(title, pill);
    const field = document.createElement("label");
    field.className = "field";
    const span = document.createElement("span");
    span.textContent = key.canTranscribe ? "Kulcs (átírásra is)" : "Kulcs (csak tisztítás)";
    const input = document.createElement("input");
    input.type = "password";
    input.autocomplete = "off";
    input.placeholder = key.placeholder;
    input.dataset.provider = key.provider;
    field.append(span, input);
    const row = document.createElement("div");
    row.className = "row";
    const save = document.createElement("button");
    save.className = "btn";
    save.type = "button";
    save.textContent = state.platform === "macos" ? "Mentés a Kulcskarikába" : "Mentés a Hitelesítőadat-kezelőbe";
    save.addEventListener("click", () => storeKey(key.provider, input.value, false));
    const test = document.createElement("button");
    test.className = "btn secondary";
    test.type = "button";
    test.textContent = "Ellenőrzés";
    test.addEventListener("click", () => storeKey(key.provider, input.value, true));
    const clear = document.createElement("button");
    clear.className = "btn secondary";
    clear.type = "button";
    clear.textContent = "Törlés";
    clear.addEventListener("click", () => storeKey(key.provider, "", false));
    const link = document.createElement("a");
    link.href = key.keyPage;
    link.target = "_blank";
    link.rel = "noreferrer";
    link.textContent = "Kulcs igénylése";
    link.style.color = "var(--accent)";
    row.append(save, test, clear, link);
    const status = document.createElement("p");
    status.className = "status";
    status.dataset.statusFor = key.provider;
    card.append(header, field, row, status);
    root.append(card);
  }
}

function renderHistory() {
  const root = $("history-list");
  root.replaceChildren();
  if (state.history.length === 0) {
    const empty = document.createElement("p");
    empty.className = "empty";
    empty.textContent = "Még nincs diktálás. Tartsd lenyomva a billentyűt, és beszélj.";
    root.append(empty);
    return;
  }
  for (const item of [...state.history].reverse()) {
    const row = document.createElement("article");
    row.className = "item";
    const body = document.createElement("div");
    const text = document.createElement("p");
    text.textContent = item.text;
    const meta = document.createElement("small");
    const when = new Date(item.dateMs);
    meta.textContent = `${when.toLocaleString("hu-HU")} · ${item.appName || "ismeretlen"} · ${Math.max(1, Math.round(item.durationSeconds))} mp`;
    body.append(text, meta);
    const button = document.createElement("button");
    button.className = "btn secondary";
    button.type = "button";
    button.textContent = "Beillesztés";
    button.addEventListener("click", async () => {
      await invoke("paste_history_item", { id: item.id });
    });
    row.append(body, button);
    root.append(row);
  }
}

function setSwitch(button: HTMLButtonElement, on: boolean) {
  button.setAttribute("aria-checked", on ? "true" : "false");
}

function readSwitch(id: string): boolean {
  return $(id).getAttribute("aria-checked") === "true";
}

function readDictate() {
  state.settings.language = $<HTMLSelectElement>("language").value;
  const mic = $<HTMLSelectElement>("mic").value;
  state.settings.microphoneName = mic || null;
  state.settings.smartFormatting = readSwitch("cleanup");
  state.settings.launchAtLogin = readSwitch("login");
  state.settings.paidPrefetch = readSwitch("prefetch");
  applyHotkeys();
}

function readModels() {
  const stt = $<HTMLSelectElement>("stt-provider").value;
  const sttModel = $<HTMLSelectElement>("stt-model").value;
  const clean = $<HTMLSelectElement>("clean-provider").value;
  const cleanModel = $<HTMLSelectElement>("clean-model").value;
  state.settings.transcriptionProvider = stt;
  state.settings.cleanupProvider = clean;
  if (stt === "gemini") state.settings.geminiTranscriptionModel = sttModel;
  else {
    state.settings.transcriptionModel = sttModel;
    state.settings.fallbackTranscriptionModel = "gpt-4o-transcribe";
  }
  if (clean === "anthropic") state.settings.anthropicChatModel = cleanModel;
  else if (clean === "gemini") state.settings.geminiChatModel = cleanModel;
  else {
    state.settings.chatModel = cleanModel;
    state.settings.fallbackChatModel = cleanModel === "gpt-4.1-mini" ? "gpt-4.1-nano" : "gpt-4.1-mini";
  }
}

async function save(statusId: string) {
  const status = $(statusId);
  try {
    await invoke("save_settings", { settings: state.settings });
    status.classList.remove("bad");
    status.textContent = "Elmentve ezen a gépen.";
  } catch (err) {
    status.classList.add("bad");
    status.textContent = String(err);
  }
}

async function storeKey(provider: string, key: string, alsoTest: boolean) {
  const status = document.querySelector(`[data-status-for="${provider}"]`) as HTMLElement;
  try {
    if (key === "") await invoke("set_api_key", { provider, key: "" });
    else await invoke("set_api_key", { provider, key });
    if (alsoTest) {
      await invoke("test_api_key", { provider, key });
      status.classList.remove("bad");
      status.textContent = "A kulcs érvényes.";
    } else {
      status.classList.remove("bad");
      status.textContent = key ? "Elmentve a rendszer titkos tárolójába." : "A helyben tárolt kulcs törölve.";
    }
    state = await invoke<UiState>("get_state");
    renderKeys();
  } catch (err) {
    status.classList.add("bad");
    status.textContent = String(err);
  }
}

function wire() {
  document.querySelectorAll<HTMLButtonElement>(".nav button").forEach((button) => {
    button.addEventListener("click", () => {
      document.querySelectorAll(".nav button").forEach((b) => b.removeAttribute("aria-current"));
      button.setAttribute("aria-current", "page");
      document.querySelectorAll(".view").forEach((view) => view.classList.remove("active"));
      $(button.dataset.view || "dictate").classList.add("active");
    });
  });
  document.querySelectorAll<HTMLButtonElement>(".switch").forEach((button) => {
    button.addEventListener("click", () => setSwitch(button, button.getAttribute("aria-checked") !== "true"));
  });
  $("save-dictate").addEventListener("click", () => { readDictate(); void save("dictate-status"); });
  $("try-dictate").addEventListener("click", () => void invoke("toggle_dictation"));
  $("save-models").addEventListener("click", () => { readModels(); void save("model-note"); });
  $("stt-provider").addEventListener("change", renderModels);
  $("clean-provider").addEventListener("change", renderModels);
  $("save-history").addEventListener("click", () => {
    state.settings.historyEnabled = readSwitch("history-on");
    void save("history-status");
  });
  $("clear-history").addEventListener("click", async () => {
    const button = $<HTMLButtonElement>("clear-history");
    if (button.dataset.armed !== "1") {
      button.dataset.armed = "1";
      button.textContent = "Biztosan törlöd?";
      return;
    }
    await invoke("clear_history");
    button.dataset.armed = "0";
    button.textContent = "Előzmények törlése";
    state = await invoke<UiState>("get_state");
    renderHistory();
  });
  $("dict-add").addEventListener("click", () => {
    const term = $<HTMLInputElement>("dict-term").value.trim();
    const heard = $<HTMLInputElement>("dict-heard").value.split(",").map((part) => part.trim()).filter(Boolean);
    if (!term) return;
    state.settings.dictionary.unshift({ id: crypto.randomUUID(), term, heardAs: heard, autoLearned: false });
    $<HTMLInputElement>("dict-term").value = "";
    $<HTMLInputElement>("dict-heard").value = "";
    void save("learn-status");
    renderDictionary();
  });
  $("learn").addEventListener("click", async () => {
    const status = $("learn-status");
    try {
      status.textContent = String(await invoke("learn_correction", { later: $<HTMLInputElement>("learn-later").value }));
      status.classList.remove("bad");
      state = await invoke<UiState>("get_state");
      renderDictionary();
    } catch (err) {
      status.classList.add("bad");
      status.textContent = String(err);
    }
  });
  $("sn-add").addEventListener("click", () => {
    const trigger = $<HTMLInputElement>("sn-trigger").value.trim();
    const expansion = $<HTMLInputElement>("sn-expansion").value.trim();
    if (!trigger || !expansion) return;
    state.settings.snippets.unshift({ id: crypto.randomUUID(), trigger, expansion });
    void save("learn-status");
    renderSnippets();
  });
  $("save-style").addEventListener("click", () => {
    document.querySelectorAll<HTMLSelectElement>("[data-style-for]").forEach((select) => {
      const key = select.dataset.styleFor || "other";
      state.settings.styles[key] = select.value;
    });
    void save("learn-status");
  });
  $("polish-save").addEventListener("click", () => {
    state.settings.defaultPolishId = $<HTMLSelectElement>("polish-preset").value;
    void save("learn-status");
  });
  $("polish-now").addEventListener("click", () => void invoke("polish_now"));
  $("meet-save").addEventListener("click", async () => {
    state.settings.meetingsEnabled = readSwitch("meet-on");
    state.settings.meetingBotName = $<HTMLInputElement>("meet-name").value.trim() || "Business Flow jegyzetelő";
    await save("meet-status");
    const ical = $<HTMLInputElement>("meet-ical").value.trim();
    if (ical) await invoke("set_named_secret", { name: "ical-url", value: ical });
  });
  $("meet-preview").addEventListener("click", async () => {
    const list = await invoke<{ title: string; start: number; platform: string; skipped: boolean }[]>("preview_meetings");
    const root = $("meet-list");
    root.replaceChildren();
    for (const item of list) {
      const row = document.createElement("article");
      row.className = "item";
      row.textContent = `${new Date(item.start * 1000).toLocaleString("hu-HU")} · ${item.title} · ${item.platform}${item.skipped ? " · kihagyva" : ""}`;
      root.append(row);
    }
    const notes = await invoke<{ id: string; title: string; hasSummary: boolean }[]>("list_meeting_notes");
    const notesRoot = $("note-list");
    notesRoot.replaceChildren();
    for (const note of notes) {
      const row = document.createElement("article");
      row.className = "item";
      row.textContent = `${note.title}${note.hasSummary ? " · összefoglaló" : ""}`;
      notesRoot.append(row);
    }
  });
  $("meet-join").addEventListener("click", async () => {
    const status = $("meet-status");
    try {
      status.textContent = String(await invoke("request_meeting_join", { url: $<HTMLInputElement>("meet-url").value, title: "" }));
      status.classList.remove("bad");
    } catch (err) {
      status.classList.add("bad");
      status.textContent = String(err);
    }
  });
  $("dismiss-intro").addEventListener("click", () => {
    state.settings.onboardingDone = true;
    readDictate();
    void save("dictate-status");
    $("intro").hidden = true;
  });
}

async function boot() {
  state = await invoke<UiState>("get_state");
  render();
  wire();
  await listen<{ phase: string }>("dictation", async (event) => {
    if (event.payload.phase === "done" || event.payload.phase === "idle") {
      state = await invoke<UiState>("get_state");
      renderHistory();
    }
  });
}

void boot();
