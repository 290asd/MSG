# MSG

Tauri-diaesitys (Rust + järjestelmän web view) booru-sivustoille (pohjana Chirmayan BooruSlideshow 10.6, aiemmin Electron). Kuvaus, ominaisuudet ja käyttöohjeet ovat `README.md`:ssä, joten niitä ei toisteta tässä.

## Rakenne
- `src-tauri/` on natiivipuoli (Rust): `main.rs` (ikkuna, komennot), `store.rs` (settings.json), `net.rs` (verkko), `files.rs` (kansiot, taustakuva), `downloads.rs` (lataukset). Sivut ovat kansiossa `frontend/`, ja kaikki alla mainitut `js/`, `css/`, `img/` ja `joi/` ovat siellä.
- `frontend/js/tauri_shim.js` tekee Rust-komennoista saman `window.chrome`- ja `window.appInfo`-rajapinnan, jota sivut käyttävät (`js/chrome_api.js` kokoaa `window.chrome`n). Uusi natiivitoiminto: Rust-komento (`generate_handler!`) ja sen kutsu shimissä. Shim myös korvaa `XMLHttpRequest`in ja `fetch`in (ulkoiset osoitteet kulkevat Rustin kautta) ja ohjaa kuvien ja videoiden `src`-osoitteet `msg-proxy`-skeeman kautta.
- `slideshow.html` (etusivu) ja `personal_list.html` (favorites). Logiikka on `js/`, tyylit `css/app.css`.
- Omat lisäykset (asetukset, pikanäppäimet, lasi) ovat `js/app_settings.js`:ssä. Alkuperäinen laajennuskoodi on `js/mvc` ja `js/objects`.
- Etusivun ja favorites-sivun yhteinen koodi on `js/mvc/media_model.js`, `media_view.js` ja `media_controller.js` (sivujen omat luokat perivät ne). Yksinkertaiset asetukset (kenttä + yksi kontrolli, esim. `includeSafe`) ovat yhdessä taulukossa `js/settings_table.js`: uusi asetus lisätään sinne, ei omia settereitä. Tallennus kulkee `DataLoader.save(avain)`-kutsun kautta, eikä se kirjoita takaisin sitä, mitä juuri luettiin.
- `src-tauri/src/native/` on toinen binääri `MSG-native`: sama sovellus egui-ikkunana ilman web viewiä (`cargo run --bin MSG-native`). Jakaa `settings.json`in ja `store.rs`:n Tauri-version kanssa; sivustojen haku (`sites.rs`), haun tila (`session.rs`), suosikit, lataukset ja pikanäppäimet on portattu JS:stä Rustiin. Realbooru, joi, Refract/lasiefektit, tag-analyysi, kosketustila ja "Find pools in favorites" jätettiin pois. Video toimii libmpv:llä (`video.rs`), joka ladataan ajon aikana (`mpv-2.dll` exe:n viereen, Linuxissa `libmpv.so`, macOS:ssä `libmpv.dylib`). Ilman sitä videopostaus näyttää virheen. eframe käyttää glow-taustaa (libmpv:n render API vaatii OpenGL:n). Testausapu: `MSG_SHOT=<png>` ottaa kuvakaappauksen `MSG_SHOT_AFTER` sekunnin (oletus 12) kuluttua ja sulkee ohjelman, `MSG_SEARCH=<tagit>` hakee käynnistyksessä. CI rakentaa binäärin (`build.yml`, askel "Build MSG-native") ja liittää sen releaseen sellaisenaan (`MSG-native-windows-x64.exe`), ilman libmpv:tä. Tätä CI-ajoa ei ole vielä ajettu.
- `js/vendor/thinking-orbs-engine.js` ja `css/vendor/border-beam.css` on tarkoituksella karsittu siihen, mitä MSG käyttää (otsikossa kerrotaan mitä). Älä palauta poistettuja osia.
- Tauri upottaa koko `frontend/`-kansion binääriin (`build.frontendDist`), joten sinne ei pidä lisätä kehitystiedostoja. Sivujen muutokset näkyvät vasta, kun sovellus käännetään ja käynnistetään uudelleen.
- `joi/` on vendoroitu joi.how (GPL-3.0). Älä muokkaa sitä suoraan: muutokset kuuluvat `joi/msg-changes.patch`-tiedostoon ja `joi/README.md`:hen.
- `js/vendor` ja `css/vendor` ovat kolmansien osapuolten koodia. Niiden lisenssit ovat tiedostojen alussa, ja ne on lueteltu README:ssä ja Settings → About -osiossa.

## Tiedostomuoto
- Lähdetiedostot ovat **CRLF**. Käytä Edit-työkalua. `sed -i` ja bash-heredoc muuttavat rivinvaihdot tai rikkovat lainausmerkit, ja se on jo tapahtunut useasti.
- Jos pitää skriptata muokkaus, kirjoita skripti tiedostoon ja säilytä CRLF.

