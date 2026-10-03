# MSG

Natiivi Windows-diaesitys (Rust + egui) booru-sivustoille (pohjana Chirmayan BooruSlideshow 10.6). Kuvaus, ominaisuudet ja käyttöohjeet ovat `README.md`:ssä, joten niitä ei toisteta tässä.

Vanhat versiot ovat vain git-historiassa, eikä niitä kehitetä: Electron (tagi `electron-final`) ja Tauri-versio web viewillä ja lasiefekteillä (tagi `webview-final`). Jatkossa kehitetään vain tätä Rust-versiota, ja vain Windowsille (päätös 1.10.2026). Älä palauta web view -koodia (`frontend/`, Tauri, joi, Refract).

## Rakenne
- `src/main.rs` (ikkuna), `app.rs` (sovelluksen tila ja näkymät), `settings_ui.rs` (asetukset), `hotkeys.rs`, `session.rs` (haun tila), `sites.rs` (sivustojen haku ja jäsentimet), `slide.rs`, `favorites.rs`, `downloads.rs`, `files.rs` (kansiot), `media.rs` (kuvat), `video.rs` (libmpv), `net.rs` (verkko), `store.rs` (settings.json).
- Sivustojen haku, haun tila, suosikit, lataukset ja pikanäppäimet on portattu selainlaajennuksen JavaScriptistä Rustiin. Realbooru, joi, Refract/lasiefektit, tag-analyysi, kosketustila ja "Find pools in favorites" jätettiin pois.
- Video toimii libmpv:llä (`video.rs`), joka ladataan ajon aikana (`mpv-2.dll` exe:n viereen). Ilman sitä videopostaus näyttää virheen. eframe käyttää glow-taustaa, koska libmpv:n render API vaatii OpenGL:n.
- **Terminaaliversio `msg-cli`** (päätös 2.10.2026): toinen binääri, jonka crate root on `src/cli.rs` (tila, näppäimet ja piirto) ja kuvat ovat `src/term.rs`:ssä. Se käyttää ikkunan moduuleja (`session`, `sites`, `favorites`, `downloads`, `hotkeys`, `store` …) ja samaa settings.json:ia. Alussa mukana on vain e621 ja vain kuvat. Kuvat piirretään Sixelillä, jos terminaali kertoo osaavansa sen (vastaus kyselyyn `ESC [ c` sisältää 4). Tunnistus ei voi perustua `WT_SESSION`-muuttujaan, koska tuplaklikattu exe avautuu Windows Terminaliin ilman sitä. Sixel-tilassa virtuaalisolu on 10×20 px; kuva piirretään 60 px:n kaistaleina, joista jokainen on oma sixel-kuvansa ja jolla on oma 256 värin paletti, koska WT värittää koko kuvan sillä paletilla, joka on voimassa kuvan lopussa; tämä testattiin 3.10.2026) ja muualla puolilohkoilla. **Jaetut moduulit eivät saa riippua eguista**: `Engine.wake` korvaa `egui::Context`in. `cli.rs`:ssä on omat `mod`-rivit, joten jaettu moduuli lisätään molempiin crate rootteihin (`main.rs`, `cli.rs`).
- `img/` sisältää logot ja kuvakkeet (ikkunan kuvake upotetaan binääriin `main.rs`:ssä).
- `settings.json` (`%APPDATA%\MSG`) ja `store.rs` ovat samat kuin vanhoissa versioissa, joten vanhat asetukset ja suosikit toimivat.
- Testausapu: `MSG_SHOT=<png>` ottaa kuvakaappauksen `MSG_SHOT_AFTER` sekunnin (oletus 12) kuluttua ja sulkee ohjelman, `MSG_SEARCH=<tagit>` hakee käynnistyksessä. `MSG_SCRIPT="18:shot=a,19:next,21:prev,25:shot=b,27:quit"` tekee toiminnot annettuina sekunteina (`next`, `prev`, `shot=<nimi>`, `quit`) ja tallentaa kuvat kansioon `MSG_SHOT_DIR`. Näin voi toistaa esim. videon ja kuvan vaihdot. Video tarvitsee `mpv-2.dll`:n tai `libmpv-2.dll`:n exe:n viereen (kopioi `target/debug` tai `target/release`).

