# Business Flow – design tokenek (businessnative.hu, sötét mód)

Gépi formátum: `tokens.json` (ugyanebben a mappában). Az alapértelmezett téma a `businessnative-dark`.
Készült: 2026-10-28. Wispr kódot és assetet nem tartalmaz. A Wispr-ből csak a viselkedést és az állapotokat vesszük át, a megjelenés teljes egészében BN.

## 0. Fontos megállapítások
1. **A site eleve sötét.** A `:root` alapértéke a sötét paletta. Van `.light-mode` osztály is (webflow CSS, 7890. sor), de a főoldal nem használja. A sötét módot ezért nem kell kitalálni: a weboldal tokenjeit használjuk 1:1-ben.
2. **A fő CSS nincs benne a repóban.** A `crawl.mjs:12` build közben tükrözi a Modulabs Webflow-sablont (`https://modulabs-template.webflow.io`). A tokenek forrása így az élő CSS: `https://www.businessnative.hu/assets/6ad8fc6f-modulabs-template.webflow.shared.8b6296212.css`. A BN saját rétege a repóban van: `motion.mjs`, `assistant-widget.mjs`, `shared.mjs`, `crawl.mjs`, `polish.mjs`, `overrides.mjs`.

## 1. Betűk
| Szerep | Család | Súly | Forrás |
|---|---|---|---|
| Címsor, szöveg, gomb | **Geist Variable** (100–900) | címsor 400, szöveg 400, kiemelt szöveg 500, gomb 500 | webflow CSS, @font-face 2061. sor, fájl: `/assets/0d2a37a8-…_Geist-VariableFont_wght.ttf` |
| Címke, eyebrow | **Geist Mono Variable** | 300, NAGYBETŰ, betűköz 0,75 px | @font-face 2069. sor, `/assets/07efb38e-…_GeistMono-VariableFont_wght.ttf` |
| Wordmark | Geist | 600, betűköz −0,02em | `crawl.mjs:279` |
| ~~Raveo Display 500~~ | csak `.cta-sales` és `.text-sales-card` | – | sablonbetű, a licence nincs igazolva, **nem használjuk** |

- **Licenc:** a Geist és a Geist Mono SIL OFL 1.1 alatt jelenik meg (Vercel, github.com/vercel/geist-font), tehát bele lehet csomagolni a Mac appba. A betűket a hivatalos kiadásból vegyük, ne a site-ról. A site helyben szolgálja ki őket, nem a Google Fonts-ról: a `fonts.googleapis` csak preconnect, betűt nem tölt.
- **Tartalék betűk:** SF Pro, illetve SF Mono (ugyanez a megoldás van a az iOS-os Business Flow design tokenjei fájlban).

**Méretskála** (méret / sorköz / betűköz, px). Asztali értékek, zárójelben a kompakt, mobil értékek (`@media max-width:767px`, 8292. sor):
- H4 32/36/−0,5 (28/32/−1)
- H5 28/32/−0,25 (20/24/−0,5)
- H6 24/28/0 (20/24/−0,25)
- Body-1 20/28 (16/24)
- **Body-2 16/24** (14/20)
- **Body-3 14/20** (12/16)
- Button-1 14/24 (12/20)
- **Button-2 12/16** (10/16)
- **Label-1 12/16, +0,75** (10/12)
- Label-2 10/12, +0,75 (8/10)

A Mac appban a félkövérrel jelölt asztali méreteket használjuk.

## 2. Színek (sötét mód) — webflow CSS `:root`, 2075. sortól
| Token | Érték |
|---|---|
| background-base | **#0a0a0b** |
| background-lift | **#121212**, 80%-os változat: #121212cc |
| background-depth | **#212122** |
| neutral-light-100 (szöveg) | **#f6f3ec** |
| neutral-light-88 (törzsszöveg, strong) | #f6f3ece0 |
| neutral-light-64 (medium szöveg, ikon) | rgba(246,243,236,.64), #0a0a0b-n átlátszatlanul #a19f9b |
| neutral-light-48 (placeholder) | .48 / #7b7a77 |
| neutral-light-32 | .32 / #565553 |
| neutral-light-16 (**border-subtle**) | .16 / #302f2f |
| neutral-light-8 (input és secondary gomb háttér) | .08 / #1d1d1d |
| **accent-primary (levendula)** | **#c0c0e7** |
| accent „eszközök" (világoskék) | **#b9d5ed** (`shared.mjs:133`, `:245–251`, a Tudástár eszköz-blokkja) |
| ui-error | **#c94040** |
| ui-success | **#2ad87f** |
| ui-warning | **#c56a21** |

A világos mód csak referenciaként szerepel: alap #f6f3ec, lift #fcfaf6, depth #e6e0da, contrast #040404.
Eltérés: az iOS-os kiadás hibaszíne #e7a0a0, a site tokenje viszont #c94040. Itt a site tokenjét használom.