## Build ja testaus
- Tarvitaan Rust (rustup) ja Windowsissa Visual Studio C++ Build Tools. `npm start` ajaa kehityksessä (debug), `npm run build` tekee pelkän `src-tauri/target/release/MSG.exe`:n ja `npm run tauri build` asennuspaketit. Rakennetaan vain Windowsille (päätös 1.10.2026): Linux- ja Mac-paketteja ei enää tehdä, ja `build.yml` rakentaa vain Windowsin. Linux- ja Mac-koodipolut (esim. libmpv-nimet) ovat koodissa, mutta niitä ei rakenneta eikä tueta.
- **Sulje MSG.exe ennen buildia.** Muuten build epäonnistuu.
- Testaa uudella buildilla käynnistämällä MSG.exe ympäristömuuttujalla `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=<portti>` ja ohjaa sivua CDP:llä (`http://127.0.0.1:<portti>/json`). Käynnistä sovellus lopuksi tavallisesti.
- Verkkotesti: hae jokaiselta sivustolta oikeilla checkbox-klikkauksilla (ei `sitesToSearch`in suoralla asetuksella) ja tarkista, että `#current-image` latautuu. Danbooru (Cloudflare) hylkää selaimeksi tekeytyvän User-Agentin, joten `net.rs` käyttää rehellistä `MSG/<versio>`-tunnistetta. Älä vaihda sitä selain-UA:ksi.
- Asetukset ovat käyttäjän datakansiossa (`%APPDATA%\MSG\settings.json`) ja sisältävät suosikit (~4 Mt). **Varmuuskopioi ennen testiä ja palauta testin jälkeen.** Poista testin luomat kansiot.
- **Clauden desktop-sovellus on Windows-paketti (MSIX): sen kautta ajettujen prosessien AppData-kirjoitukset menevät kopioon `%LOCALAPPDATA%\Packages\Claude_pzs8sxrjxfjjc\LocalCache\Roaming\MSG`.** Claude-istunnon ajama `npm start` tai MSG.exe käyttää siis eri `settings.json`:ia kuin käyttäjän tuplaklikkaama MSG.exe, vaikka polku näyttää samalta. Oikea tiedosto luetaan tai kirjoitetaan vain ajamalla `.cmd` komennolla `Start-Process explorer.exe <skripti>`. Käyttäjän exe on siis ajettava Resurssienhallinnan kautta (`Start-Process explorer.exe <polku>`), muuten se käynnistyy Clauden kopiota vasten. Kopiot yhdistettiin 25.9.2026 (oikea varmuuskopioitu nimellä `settings.before-copy.json`). Sama koskee asset-skeemaa: Clauden kautta ajettu MSG saa 403:n `%APPDATA%\MSG`-kansion tiedostoille (esim. taustakuva), koska polku ohjautuu kopioon. Oikea käytös nähdään vain Resurssienhallinnan kautta ajetulla exellä (`.cmd`, joka asettaa `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS`in ja käynnistää exen).
- Älä muuta käyttäjän oikeita asetuksia (esimerkiksi valittuja sivustoja tai lajittelua) ilman palautusta.

## Turvallisuus
- Virheilmoituksiin ei koskaan pyyntö-URL:ia. Rule34-URL sisältää API-avaimen. Näytä vain sivuston nimi.
- Sivustojen data näytetään tekstinä (`textContent`), ei `innerHTML`illä. Käytä `escapeHtml`ia, jos HTML on pakko.
- Sivuilla ei ole suoraa verkko- tai tiedosto-oikeutta: verkko kulkee Rustin `http_request`in ja `msg-proxy`-skeeman kautta (`net.rs`; Referer, offline-tila ja Range-tuki videolle), paikalliset tiedostot asset-skeeman kautta, jonka sallitut kansiot kasvavat vain valittaessa (`files::allow_dir`). Kuvien ja videoiden pitää kulkea proxyn kautta myös siksi, että Refract piirtää ne canvasille (`crossOrigin`). CSP on molemmilla sivuilla (`ipc:`, `asset:` ja `msg-proxy:` sallittu). Tauri-komennot ovat kaikkien sivun skriptien käytössä, joten pidä sivut omina tiedostoinaan (`script-src 'self'`).
- Lataus- ja taustakuvapolut tarkistetaan (ei polkujen ohitusta), ja vain http(s)-osoitteet hyväksytään.

## Julkaisu
- Versio on `src-tauri/Cargo.toml`:ssa (`tauri.conf.json` ei sisällä versiota). `changelog.txt` on alkuperäisen laajennuksen historia (versioon 10.6 asti). Älä kirjaa sinne omia muutoksia.
- Julkaisu: nosta versio, committaa, tee tagi `vX.Y.Z` ja pushaa se. `.github/workflows/build.yml` rakentaa Windows-paketit `tauri-action`illa ja liittää ne releaseen. Tarkista lopuksi, että releasessa on setup.exe, msi ja `MSG-native-windows-x64.exe`.
- Julkaisupaketti ei sisällä käyttäjän suosikkeja, asetuksia tai API-avaimia. Ne ovat koneen datakansiossa, ja uusi kone alkaa tyhjästä. Ominaisuuksia, jotka ovat oletuksena pois (joi, lajitteluvalikko, Refract, offline-kopiot), pitää kytkeä asetuksista.
- Paikalliset buildit (`src-tauri/target/`, `MSG-*/`, `release/`, `node_modules/`) ovat `.gitignore`ssa.

## Työtapa
- Kerro aina, mitä ei ole testattu.
- Yksi ominaisuus kerrallaan, ja committaa jokainen erikseen. Pysyvät päätökset kirjataan tähän tiedostoon.
