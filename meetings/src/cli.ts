#!/usr/bin/env node
import { listMeetings, getTranscript, getSummary, searchNotes, requestJoin, notesDir, listActionItems, exportMeeting, claudeDesktopSetup } from './core.js';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const [cmd, ...rest] = process.argv.slice(2);
const json = rest.includes('--json'); const args = rest.filter((a) => a !== '--json');
const print = (v: unknown) => console.log(typeof v === 'string' ? v : JSON.stringify(v, null, 2));
const usage = `bflow – Business Flow meeting-jegyzetek
  bflow meetings list [--json]     jegyzetelt meetingek (legújabb elöl)
  bflow transcript <id|latest>     átirat (Markdown)
  bflow summary <id|latest>        összefoglaló (Markdown)
  bflow search <kifejezés…>        keresés az összefoglalókban és átiratokban
  bflow actions [--open] [--json]  teendők az összes meetingből
  bflow export <id> [--format md|json]  meeting egyben (megosztáshoz)
  bflow join <meet-url> [cím]      csatlakozási kérés a futó appnak
  bflow mcp setup claude           MCP regisztrálása a Claude Desktopba (mentéssel)
  Mappa: ${notesDir()}`;

function fail(m: string): never { console.error(m); process.exit(1); }
switch (cmd) {
  case 'meetings': case 'list': {
    const ms = listMeetings();
    if (json) print(ms); else if (!ms.length) print('Nincs még jegyzet.');
    else for (const m of ms) print(`${m.id}\t${m.status}${m.hasSummary ? '\t[összefoglaló]' : ''}`);
    break;
  }
  case 'transcript': print(getTranscript(args[0] ?? 'latest') ?? fail('Nincs ilyen átirat.')); break;
  case 'summary': print(getSummary(args[0] ?? 'latest') ?? fail('Nincs ilyen összefoglaló.')); break;
  case 'search': { const h = searchNotes(args.join(' ')); if (json) print(h); else for (const x of h) print(`${x.id} [${x.file}] ${x.line}`); if (!h.length) process.exitCode = 1; break; }
  case 'actions': { const a = listActionItems({ openOnly: rest.includes('--open') }); if (json) print(a); else for (const x of a) print(`${x.done ? '[x]' : '[ ]'} ${x.text}  — ${x.id}`); break; }
  case 'export': { const fi = args.indexOf('--format'); const fmt = fi >= 0 && args[fi + 1] === 'json' ? 'json' : 'md';
    print(exportMeeting(args[0] ?? 'latest', fmt) ?? fail('Nincs ilyen meeting.')); break; }
  case 'mcp': {
    if (args[0] !== 'setup' || (args[1] ?? 'claude') !== 'claude') fail('Használat: bflow mcp setup claude');
    const script = path.join(path.dirname(fileURLToPath(import.meta.url)), 'mcp.js');
    const r = claudeDesktopSetup(script); print(`Kész: ${r.cfgPath}${r.backedUp ? ' (előző mentve .bak-*)' : ''}. Indítsd újra a Claude Desktopot.`); break;
  }
  case 'join': { const r = requestJoin(args[0] ?? '', args.slice(1).join(' ') || undefined); (r.ok ? print : fail)(r.message); break; }
  default: print(usage); if (cmd && cmd !== 'help' && cmd !== '--help') process.exitCode = 1;
}
