import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs'; import os from 'node:os'; import path from 'node:path';

const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'bflow-test-'));
process.env.BFLOW_NOTES_DIR = dir;
const core = await import('../core.js');

const mk = (name: string, meta: object, sum?: string, tr?: string) => {
  const d = path.join(dir, name); fs.mkdirSync(d);
  fs.writeFileSync(path.join(d, 'meta.json'), JSON.stringify(meta));
  if (sum) fs.writeFileSync(path.join(d, 'summary.md'), sum);
  if (tr) fs.writeFileSync(path.join(d, 'transcript.md'), tr);
};
mk('2026-11-03 0900 – Régi', { title: 'Régi', start: '2026-11-03T08:00:00Z', status: 'done', platform: 'googleMeet' }, '## Teendők\n- [ ] Ajánlat küldése péntekig\n- [x] Kész dolog\n## Nyitott kérdések\n- [ ] nem teendő');
mk('2026-11-04 1000 – Új', { title: 'Új', start: '2026-11-04T09:00:00Z', status: 'done', platform: 'googleMeet' }, undefined, 'Költségvetés jóváhagyva');

test('list newest first', () => { const l = core.listMeetings(); assert.equal(l[0].title, 'Új'); assert.equal(l.length, 2); assert.equal(l[1].hasSummary, true); });
test('resolve prefix/latest, no traversal', () => {
  assert.equal(core.resolve('latest')?.title, 'Új');
  assert.equal(core.resolve('2026-11-03')?.title, 'Régi');
  assert.equal(core.resolve('2026-11'), undefined);
  assert.equal(core.getTranscript('../../etc'), undefined);
});
test('search', () => { assert.equal(core.searchNotes('ajánlat péntekig')[0].file, 'summary'); assert.equal(core.searchNotes('költségvetés')[0].id, '2026-11-04 1000 – Új'); });
test('join validation + queue', () => {
  assert.equal(core.requestJoin('https://evil.example/x').ok, false);
  assert.equal(core.requestJoin('https://teams.microsoft.com/l/meetup-join/x').ok, false);
  assert.equal(core.requestJoin('https://meet.google.com/abc-defg-hij', 'T').ok, true);
  const q = fs.readdirSync(core.queueDir()); assert.equal(q.length, 1);
  assert.equal(JSON.parse(fs.readFileSync(path.join(core.queueDir(), q[0]), 'utf8')).url, 'https://meet.google.com/abc-defg-hij');
  assert.equal(core.listMeetings().length, 2); // a .queue nem meeting
});

test('action items + export + claude setup', () => {
  const a = core.listActionItems(); assert.equal(a.length, 2); assert.equal(core.listActionItems({ openOnly: true }).length, 1);
  assert.ok(core.exportMeeting('latest')!.includes('Költségvetés'));
  assert.equal(JSON.parse(core.exportMeeting('2026-11-03', 'json')!).title, 'Régi');
  const cfg = path.join(dir, 'claude.json'); fs.writeFileSync(cfg, JSON.stringify({ mcpServers: { other: { command: 'x' } } }));
  const r = core.claudeDesktopSetup('/x/mcp.js', '/usr/bin/node', cfg); assert.equal(r.backedUp, true);
  const c = JSON.parse(fs.readFileSync(cfg, 'utf8')); assert.ok(c.mcpServers.other && c.mcpServers['business-flow-meetings']);
});