## 3. Lekerekítés, térköz, árnyék, blur
- **Lekerekítés:**
  - default, gomb, input és button-circle: **8 px** (`--radius---*`)
  - large: **16 px**
  - full: 100vw (pill)
  - apró elemek: 4 px és 2 px
  - kerek: 50%
- **Térköz-skála:** 4 / 8 / 12 / 16 / 20 / 24 / 32 / 48 / 64 / 80 / 120 px.
- **Ikonok:**
  - méretek: 12 / 16 / 20 (24) px
  - **vonalvastagság 1,5 px**
  - készlet: **Lucide (ISC)** (`platform/components/icons.js`)
- **Árnyékok:**
  - **Hairline bevel**, a BN védjegye: `0 -1px 0 0 light-16, 0 1px 0 0 light-8`. Kártyákon és secondary gombon szerepel.
  - Inset keret: `inset 0 0 0 1px border-subtle`.
  - Panel: `-22px 0 70px rgba(0,0,0,.34)` (`assistant-widget.mjs:175`). Lebegő változata, amit én javaslok: `0 22px 70px rgba(0,0,0,.34)`.
- **Üveghatás:** `backdrop-filter: blur(8px)`, erősebb változatban 16 px (`.card-case-*`).

## 4. Mozgás — `motion.mjs`, `assistant-widget.mjs`, inline CSS
- **Easingek:**
  - **BN fő easing: `cubic-bezier(.22,1,.36,1)`** (`motion.mjs:23`, `EASE_CSS`)
  - lágy: `cubic-bezier(.22,.61,.36,1)`
  - panel: `cubic-bezier(.22,.8,.3,1)` (`assistant-widget.mjs:117, 176`)
- **Időtartamok:**
  - hover-szín 180 ms ease
  - fade 220 ms ease-out
  - border 250 ms
  - panel 300 ms
  - szín 300 ms
  - mikro 350 ms
  - pane 550 ms (power3.out)
  - gombháttér 650 ms
  - reveal 750 ms
  - kép-zoom 900 ms
  - clip 1 s
  - stagger 80 ms (legfeljebb 6 elemig)
  - betűoszlop 13 ms/betű
- **Keyframe-ek:**
  - `spin`: .8s linear infinite (webflow CSS, 2030. sor)
  - `bnAssistantFade`: 0 → 1
  - `bnAssistantSlide`: translateX 100% → 0
  - `rise`: translateY 28 px, opacity 0 → 1
  - nyíl-csúszás: −6 px, a max-width 0-ról 1,4em-re nő
  - `bnm-scroll`: marquee, 42 s
- **GSAP:** a site GSAP 3 + ScrollTrigger kombinációt használ. Framer Motion nincs.
- **Szabályok** (`motion.mjs:21–22`):
  - csak transform és opacity animál
  - nincs default vagy linear easing (kivétel a loader és a marquee)
  - minden reveal csak egyszer fut le
  - reduced-motion beállításnál minden animáció kikapcsol
  - A Mac appban ez a macOS „Mozgás csökkentése" beállítását jelenti.

## 5. Logó és brand assetek
- **Wordmark:** „business" + dot-matrix nyíl + „native" (`crawl.mjs:264–300`). Az SVG viewBox-a `0 0 15 13`, 10 körből áll (r = 1,1, a koordináták a tokens.json-ban vannak), színe currentColor vagy #f6f3ec. A navbarban 21–24 px, a footerben 26 px.
  - **Menüsor-ikonnak** a dot-matrix nyíl önmagában is jó, monokróm template image-ként.
- **Favicon:** a `favicon/` mappában: ico, 16/32/48 px png, apple-touch-icon, 192 és 512 px. Teli fekete négyzet (`scripts/apply-favicon.mjs`, VERSION 4). Ebből készülhet az **app-ikon**.
- **Képi világ:** `bn-assets/bnx-smoke-*.jpg` és `bnx-light-*.jpg` (levendula, teal, sage, violet, amber, peach). Az onboardinghoz jó háttér.

## 6. Flow Bar állapotok → BN tokenek
Közös elemek:
- **Betű:** Geist.
- **Méretváltás:** 350 ms `cubic-bezier(.22,1,.36,1)`.
- **Színváltás:** 180 ms ease.
- **Megjelenés:** bnAssistantFade, 220 ms ease-out.
- **Lekerekítés:** mindenhol **8 px**. Ez a BN gomb-radius; tudatosan nem a Wispr-féle teljes pill.
- **Hullámforma:** 10 sáv, egyenként 2 px széles, 1 px radiusszal, 2 px réssel. A szintváltozás 120 ms, BN easinggel.

