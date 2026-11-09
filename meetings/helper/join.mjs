#!/usr/bin/env node
// Business Flow – Google Meet vendég-csatlakozó (1. szakasz).
// Külön, eldobható Chrome-profilban fut (nem a felhasználó böngészője/fiókja), kamera és mikrofon KI,
// csak a távoli résztvevők hangját rögzíti 16 kHz mono s16le PCM-be. Kulcsot nem kap, hálózatra csak a Meet felé megy.
// Kimenet stdout-on: "STATUS <joining|lobby|in-meeting|denied|ended|error:...>".
import { chromium } from 'playwright-core';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

const args = Object.fromEntries(process.argv.slice(2).reduce((a, v, i, arr) => (v.startsWith('--') ? a.concat([[v.slice(2), arr[i + 1]]]) : a), []));
const url = args.url, name = args.name || 'Business Flow jegyzetelő', out = args.out;
const until = Number(args.until || 0) * 1000 || Date.now() + 2 * 3600e3;
const lobbyTimeout = Number(args['lobby-minutes'] || 10) * 60e3;
const aloneTimeout = Number(args['alone-minutes'] || 3) * 60e3;
const show = !!process.env.BNFLOW_SHOW_BROWSER;
if (!url || !/^https:\/\/meet\.google\.com\/[a-z]{3}-[a-z]{4}-[a-z]{3}/.test(url) || !out) { console.log('STATUS error:args'); process.exit(2); }

const status = (s) => console.log(`STATUS ${s}`);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const profile = fs.mkdtempSync(path.join(os.tmpdir(), 'bflow-meet-'));
const silent = path.join(profile, 'silence.wav');
{ // 1 s csend a „hamis mikrofonnak” – ha a némítás nem sikerülne, akkor se szóljon semmi
  const n = 16000, b = Buffer.alloc(44 + n * 2);
  b.write('RIFF', 0); b.writeUInt32LE(36 + n * 2, 4); b.write('WAVEfmt ', 8); b.writeUInt32LE(16, 16); b.writeUInt16LE(1, 20); b.writeUInt16LE(1, 22);
  b.writeUInt32LE(16000, 24); b.writeUInt32LE(32000, 28); b.writeUInt16LE(2, 32); b.writeUInt16LE(16, 34); b.write('data', 36); b.writeUInt32LE(n * 2, 40);
  fs.writeFileSync(silent, b);
}
const sink = fs.createWriteStream(out, { flags: 'a' });
// Feliratok (beszélőnévvel) és résztvevők: a Meet saját élő feliratából, a hangfájl mellé.
const capFile = path.join(path.dirname(out), 'captions.jsonl');
const partFile = path.join(path.dirname(out), 'participants.json');
const capSink = fs.createWriteStream(capFile, { flags: 'a' });
const t0 = Date.now(); const participants = new Set();
let bytes = 0, done = false, ctx;

async function finish(code, st) {
  if (done) return; done = true;
  if (st) status(st);
  try { const p = ctx?.pages()[0]; await p?.locator('[aria-label*="Leave call" i]').first().click({ timeout: 2000 }); } catch {}
  try { await ctx?.close(); } catch {}
  try { await ctx?.pages()[0]?.evaluate(() => window.__bfFlushCaptions?.()); } catch {}
  await new Promise((r) => sink.end(r)); await new Promise((r) => capSink.end(r));
  try { fs.writeFileSync(partFile, JSON.stringify([...participants].sort(), null, 1)); } catch {}
  fs.rmSync(profile, { recursive: true, force: true });
  console.log(`AUDIO_SECONDS ${(bytes / 32000).toFixed(1)}`);
  process.exit(code);
}
process.on('SIGINT', () => finish(0, 'ended'));
process.on('SIGTERM', () => finish(0, 'ended'));

