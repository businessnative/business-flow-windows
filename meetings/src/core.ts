// Közös mag: a jegyzetmappa olvasása és a csatlakozási kérés sorba állítása.
// Ezt használja a `bflow` CLI és az MCP-szerver is; API-kulcsot SOHA nem kezel (az átírást a Mac-app végzi).
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

export const notesDir = () => process.env.BFLOW_NOTES_DIR || path.join(os.homedir(), 'Documents', 'Business Flow', 'Meeting-jegyzetek');
export const queueDir = () => path.join(notesDir(), '.queue');

export interface Meeting {
  id: string; folder: string; title: string; platform: string; url?: string;
  start?: string; end?: string; status: string; audioSeconds?: number;
  hasTranscript: boolean; hasSummary: boolean;
}

const MEET_RE = /^https:\/\/meet\.google\.com\/[a-z]{3}-[a-z]{4}-[a-z]{3}(\?.*)?$/;
const TEAMS_RE = /^https:\/\/teams\.(microsoft|live)\.com\//;

function read(p: string): string | undefined { try { return fs.readFileSync(p, 'utf8'); } catch { return undefined; } }

export function listMeetings(limit = 50): Meeting[] {
  let dirs: string[] = [];
  try { dirs = fs.readdirSync(notesDir(), { withFileTypes: true }).filter((d) => d.isDirectory() && !d.name.startsWith('.')).map((d) => d.name); } catch { return []; }
  const out = dirs.map((folder): Meeting => {
    const dir = path.join(notesDir(), folder);
    let meta: Record<string, unknown> = {};
    try { meta = JSON.parse(read(path.join(dir, 'meta.json')) || '{}'); } catch {}
    return {
      id: folder, folder: dir, title: String(meta.title ?? folder), platform: String(meta.platform ?? 'unknown'),
      url: meta.url as string | undefined, start: meta.start as string | undefined, end: meta.end as string | undefined,
      status: String(meta.status ?? 'unknown'), audioSeconds: meta.audioSeconds as number | undefined,
      hasTranscript: fs.existsSync(path.join(dir, 'transcript.md')), hasSummary: fs.existsSync(path.join(dir, 'summary.md')),
    };
  });
  return out.sort((a, b) => (b.start ?? b.id).localeCompare(a.start ?? a.id)).slice(0, limit);
}

/** id = mappanév; elfogad egyedi előtagot vagy „latest”-et is. */
export function resolve(id: string): Meeting | undefined {
  const all = listMeetings(10_000);
  if (id === 'latest') return all[0];
  if (id.includes('/') || id.includes('..')) return undefined;
  return all.find((m) => m.id === id) ?? (() => { const c = all.filter((m) => m.id.startsWith(id)); return c.length === 1 ? c[0] : undefined; })();
}

export const getTranscript = (id: string) => { const m = resolve(id); return m && read(path.join(m.folder, 'transcript.md')); };
export const getSummary = (id: string) => { const m = resolve(id); return m && read(path.join(m.folder, 'summary.md')); };

export interface Hit { id: string; title: string; file: 'summary' | 'transcript'; line: string }
export function searchNotes(q: string, limit = 30): Hit[] {
  const terms = q.toLowerCase().split(/\s+/).filter(Boolean); const hits: Hit[] = [];
  if (!terms.length) return hits;
  for (const m of listMeetings(10_000)) {
    for (const file of ['summary', 'transcript'] as const) {
      const t = read(path.join(m.folder, `${file}.md`)); if (!t) continue;
      for (const line of t.split('\n')) {
        const l = line.toLowerCase();
        if (terms.every((x) => l.includes(x))) { hits.push({ id: m.id, title: m.title, file, line: line.trim().slice(0, 300) }); if (hits.length >= limit) return hits; }
      }
    }
  }
  return hits;
}

