# Business Flow

Diktáló asztali alkalmazás Windows 10/11-re és macOS-re. Lenyomod egy billentyűt, beszélsz, elengeded, és a szöveg bekerül abba a mezőbe, ahol a kurzor áll.

A hang a **te** OpenAI- vagy Gemini-kulcsoddal megy át szöveggé. A tisztítás lehet egy nyelvi modell (OpenAI, Claude vagy Gemini), vagy a beépített, hálózat nélküli magyar szabálykészlet. Nincs szerzői fiók, nincs közvetítő szerver, nincs telemetria, nincs automatikus frissítés. A kulcs a gépen marad, amíg az alkalmazás közvetlenül a szolgáltatót nem hívja.

A logika a nyilvános [Mac-alkalmazás](https://github.com/businessnative/business-flow-mac) platformfüggetlen magjából származik: magyar szövegszabályok, a rövid mondatnál kihagyott modellhívás, a nyomva tartás és a kapcsoló állapotgépe, valamint ugyanazok a modellnevek.

A csomagazonosító helyőrző: `com.example.businessflow`. Publikálás előtt cseréld le: `scripts/set-bundle-id.sh com.sajatszervezet.businessflow`.

## Összehasonlítás a Mac appal

| Funkció | Mac Swift app | Ez a kiadás |
| --- | --- | --- |
| Nyomva tartás és kihangosított diktálás | fn, fn+Szóköz | Kész. Windows alap: jobb Ctrl, Ctrl+Szóköz. Macen fn, ha a billentyű eljut az appig. |
| Parancs mód | fn+Ctrl, kijelölés AX-szel | Kész. Windows: jobb Ctrl+Shift. A kijelölést Ctrl+C / Cmd+C olvassa. |
| Polírozás és szószintű különbség | Ctrl+Cmd+P | Kész. Ctrl+Shift+P, a sávon „Kész · N változás”. |
| Utolsó szöveg újra | Ctrl+Cmd+V | Kész. Ctrl+Shift+V. |
| Okos formázás, helyi magyar motor, rövid mondat LLM nélkül | Igen | Kész, ugyanazok a tesztek. |
| Szótár, részletek, stílus kategóriánként | Hub | Kész. |
| Szótár automatikus tanulása a mezőből | AX olvasás pár másodperc múlva | Egyenértékű: a Kezdőlap/Szótár „javított mondat” mezője ugyanazt a függvényt hívja. A mezőt a rendszer nem adja oda. |
| Előzmény, keresés, szavak, WPM, sorozat | Hub | Kész. |
| Első lépések | 5 lépés | Kész, számláló a Kezdőlapon. A mikrofonengedélyt csak felvételkor lehet biztosan látni. |
| Lebegő sáv, hullám, stop | NSPanel | Kész, átlátszó overlay, nem veszi el a fókuszt. |
| TLS-előmelegítés, hangelőkészítés | Igen | Kész a felvétel indításakor és a felengedéskor. |
| Fizetős előretöltés szünet alatt | Alapból ki | A kapcsoló megvan és alapból ki. A második, megszakítható hívást nem indítjuk, mert a szolgáltató azt is kiszámlázhatja. |
| Kerethiba | Magyar üzenet | Kész: „Elfogyott az átírás kerete. Nézd meg az OpenAI vagy a Gemini számlázását.” |
| Meeting-jegyzetelő, bflow, MCP | Chrome vendég, helyi jegyzet | Kész, ugyanaz a `meetings/` segéd. Kell Node.js, Chrome és `npm install` a meetings mappában. A Teams csatlakozás a Mac appban is későbbi szakasz. |
| Média némítása, szintetizált hangjelzés | Van, a hang alapból ki | Nincs külön némítás: a rendszer hangját nem vesszük el. Hangjelzés nincs, a Mac alapértelmezése is a csend. |
| Kurzor előtti szöveg, jelszómező | Accessibility | Windowson az ablakcím és a programnév alapján kategorizálunk. A mező szövegét nem olvassuk, jelszómezőbe ezért nem írunk bele környezetet. |
| iOS hívás-jegyzet | Csak iPhone | Nem asztali funkció. |

## Követelmények

- Windows 10/11 (64 bites) vagy macOS 11 vagy újabb.
- Mikrofon.
- Windows: a beillesztéshez nem kell külön engedély, kivéve a rendszergazdaként futó ablakokat.
- macOS: mikrofonengedély, és Akadálymentesítés a beillesztéshez (Cmd+V).
- Átíráshoz saját **OpenAI** vagy **Gemini** kulcs. A Claude csak a már leírt szöveget tudja tisztítani, hangot nem.

## Telepítés Windowsra

A CI aláíratlan MSI-t és NSIS-telepítőt készít. Töltsd le a `business-flow-windows` artifactot a GitHub Actions futásából.

1. Nyisd meg a `.msi` vagy a `.exe` telepítőt.
2. A SmartScreen figyelmeztet, mert a fájl nincs aláírva. Kattints a **További információ** (More info) linkre, majd a **Futtatás mindenképp** (Run anyway) gombra.
3. A telepítő az aktuális felhasználóhoz telepít, rendszergazda nélkül.
4. Indítsd el a Business Flow-t. A tálcán a pontmátrix-nyíl az ikonja. Az ablak bezárása a tálcára teszi, nem lép ki.
5. A Szolgáltatók lapon ments el egy OpenAI- vagy Gemini-kulcsot. Az Ellenőrzés gomb a szolgáltató ingyenes modelllistáját hívja, nem számláz diktálást.
6. A Diktálás lapon válaszd ki a mikrofont és a billentyűt. Az alapértelmezés Windowson: **jobb Ctrl** nyomva tartása, és **Ctrl+Szóköz** kapcsoló.
7. Kattints egy szövegmezőbe egy másik programban, tartsd lenyomva a billentyűt, beszélj, engedd el. A szöveg beillesztődik, a korábbi vágólap-tartalom visszakerül.

Ha a SmartScreen ablaka nem kínálja a További információt, a fájl tulajdonságainál jelöld be a **Feloldás** (Unblock) négyzetet, majd indítsd újra.

## Telepítés macOS-re

A CI aláíratlan DMG-t készít (`business-flow-macos`).

1. Nyisd meg a DMG-t, és húzd az alkalmazást az Alkalmazások mappába.
2. Az első indításkor a Gatekeeper megállítja. Ne a sima dupla kattintást használd: **jobb klikk az alkalmazáson → Megnyitás** (Open), majd a párbeszédben ismét Megnyitás.
3. Engedélyezd a **mikrofont**: Rendszerbeállítások → Adatvédelem és biztonság → Mikrofon → Business Flow.
4. Engedélyezd az **Akadálymentesítést** ugyanott, különben a program nem tud Cmd+V-t küldeni a fókuszban lévő mezőbe. Ha a beillesztés nem megy, a szöveg a vágólapon marad, és kézzel beillesztheted.
5. A Szolgáltatók lapon ments el egy OpenAI- vagy Gemini-kulcsot a Kulcskarikába.
6. Alapértelmezett billentyű Macen: **fn** nyomva tartása, **fn+Szóköz** kapcsoló. Ha a gépeden az fn nem jut el az alkalmazásig, válassz a Diktálás lapon Ctrl-t vagy Alt-ot.

A bejelentkezéskor indulás alapból **ki** van kapcsolva mindkét rendszeren.

## Kulcsok

- OpenAI: <https://platform.openai.com/api-keys> — `sk-…` alakú kulcs. Átírásra és tisztításra is jó.
- Gemini: <https://aistudio.google.com/apikey> — `AIza…` alakú kulcs. Átírásra és tisztításra is jó. Az ingyenes keretnek korlátja van, és a Google a feltételei szerint felhasználhatja a kérést.
- Claude: <https://console.anthropic.com/settings/keys> — `sk-ant-…`. Csak tisztítás. Átíráshoz emellett OpenAI- vagy Gemini-kulcs kell.

A kulcs ellenőrzése a szolgáltató modelllistáját kéri le. A diktálás számlázása a lentebbi becslés szerint a beszéd hosszától függ.

Átíró modellek, a Mac-alkalmazással egyezően: `gpt-transcribe` (tartalék `gpt-4o-transcribe`), választható a `gpt-4o-mini-transcribe`; Gemini: `gemini-3.5-flash-lite`. Tisztítás: OpenAI `gpt-5.6-luna` (tartalék `gpt-4.1-mini`), Claude `claude-haiku-5-5`, Gemini `gemini-3.5-flash-lite`. A rövid, egyszerű mondatot a program nem küldi modellnek: a helyi szabályok gyorsabban ugyanezt adják.

## Használat

- **Nyomva tartás:** a billentyű lenyomása indít, a felengedés befejez. A 0,2 másodpercnél rövidebb koppintás nem diktálás.
- **Kapcsoló:** az első lenyomás indít, a második leáll. A sáv Stop gombja is leállít. Esc megszakít.
- **Parancs:** a parancsbillentyű nyomva tartása közben mondd az utasítást. Ha van kijelölés, azt alakítja át; ha nincs, megírja a kért szöveget.
- **Polírozás:** jelöld ki a szöveget, majd Ctrl+Shift+P (Macen a Beállításokban átállítható).
- **Tisztítás ki:** csak a helyi magyar szabályok (vessző, kérdőjel, ékezet, lista, kimondott e-mail, önjavítás).
- **Előzmény:** csak szöveg, csak ezen a gépen, törölhető. A Kezdőlap mutatja a szavakat, a heti szavakat, a percenkénti szószámot és a sorozatot.
- **Meeting:** Meetingek lap, titkos iCal-cím, majd `cd meetings && npm install`. A `bflow` parancs: `cd meetings && npm run build && npm link`. Részletek: `docs/MEETINGS.md`.
- A felvétel legfeljebb 3 perc. A jelző a képernyő alján jelenik meg, és nem veszi el a fókuszt. Ha elfogy a keret, a sáv ezt írja: „Elfogyott az átírás kerete. Nézd meg az OpenAI vagy a Gemini számlázását.”

## Telepítés AI-ügynökkel

Ezt a szakaszt másold be a saját ügynöködnek. Ne kérj tőle szerzői kulcsot, és ne tegyél kulcsot a promptba.

1. Klónozd ezt a tárolót, és a `main` ágon dolgozz, vagy töltsd le a legutóbbi zöld CI artifactot.
2. Ha forrásból építesz: Node.js 22 és stabil Rust kell. Windows-on `npm ci`, majd `npm run tauri build -- --bundles nsis,msi`. macOS-en mindkét cél (`aarch64-apple-darwin`, `x86_64-apple-darwin`) és `npm run tauri build -- --target universal-apple-darwin --bundles dmg`.
3. A telepítő a munkaterület `target/release/bundle/` könyvtárában keletkezik (macOS univerzális buildnél `target/universal-apple-darwin/release/bundle/dmg/`). Mondd meg a felhasználónak a pontos fájlútvonalat.
4. Windowson emlékeztesd a SmartScreen **További információ → Futtatás mindenképp** lépésre. macOS-en a **jobb klikk → Megnyitás** lépésre, majd a mikrofon és az Akadálymentesítés engedélyre.
5. A felhasználó a Szolgáltatók lapon, a saját böngészőjében igényel kulcsot, és beilleszti az alkalmazásba. Az ügynök ne tárolja, ne naplózza, és ne küldje sehova a kulcsot.
6. A `com.example.businessflow` azonosítót a `scripts/set-bundle-id.sh` cseréli, ha a közösség saját nevet akar. Utána újra kell építeni.
7. Ellenőrizd, hogy `cargo test -p bnflow-core` és `npm test` lefut. Ne vegyél fel telemetriát, frissítési URL-t vagy beépített kulcsot.

## Költségek és követelmények

A számok **diktálás** havi becslései, ha naponta ennyit beszélsz, a 2026-os nyilvános listaárak nagyságrendjében. A rövid mondat tisztítása gyakran helyben marad, ezért a tisztítás felára kicsi. A pontos ár a szolgáltató oldalán változik:

- OpenAI: <https://developers.openai.com/api/docs/pricing>
- Gemini: <https://ai.google.dev/gemini-api/docs/pricing>

| Napi beszéd | OpenAI átírás (tisztítással együtt is hasonló) | Gemini fizetős | Gemini ingyenes keret |
| --- | --- | --- | --- |
| 10 perc | kb. 1–1,5 USD | kb. 0,4–0,5 USD | 0 USD, keret és adatfelhasználás mellett |
| 30 perc | kb. 3,3–4,4 USD | kb. 1,1–1,5 USD | 0 USD, hamarabb kifogy a keret |
| 60 perc | kb. 6,5–9 USD | kb. 2,2–3 USD | 0 USD, a keret napi használatra szűk lehet |

Az OpenAI-átírás Claude-tisztítással együtt az OpenAI sorhoz hasonló nagyságrend, mert a tisztítás a beszédhez képest rövid szöveg. A Gemini ingyenes kerete 0 USD, amíg a kvóta bírja; a Google a feltételei szerint felhasználhatja az adatot. A helyi szabályok (tisztítás ki) nem hívnak modellt, az átírás díja ettől még megmarad.

Ugyanez a nagyságrend, ha naponta 30 percet diktálsz (kb. 900 perc havonta) és havonta 10 óra meetinget jegyzetelsz, okos formázással. Forrás: a Mac app 2026. november 9-i táblája, [OpenAI](https://developers.openai.com/api/docs/pricing), [Gemini](https://ai.google.dev/gemini-api/docs/pricing).

| Beállítás | Diktálás | Meetingek | Összesen / hó |
| --- | --- | --- | --- |
| Csak OpenAI (`gpt-transcribe` + `gpt-5.6-luna`) | kb. 4,1 + 0,8 USD | kb. 2,7 USD | kb. 7–8 USD |
| OpenAI átírás + Claude Haiku tisztítás | kb. 4,1 + 0,4 USD | kb. 2,7 USD | kb. 7 USD |
| Csak Gemini flash-lite, fizetős | kb. 1,1 + 1,4 USD | kb. 0,8 USD | kb. 3–4 USD |
| Gemini ingyenes keret | 0 USD | 0 USD | 0 USD, korláttal; a Google felhasználhatja a bemenetet |

Ha a keret elfogy, az app nem próbálkozik tovább csendben: „Elfogyott az átírás kerete. Nézd meg az OpenAI vagy a Gemini számlázását.”

Kell még: saját kulcs, mikrofon, és Windowson a fenti SmartScreen-lépés, macOS-en a jobb klikkes megnyitás plusz a két rendszerengedély. Aláíró tanúsítvány nincs a tárolóban.

## Fejlesztés

```bash
npm ci
cargo test -p bnflow-core
npm test
npm run tauri dev
```

A magyar szövegszabályok tesztjei a Mac-alkalmazás fixture-eit követik (`crates/bnflow-core`). A felület Vite + TypeScript, a natív rész Tauri 2.

A Geist és a Geist Mono betűk a SIL Open Font License 1.1 alatt vannak; a licenc a `src/fonts/OFL.txt` fájlban van. A színek és a pontmátrix-nyíl a `design/` tokenjeiből valók.

## Ismert korlátok

- Az aláíratlan telepítőt a Windows SmartScreen és a macOS Gatekeeper figyelmeztetéssel indítja. Kódaláírás nincs.
- Rendszergazdaként futó ablakba a szimulált Ctrl+V nem jut be. Ilyenkor a szöveg a vágólapon marad.
- A kijelölést és a mezőt nem az akadálymentesítési fa olvassa, hanem a másolás parancs. Ha a célalkalmazás nem másol, a parancs mód kijelölés nélkül ír.
- A kurzor előtti szöveg olvasása Windowson a folyamatnév és az ablakcím alapján kategorizál (például Outlook, Gmail a Chrome-ban). A mező tartalmát ez a kiadás nem olvassa, ezért a mondat közepére illesztett kisbetű nem mindig igazodik.
- A meeting-jegyzetelő külön Node.js-t és telepített Google Chrome-ot kér. A Teams-csatlakozás nincs kész, ahogy a Mac appban sem.
- macOS-en a fókuszált alkalmazás tartalmát ez a kiadás nem olvassa; a beillesztés a parancs+V-n múlik, amihez Akadálymentesítés kell.
- A globális billentyűfigyelő macOS-en szintén Akadálymentesítést kér. Windowson a jobb Ctrl és a Ctrl+Szóköz a gyakorlati alap, mert az fn gyakran nem jut el a programig.
- Nincs automatikus frissítés. Új buildet a CI artifactjából kell telepíteni.