// A böngészőbe injektált rögzítő: minden távoli hangsávot egy 16 kHz-es AudioContextbe kever.
const capture = () => {
  const AC = window.AudioContext; let ac, mix, proc;
  const ensure = () => {
    if (ac) return;
    ac = new AC({ sampleRate: 16000 });
    mix = ac.createGain();
    proc = ac.createScriptProcessor(4096, 1, 1);
    mix.connect(proc); proc.connect(ac.destination);
    proc.onaudioprocess = (e) => {
      const f = e.inputBuffer.getChannelData(0), i16 = new Int16Array(f.length);
      for (let i = 0; i < f.length; i++) { const s = Math.max(-1, Math.min(1, f[i])); i16[i] = s < 0 ? s * 0x8000 : s * 0x7fff; }
      let bin = ''; const u8 = new Uint8Array(i16.buffer);
      for (let i = 0; i < u8.length; i += 0x8000) bin += String.fromCharCode.apply(null, u8.subarray(i, i + 0x8000));
      window.__bfPcm(btoa(bin));
    };
  };
  const seen = new Set();
  const add = (track) => {
    if (track.kind !== 'audio' || seen.has(track.id)) return; seen.add(track.id);
    ensure(); if (ac.state === 'suspended') ac.resume();
    const ms = new MediaStream([track]);
    const el = new Audio(); el.muted = true; el.srcObject = ms; el.play().catch(() => {}); // Chrome: a távoli sáv csak így „folyik”
    ac.createMediaStreamSource(ms).connect(mix);
  };
  const Orig = window.RTCPeerConnection;
  window.RTCPeerConnection = function (...a) {
    const pc = new Orig(...a);
    pc.addEventListener('track', (e) => add(e.track));
    return pc;
  };
  window.RTCPeerConnection.prototype = Orig.prototype;
  Object.setPrototypeOf(window.RTCPeerConnection, Orig);
};

// Feliratfigyelő: a felirat-régió minden blokkja (beszélő + szöveg) folyamatosan bővül; a blokk végleges
// változatát akkor írjuk ki, amikor eltűnik vagy 8 mp-ig nem változik. Csak DOM-olvasás, semmit nem küld.
const captionWatcher = () => {
  const live = new Map(); // elem → {speaker, text, start, last}
  const emit = (v) => { if (v.text.trim()) window.__bfCaption(JSON.stringify({ speaker: v.speaker, text: v.text.trim(), start: v.start, end: v.last })); };
  const scan = () => {
    const region = document.querySelector('[role="region"][aria-label*="aption" i]') || document.querySelector('[jsname="dsyhDe"]');
    const seen = new Set();
    if (region) for (const blk of region.querySelectorAll(':scope > div, :scope > div > div')) {
      const nameEl = blk.querySelector('img + div, [class*="name" i], span[jsname]');
      const speaker = (nameEl?.textContent || '').trim().slice(0, 80);
      let text = (blk.textContent || '').trim();
      if (speaker && text.startsWith(speaker)) text = text.slice(speaker.length).trim();
      if (!speaker || !text || text.length > 4000) continue;
      seen.add(blk);
      const v = live.get(blk);
      if (!v) live.set(blk, { speaker, text, start: Date.now(), last: Date.now() });
      else if (v.text !== text) { v.text = text; v.last = Date.now(); }
    }
    for (const [el, v] of live) if (!seen.has(el) || Date.now() - v.last > 8000 && !el.isConnected) { emit(v); live.delete(el); }
  };
  window.__bfFlushCaptions = () => { for (const v of live.values()) emit(v); live.clear(); };
  setInterval(scan, 700);
};
async function enableCaptions(page) {
  if (await clickFirst(page, ['button[aria-label*="Turn on captions" i]'], 1500)) return true;
  try { await page.keyboard.press('c'); return true; } catch { return false; }
}
async function readParticipants(page) {
  try {
    const names = await page.evaluate(() => [...document.querySelectorAll('[data-participant-id] [data-self-name], [data-participant-id] .notranslate')].map((e) => e.textContent?.trim()).filter(Boolean));
    for (const n of names) if (n && n.length < 80) participants.add(n);
  } catch {}
}