/** Csatlakozási kérés: fájl a .queue mappába; a futó Mac-app 30 mp-en belül felveszi (bekapcsolt meeting-funkció kell). */
export function requestJoin(url: string, title?: string, maxMinutes = 120): { ok: boolean; message: string } {
  if (TEAMS_RE.test(url)) return { ok: false, message: 'Teams csatlakozás a 2. szakaszban jön; most csak Google Meet.' };
  if (!MEET_RE.test(url)) return { ok: false, message: 'Csak https://meet.google.com/xxx-xxxx-xxx link fogadható el.' };
  fs.mkdirSync(queueDir(), { recursive: true });
  const f = path.join(queueDir(), `${Date.now()}.json`);
  fs.writeFileSync(f, JSON.stringify({ url, title: title?.slice(0, 120), maxMinutes: Math.min(Math.max(5, maxMinutes), 240), requestedAt: new Date().toISOString() }));
  return { ok: true, message: 'Kérés sorba állítva; a Business Flow app (bekapcsolt Meetingek funkcióval) fél percen belül csatlakozik vendégként.' };
}

export interface ActionItem { id: string; title: string; start?: string; text: string; done: boolean }
/** Teendők az összes összefoglalóból („- [ ]” / „- [x]” sorok a Teendők szakaszban). */
export function listActionItems(opts: { openOnly?: boolean; limit?: number } = {}): ActionItem[] {
  const out: ActionItem[] = [];
  for (const m of listMeetings(10_000)) {
    const t = read(path.join(m.folder, 'summary.md')); if (!t) continue;
    let inTasks = false;
    for (const line of t.split('\n')) {
      if (/^#{1,6}\s/.test(line)) { inTasks = /teend|action/i.test(line); continue; }
      const mm = inTasks && line.match(/^\s*[-*]\s*\[( |x|X)\]\s*(.+)$/);
      if (mm && !(opts.openOnly && mm[1] !== ' ')) out.push({ id: m.id, title: m.title, start: m.start, text: mm[2].trim(), done: mm[1] !== ' ' });
    }
  }
  return out.slice(0, opts.limit ?? 200);
}

/** Egy meeting egyben: metaadat + összefoglaló + átirat (export / megosztás). */
export function exportMeeting(id: string, format: 'md' | 'json' = 'md'): string | undefined {
  const m = resolve(id); if (!m) return undefined;
  const summary = getSummary(m.id) ?? '', transcript = getTranscript(m.id) ?? '';
  let participants: string[] = [];
  try { participants = JSON.parse(read(path.join(m.folder, 'participants.json')) || '[]'); } catch {}
  if (format === 'json') { const { folder, ...meta } = m; return JSON.stringify({ ...meta, participants, summary, transcript }, null, 2); }
  return [summary || `# ${m.title}`, participants.length ? `\n**Résztvevők:** ${participants.join(', ')}\n` : '', '\n---\n', transcript].join('\n');
}

/** Claude Desktop konfiguráció bővítése (mentéssel); a meglévő szervereket nem bántja. */
export function defaultClaudeConfig(): string {
  if (process.platform === 'win32') return path.join(process.env.APPDATA || os.homedir(), 'Claude', 'claude_desktop_config.json');
  if (process.platform === 'darwin') return path.join(os.homedir(), 'Library', 'Application Support', 'Claude', 'claude_desktop_config.json');
  return path.join(os.homedir(), '.config', 'Claude', 'claude_desktop_config.json');
}

export function claudeDesktopSetup(mcpScript: string, nodePath = process.execPath, cfgPath = defaultClaudeConfig()) {
  let cfg: { mcpServers?: Record<string, unknown> } = {};
  const exists = fs.existsSync(cfgPath);
  if (exists) { cfg = JSON.parse(fs.readFileSync(cfgPath, 'utf8') || '{}'); fs.copyFileSync(cfgPath, `${cfgPath}.bak-${Date.now()}`); }
  cfg.mcpServers = { ...(cfg.mcpServers ?? {}), 'business-flow-meetings': { command: nodePath, args: [mcpScript] } };
  fs.mkdirSync(path.dirname(cfgPath), { recursive: true });
  fs.writeFileSync(cfgPath, JSON.stringify(cfg, null, 2));
  return { cfgPath, backedUp: exists };
}
