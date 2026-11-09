import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./overlay.css";

const bars = document.getElementById("bars") as HTMLElement;
const label = document.getElementById("label") as HTMLElement;
const pill = document.getElementById("pill") as HTMLElement;
for (let i = 0; i < 10; i += 1) {
  bars.append(document.createElement("i"));
}

type Payload = { phase: string; message: string; level: number };

void listen<Payload>("dictation", (event) => {
  const { phase, message, level } = event.payload;
  label.textContent = message || (phase === "recording" ? "Felvétel" : "");
  pill.classList.toggle("error", phase === "error");
  pill.classList.toggle("done", phase === "done");
  pill.classList.toggle("handsfree", phase === "handsfree");
  const stop = document.getElementById("stop") as HTMLButtonElement;
  stop.hidden = phase !== "handsfree" && phase !== "recording";
  stop.onclick = () => void invoke("toggle_dictation");
  const nodes = bars.querySelectorAll("i");
  nodes.forEach((node, index) => {
    const wave = phase === "recording" ? 6 + Math.abs(Math.sin(index + level * 8)) * (8 + level * 14) : 4;
    (node as HTMLElement).style.height = `${wave}px`;
  });
});
