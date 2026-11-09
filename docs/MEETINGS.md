# Meeting-jegyzetelő (Windows és macOS)

Ugyanaz a vendég-csatlakozás, mint a Mac appban: Google Chrome, külön profil, csak a távoli hang. A jegyzetek a `Dokumentumok/Business Flow/Meeting-jegyzetek` mappába kerülnek. A `bflow` parancs és az MCP-szerver ebből a `meetings/` mappából fut, API-kulcs nélkül. Az átírást az asztali app végzi a kulcstartóban lévő kulccsal.

Windowsos Claude Desktop konfiguráció: `%APPDATA%\Claude\claude_desktop_config.json`. A `bflow mcp setup claude` ezt használja.

# Meeting-jegyzetelő (1. szakasz)

Automatikus jegyzetelő: a Business Flow figyeli a Google Naptárat, a Google Meet meetingekhez
**vendégként** csatlakozik „Business Flow jegyzetelő” néven, rögzíti a **résztvevők** hangját
(a te mikrofonodat nem), átírja a meglévő átíró folyamattal, összefoglalót készít, és mindent a
`~/Documents/Business Flow/Meeting-jegyzetek` mappába ment. A jegyzeteket a `bflow` CLI és egy
MCP-szerver (Claude Desktop, ChatGPT) is eléri.

## Felépítés

```
Google Naptár (titkos iCal-cím, keys.enc-ben titkosítva)
        │ 5 percenként
        ▼
Business Flow app (Swift) ── MeetingService ──► node meetings/helper/join.mjs  (Playwright + rendszer-Chrome,
   │  UI: Beállítások → Meetingek           │      külön eldobható profil, ablak képernyőn kívül)
   │  - be/ki kapcsoló, név                  │      ◄── STATUS joining|lobby|in-meeting|denied|ended
   │  - közelgő meetingek + Kihagy           │      ──► audio.pcm (16 kHz mono, csak távoli sávok)
   │                                         ▼
   │                      10 perces WAV darabok → AIRouter.transcribe (ugyanaz a modell/tartalék, mint diktálásnál)
   │                      → transcript.md → chatProvider → summary.md → audio.pcm törlése
   ▼
~/Documents/Business Flow/Meeting-jegyzetek/<YYYY-MM-DD HHmm – cím>/{meta.json, transcript.md, summary.md, captions.jsonl, participants.json}
        ▲                         ▲
   .queue/*.json             olvasás
        │                         │
   ┌────┴─────────── meetings/src/core.ts (közös mag, Node/TS) ───────────┐
   │  bflow CLI (dist/cli.js)            MCP-szerver (dist/mcp.js)         │
   └───────────────────────────────────────────────────────────────────────┘
```

- **Közös mag** (`meetings/src/core.ts`): mappaformátum olvasása, keresés, csatlakozási kérés
  sorba állítása. A CLI és az MCP ennek vékony rétegei. A mag **nem kap API-kulcsot**: az
  átírást és összefoglalót kizárólag az app végzi a `keys.enc`-ben tárolt kulcsokkal.
- **App** (`Sources/BNFlow/Meetings.swift`, logika: `Sources/BNFlowCore/Meetings.swift`):
  iCal-értelmezés (ismétlődés: DAILY/WEEKLY/MONTHLY, INTERVAL, COUNT, UNTIL, BYDAY, EXDATE,
  áthelyezett alkalom, lemondott és egész napos események kihagyása), Meet/Teams link-felismerés,
  ütemezés (kezdés előtt 1 perccel), feldolgozás.
- **Csatlakozó segéd** (`meetings/helper/join.mjs`): a `RTCPeerConnection` távoli hangsávjait
  egy 16 kHz-es AudioContextbe keveri, a PCM-et Playwright-bindingen át fájlba írja. Kamera és
  mikrofon kikapcsolva, a „hamis mikrofon” csendfájl, a lap hangja némítva (`--mute-audio`).

## Korlátok (fontos)

- **Beengedés**: vendégként lép be → a szervezőnek/résztvevőnek **be kell engednie** („Ask to join”).
  10 perc várakozás után feladja (`denied`). Ha a meeting csak szervezeten belülieknek vagy csak
  bejelentkezett felhasználóknak engedélyezett, vendég nem léphet be (`error:guest-not-allowed`).
- **Hozzájárulás**: a jegyzetelő látható résztvevőként jelenik meg a nevével. Rögzítés előtt
  **szólj a résztvevőknek** (GDPR / helyi szabályok). A Meet nem jelez magától felvételt.
- **A Mac legyen bekapcsolva**, ébren, az app fusson, internet legyen. Alvó gépen nem csatlakozik.
- Kilépés: ha a meeting véget ér, kidobják, 3 percig egyedül van, vagy a tervezett vége után 15 perccel;
  kézzel: Beállítások → Meetingek → „Jegyzetelő leállítása”.