const textOnPage = async (page, re) => { try { return re.test(await page.locator('body').innerText({ timeout: 2000 })); } catch { return false; } };
async function clickFirst(page, sels, timeout = 1500) {
  for (const s of sels) { try { const l = page.locator(s).first(); if (await l.isVisible({ timeout })) { await l.click(); return true; } } catch {} }
  return false;
}

try {
  status('joining');
  ctx = await chromium.launchPersistentContext(profile, {
    channel: process.env.BNFLOW_CHROME_CHANNEL || 'chrome', headless: false,
    viewport: { width: 1280, height: 800 }, locale: 'en-US',
    args: ['--use-fake-ui-for-media-stream', '--use-fake-device-for-media-stream', `--use-file-for-fake-audio-capture=${silent}`,
      '--autoplay-policy=no-user-gesture-required', '--mute-audio', '--lang=en-US', '--no-first-run', '--no-default-browser-check',
      ...(show ? [] : ['--window-position=-3000,-3000', '--window-size=1280,800'])],
    ignoreDefaultArgs: ['--enable-automation'],
  });
  await ctx.exposeBinding('__bfPcm', (_s, b64) => { const b = Buffer.from(b64, 'base64'); bytes += b.length; sink.write(b); });
  await ctx.exposeBinding('__bfCaption', (_s, j) => { try { const c = JSON.parse(j); participants.add(c.speaker);
    capSink.write(JSON.stringify({ speaker: c.speaker, text: c.text, t: Math.max(0, (c.start - t0) / 1000), tEnd: Math.max(0, (c.end - t0) / 1000) }) + '\n'); } catch {} });
  await ctx.addInitScript(capture);
  const page = ctx.pages()[0] || await ctx.newPage();
  await page.goto(url + (url.includes('?') ? '&' : '?') + 'hl=en', { waitUntil: 'domcontentloaded', timeout: 45e3 });
  await sleep(4000);
  await clickFirst(page, ['button:has-text("Got it")', 'button:has-text("Dismiss")', 'button:has-text("Continue without microphone and camera")']);
  if (await textOnPage(page, /You can't join this video call|Check your meeting code|Sign in to join/i)) await finish(1, 'error:guest-not-allowed');
  const nameBox = page.locator('input[aria-label="Your name"], input[placeholder="Your name"], input[type="text"]').first();
  await nameBox.fill(name, { timeout: 20e3 });
  await clickFirst(page, ['[role="button"][aria-label*="Turn off microphone" i]', 'button[aria-label*="Turn off microphone" i]']);
  await clickFirst(page, ['[role="button"][aria-label*="Turn off camera" i]', 'button[aria-label*="Turn off camera" i]']);
  if (!await clickFirst(page, ['button:has-text("Ask to join")', 'button:has-text("Join now")'], 5000)) await finish(1, 'error:no-join-button');
  status('lobby');
  const lobbyStart = Date.now(); let inMeeting = false, aloneSince = 0;
  while (!done) {
    await sleep(3000);
    if (Date.now() > until) await finish(0, 'ended');
    const leave = await page.locator('[aria-label*="Leave call" i]').first().isVisible().catch(() => false);
    if (!inMeeting) {
      if (leave) {
        inMeeting = true; status('in-meeting');
        await sleep(2000); await enableCaptions(page); await page.evaluate(captionWatcher).catch(() => {});
        continue;
      }
      if (await textOnPage(page, /denied your request|You can't join this call|removed from the meeting/i)) await finish(1, 'denied');
      if (Date.now() - lobbyStart > lobbyTimeout) await finish(1, 'denied');
      continue;
    }
    if (Math.random() < 0.1) await readParticipants(page);
    if (!leave || await textOnPage(page, /You left the meeting|The call has ended|You've been removed|removed you from the meeting/i)) await finish(0, 'ended');
    if (await textOnPage(page, /You're the only one here|No one else is here/i)) { aloneSince ||= Date.now(); if (Date.now() - aloneSince > aloneTimeout) await finish(0, 'ended'); }
    else aloneSince = 0;
  }
} catch (e) {
  await finish(1, `error:${String(e?.name || 'exception').slice(0, 40)}`);
}