## Tiedostomuoto
- Lähdetiedostot ovat **CRLF**. Käytä Edit-työkalua. `sed -i` ja bash-heredoc muuttavat rivinvaihdot tai rikkovat lainausmerkit, ja se on jo tapahtunut useasti.
- Jos pitää skriptata muokkaus, kirjoita skripti tiedostoon ja säilytä CRLF.

## Build ja testaus
- Tarvitaan Rust (rustup) ja Visual Studio C++ Build Tools. `cargo run --release` ajaa, `cargo build --release` tekee `target/release/MSG.exe`:n, `cargo test` ajaa testit.
- **Sulje MSG.exe ennen buildia.** Muuten build epäonnistuu. `cargo build --release --bin msg-cli` kääntää pelkän terminaaliversion.
- msg-cli:n testi ilman oikeita asetuksia: aja se Windows Terminalissa komennolla `cmd /c "set APPDATA=<tyhjä kansio>&& set MSG_SCRIPT=12:quit&& msg-cli.exe <tagit>"` ja ota näytöstä kuvakaappaus (PowerShellin `CopyFromScreen`). `MSG_SCRIPT` hyväksyy `next`, `prev`, `quit` tai minkä tahansa pikanäppäintoiminnon (esim. `toggleTags`).
- Käynnistä uusi build `MSG_SEARCH`- ja `MSG_SHOT`-muuttujilla ja katso kuvakaappaus. Verkkotesti: hae jokaiselta sivustolta ja tarkista, että kuva latautuu. Danbooru (Cloudflare) hylkää selaimeksi tekeytyvän User-Agentin, joten `net.rs` käyttää rehellistä `MSG/<versio>`-tunnistetta. Älä vaihda sitä selain-UA:ksi.
- Asetukset ovat käyttäjän datakansiossa (`%APPDATA%\MSG\settings.json`) ja sisältävät suosikit (~4 Mt). **Varmuuskopioi ennen testiä ja palauta testin jälkeen.** Poista testin luomat kansiot.
- **Clauden desktop-sovellus on Windows-paketti (MSIX): sen kautta ajettujen prosessien AppData-kirjoitukset menevät kopioon `%LOCALAPPDATA%\Packages\Claude_pzs8sxrjxfjjc\LocalCache\Roaming\MSG`.** Claude-istunnon ajama MSG.exe käyttää siis eri `settings.json`:ia kuin käyttäjän tuplaklikkaama MSG.exe, vaikka polku näyttää samalta. Oikea tiedosto luetaan tai kirjoitetaan vain ajamalla skripti komennolla `Start-Process explorer.exe <skripti>`, ja käyttäjän exe on ajettava Resurssienhallinnan kautta, muuten se käynnistyy Clauden kopiota vasten. Kopiot yhdistettiin 25.9.2026 (oikea varmuuskopioitu nimellä `settings.before-copy.json`).
- Älä muuta käyttäjän oikeita asetuksia (esimerkiksi valittuja sivustoja tai lajittelua) ilman palautusta.

## Turvallisuus
- Virheilmoituksiin ei koskaan pyyntö-URL:ia. Rule34-URL sisältää API-avaimen. Näytä vain sivuston nimi.
- Lataus- ja taustakuvapolut tarkistetaan (ei polkujen ohitusta, `files::download_path`), ja vain http(s)-osoitteet hyväksytään.

## Julkaisu
- Versio on `Cargo.toml`:ssa. `changelog.txt` on alkuperäisen laajennuksen historia (versioon 10.6 asti). Älä kirjaa sinne omia muutoksia.
- Julkaisu: nosta versio, committaa, tee tagi `vX.Y.Z` ja pushaa se. `.github/workflows/build.yml` rakentaa Windows-exen ja liittää sen releaseen (`MSG-windows-x64.exe`, ilman libmpv:tä). Tarkista lopuksi, että releasessa on exe.
- Julkaisu ei sisällä käyttäjän suosikkeja, asetuksia tai API-avaimia. Ne ovat koneen datakansiossa, ja uusi kone alkaa tyhjästä.
- Paikalliset buildit (`target/`) ovat `.gitignore`ssa.

## Työtapa
- Kerro aina, mitä ei ole testattu.
- Yksi ominaisuus kerrallaan, ja committaa jokainen erikseen. Pysyvät päätökset kirjataan tähän tiedostoon.