- Beszélőnevek: a Meet élő feliratából (`captions.jsonl`, a segéd bekapcsolja); a hangalapú átirat egy kevert
  sáv, időbélyeg 10 percenként. Ha nincs hang, a felirat a tartalék átirat. A felirat-szelektorok a Meet DOM-jától függnek.
- **Auto-admit tipp**: saját meetingnél Host controls → Meeting access „Open”, vagy hívd meg előre a jegyzetelőt; különben kézzel kell beengedni.
- A Meet felülete változhat: a gombokat angol szövegük/aria-label alapján keresi (`hl=en`).
  Ha a Google módosít, a `join.mjs` szelektorait kell frissíteni.
- Teams: a linkeket már felismeri és listázza, a csatlakozás a **2. szakasz**. Outlook-naptár is később.
- Költség: nincs új szolgáltatás; csak a meglévő átíró/szöveg API-hívások (kb. a meeting hosszával
  arányos átírási díj a saját kulcson). Csendes darabokat nem küld el.

## Beállítás

1. Segéd telepítése (egyszer):
   ```sh
   cd ~/business-flow-mac/meetings && npm install && npm run build
   ```
   (Rendszer-Google Chrome-ot használ, nem tölt le böngészőt. Más hely: `BNFLOW_MEETINGS_DIR`.)
2. Google Naptár → Beállítások → saját naptár → **„Titkos cím iCal formátumban”** → másold be:
   Business Flow → Beállítások → Meetingek → iCal-cím → Mentés. (Titkosítva a `keys.enc`-be kerül,
   naplóba soha.) Ingyenes, nincs Google-bejelentkezés vagy OAuth.
3. Kapcsold be: „Automatikus csatlakozás a Google Meet meetingekhez”. A listában alkalmanként „Kihagy”.

## `bflow` CLI

```sh
cd ~/business-flow-mac/meetings && npm link     # opcionális: `bflow` parancs a PATH-ba
bflow meetings list [--json]        # jegyzetelt meetingek, legújabb elöl (id = mappanév)
bflow transcript <id|előtag|latest> # átirat
bflow summary <id|előtag|latest>    # összefoglaló
bflow search <kifejezés…> [--json]  # keresés összefoglalókban és átiratokban
bflow actions [--open] [--json]     # teendők az összes összefoglalóból
bflow export <id> [--format md|json]  # meeting egyben, megosztáshoz
bflow join <meet-url> [cím]         # csatlakozási kérés a futó appnak (.queue)
bflow mcp setup claude              # MCP beírása a Claude Desktop konfigba (mentéssel)
```
Más mappa: `BFLOW_NOTES_DIR=/út/vonal bflow …`.

## MCP-szerver

Eszközök: `list_meetings`, `get_transcript`, `get_summary`, `search_notes`, `list_action_items`, `export_meeting`, `join_meeting(url)`.
Az átirat-tartalmú eredmények `_meta.provenance.untrusted_content=true` jelölést kapnak (prompt-injekció elleni védelem, REA-minta).
Kutatás és terv: `docs/MEETINGS_RESEARCH.md`.

### Claude Desktop (helyi, stdio)
`~/Library/Application Support/Claude/claude_desktop_config.json`:
```json
{ "mcpServers": { "business-flow-meetings": {
    "command": "/usr/local/bin/node",
    "args": ["/path/to/business-flow-mac/meetings/dist/mcp.js"] } } }
```
Claude újraindítása után megjelenik.

### ChatGPT (távoli, Streamable HTTP)
```sh
export BFLOW_MCP_TOKEN=$(openssl rand -hex 24)   # kötelező, min. 24 karakter
node dist/mcp.js --http                          # http://127.0.0.1:8787/mcp
```
- Alapból csak `127.0.0.1`-en figyel, minden kérés `Authorization: Bearer <token>` fejlécet kér.
- A `join_meeting` HTTP-n alapból **ki van kapcsolva**; `BFLOW_MCP_ALLOW_JOIN=1` engedi.
- ChatGPT-nek nyilvános HTTPS-cím kell: ingyenes alagút, pl. `cloudflared tunnel --url http://127.0.0.1:8787`,
  majd ChatGPT → Settings → Connectors (Developer mode) → az alagút `/mcp` címe. Ez a jegyzeteidet
  az interneten elérhetővé teszi a token birtokosának – csak akkor futtasd, amikor kell.

## Adatvédelem

- Kulcsok és iCal-cím: csak `keys.enc` (AES-GCM, gépi kulcs). Kódban, naplóban, CLI/MCP-ben nincs.
- A nyers hang (`audio.pcm`) sikeres átírás után törlődik; részleges hibánál megmarad az újrapróbáláshoz.
- A segéd minden futáshoz új, ideiglenes Chrome-profilt használ, és utána törli.
