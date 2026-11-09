# Biztonság

A Business Flow diktáló alkalmazás. A kulcs a tiéd, a gépé, és a választott szolgáltatóé. A programnak nincs szerzői fiókja, proxyja, telemetriája, analitikája és automatikus frissítési csatornája.

- Az API-kulcs a Windows Hitelesítőadat-kezelőjébe vagy a macOS Kulcskarikába kerül (`com.example.businessflow` szolgáltatásnév, amíg nem cseréled le). A beállítások JSON-jában nincs kulcs.
- A hang és a szöveg közvetlenül a választott szolgáltatóhoz megy: `api.openai.com`, `api.anthropic.com`, `generativelanguage.googleapis.com`. Köztes szerver nincs.
- A Claude nem tud átírni. Átíráshoz OpenAI- vagy Gemini-kulcs kell.
- Az előzmény és a beállítás csak a gépen van (`Business Flow` mappa az alkalmazásadat-könyvtárban). Hangfájl nem marad meg.
- A csomagazonosító helyőrző: `com.example.businessflow`. Cseréld a sajátodra a `scripts/set-bundle-id.sh` paranccsal, mielőtt másoknak kiadod.
- Ne commitolj `.env` fájlt, aláíró tanúsítványt, provisioning profilt, hangfelvételt vagy valódi kulcsot.

## Sebezhetőség

Ne nyiss nyilvános hibajegyet kulccsal vagy hanggal. Használd a GitHub **Security → Report a vulnerability** privát bejelentését ebben a tárolóban.