| Állapot | Méret (px) | Háttér | Keret | Hullámsáv | Egyéb |
|---|---|---|---|---|---|
| Nyugalmi | 40×8 | neutral-dark-64 (rgba(10,10,11,.64)), blur 8 | 1 px light-16 | – | árnyék nélkül |
| Kész | 50×30 | lift-80 #121212cc, blur 8 | 1 px light-16 | light-48 | hairline bevel |
| Hover | 50×30 | depth #212122 | 1 px light-32 | light-48 | 180 ms ease |
| PTT felvétel | 73×30 | #121212cc | 1 px light-16 | **#c0c0e7** | hairline bevel |
| Hands-free | 102,5×30 | #121212cc | 1 px **#c0c0e7** | #c0c0e7 | Stop gomb: 18×18, #f6f3ec háttér, #0a0a0b négyzet-ikon (Lucide `square`, 8 px), radius 4, hover #f6f3ece0, fókuszgyűrű 2 px #c0c0e7 |
| Feldolgozás | 98×30 | #121212cc | 1 px light-16 | light-48 | Conic gyűrű: #c0c0e7 → átlátszó, `spin` keyframe, **1,6 s** linear (a BN loader 0,8 s; javaslom a kétszeresét) |
| Hiba | 91×30 | #121212cc | **1,5 px #c94040** | #c94040 | szöveg Button-2, #f6f3ec |
| Polish fut | 136×30 | #121212cc | 1 px light-16 | – | gyűrű, mint a feldolgozásnál; Label-2 felirat, nagybetűs, light-64 |
| Polish kész | 152×30 | #121212cc, hover #212122 | 1 px light-16 | – | „Kész · Változások" Button-2 #f6f3ec; pipa-ikon #2ad87f |
| Parancs mód | változó | #121212cc | 1 px **#b9d5ed** | **#b9d5ed** | a BN „eszközök" színe |
| Instruct mód | változó | #121212cc | 1 px **#c56a21** | #c56a21 | ui-warning (lásd a nyitott kérdéseket) |

**Tooltip:**
- Háttér #121212, keret 1 px light-16, radius 8, belső margó 6×12, hairline bevel.
- Szöveg: Body-3, #f6f3ece0.
- Billentyű-jelvény: Label-1 nagybetűvel, light-64, light-8 háttérrel, radius 4.
- Átlátszóság 180 ms ease. Mozgás: translateY 4 px → 0, 350 ms, BN easinggel.

**Pill menü:**
- Háttér rgba(18,18,18,.96), keret 1 px #2b2b31, radius 8, belső margó 4, blur 8.
- Árnyék: `0 22px 70px rgba(0,0,0,.34)`.
- Menüpontok: Button-2 500, radius 4, belső margó 6×10. Hover light-8, aktív light-16. Ikonok 16 px, light-64. Elválasztó: light-8.

**Hub és beállítások ablak:**
- Ablak háttere #0a0a0b. Oldalsáv #121212, a jobb szélén 1 px light-16 vonal.
- Navigációs elem: radius 8. Aktív állapotban light-8 háttér és #f6f3ec szöveg, egyébként light-64.
- Kártya: #121212, 1 px light-16 keret, **radius 16**, belső margó 24, hairline bevel. Hoverkor a keret #c0c0e7, 250 ms ease.
- Gombok (radius 8, belső margó 8×16, Button-1):
  - primary: #f6f3ec háttér, #0a0a0b szöveg, hover #f6f3ece0
  - secondary: light-8 háttér, light-8 keret, bevel; hoverkor krém háttér és sötét szöveg
  - Átmenet: 300 ms.
- Input: light-8 háttér és keret. Aktív állapotban light-16 háttér, light-48 keret és #f6f3ec szöveg. A címke Label-1 nagybetűvel.
- Kapcsoló: kikapcsolva light-16, bekapcsolva #c0c0e7, a gomb #f6f3ec, 180 ms.
- Görgetősáv thumb: #232327, radius 3.
- Mozgás:
  - ablak megjelenése: fade 220 ms
  - lap vagy panel: bnAssistantSlide, 300 ms
  - tartalom: rise, 750 ms, 80 ms staggerrel

## 7. Nyitott kérdések, döntést igénylő pontok
2. **Instruct mód színe:** a BN-ben nincs rózsaszín, ezért a #c56a21 (warning) a javaslatom. Alternatíva az amber vagy a peach a bnx-képekről.
3. **Radius:** 8 px (BN gomb), vagy teljes pill (`radius full`, ami a BN-ben is token)? 8 px-et javaslok, mert az a BN-esebb megoldás.
4. **Spin sebessége:** a BN loader 0,8 s, én 1,6 s-ot javaslok a bar körvonalára.
5. A sötét árnyék lebegő változata (`0 22px 70px`) a panel-árnyékból levezetett javaslat, nem szó szerinti BN token.
