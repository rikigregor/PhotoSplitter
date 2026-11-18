# PhotoSplitter — Foto delič

Windows aplikácia na rozdelenie naskenovaných rodinných fotografií. Spracuje obrázky aj viacstranové PDF a uloží jednotlivé fotky do JPEG súborov.

## Funkcie

- Automatické vyhľadanie fotiek na bielom podklade skenera.
- Priebežné náhľady: každá dokončená fotka sa zobrazí počas spracovania ďalších.
- Automatický odhad otočenia podľa tvárí, s označením neistých výsledkov.
- Ručné otáčanie v galérii so zachovaním úprav pri dopĺňaní náhľadov.
- Uloženie do nového priečinka, so zachovaním originálov.
- Rozšírený editor výrezov v prehliadači a export ZIP.
- Lokálne spracovanie bez odosielania fotografií na internet.

## Zostavenie pre Windows x64

Potrebuješ stabilný Rust a Visual Studio C++ Build Tools s Windows SDK.

```powershell
cargo test --locked
cargo build --release --locked
```

Alebo spusti `ZOSTAVIT.bat`. Výsledný program je `target/release/photo-splitter.exe`. Okno sa otvorí po dvojkliku na EXE. Spracovanie obrazu používa Rust; grafické okno je vytvorené pomocou Windows WPF a vstavaného Windows PowerShellu.

### Podpora PDF

Program používa Poppler: `pdftoppm` a `pdfinfo`. Oba nástroje a príslušné DLL musia byť dostupné v `PATH` alebo v priečinku `poppler` vedľa EXE. Ak distribúcia Poppleru obsahuje priečinok `share`, zachovaj aj jeho potrebné dáta. Binárne súbory Poppleru nie sú súčasťou tohto zdrojového repozitára.

Voliteľne možno nastaviť úplnú cestu k `pdftoppm` cez premennú `PDFTOPPM`; program očakáva `pdfinfo` v rovnakom priečinku.

## Použitie

1. Vyber obrázok alebo PDF a klikni na **Rozdeliť fotky**.
2. Náhľady sa dopĺňajú priebežne. Už hotové fotky môžeš prezerať a otáčať.
3. Po dokončení klikni na **Uložiť všetky fotky** a vyber cieľový priečinok.

Pri nepresných výrezoch použi **Upraviť výrezy**. Tento rozšírený editor sa otvorí samostatne v prehliadači; jeho zmeny sa exportujú tlačidlom ZIP a neprenášajú sa späť do okna.

### Príkazový riadok

```powershell
photo-splitter.exe "sken.pdf" --out "novy-priecinok" --dpi 300
photo-splitter.exe "sken.pdf" --plan "plan.json" --out "export" --dpi 300
photo-splitter.exe "sken.jpg" --out "kontrola" --no-open
```

Výstupný priečinok musí byť nový alebo prázdny. Plán musí patriť k tomu istému vstupu a rozlíšeniu; jeho ručné otočenia majú prednosť pred automatickým odhadom. Prepínač `--prepare-ui` je interné rozhranie medzi oknom a spracovaním: vytvára JPEG náhľady, značky `.ready.json` a `progress.json`.

## Obmedzenia

Automatické výrezy fungujú najlepšie s oddelenými fotografiami na bielom podklade. Otočenie sa odhaduje podľa tvárí; fotky bez tvárí alebo s ležiacimi ľuďmi môžu vyžadovať ručnú opravu. PDF sa štandardne renderuje pri 300 DPI; rozlíšenie možno zvýšiť cez `--dpi`. PDF s heslom nie je podporované. Veľké dokumenty môžu potrebovať veľa pamäte. Dočasné náhľady zostávajú v systémovom dočasnom priečinku.

## Testy

`cargo test --locked` overuje detekciu výrezov, otáčanie, neistú orientáciu prázdneho obrázka a spúšťanie pomocných procesov bez konzoly. Voliteľný test všetkých štyroch orientácií vyžaduje vlastný správne otočený portrét:

```powershell
$env:FOTO_ORIENTATION_TEST_IMAGE = "C:\Fotky\portret.jpg"
cargo test --release all_four_rotations_of_local_portrait -- --ignored
```

Osobné fotky ani testovacie skeny nie sú v repozitári.

## Model a závislosti

Model rozpoznávania tvárí je vložený do EXE zo súboru `src/face-model.bin`. Pochádza z projektu [RustFace](https://github.com/atomashpolskiy/rustface), portu SeetaFace. Príslušná licencia je v `LICENSE-rustface.txt`. [Poppler](https://poppler.freedesktop.org/) sa dodáva samostatne a má vlastné licenčné podmienky.

## Jazyk aplikácie / Application language

Predvolený jazyk je angličtina. Pri slovenskom používateľskom locale Windows (`sk`, napríklad `sk-SK`) sa automaticky použije slovenčina. Rovnaký výber platí pre okno, hlásenia spracovania a editor v prehliadači. Systémové dialógy Windows používajú vlastné jazykové nastavenie systému. Ostatné locale vrátane češtiny používajú angličtinu.

English is the default. A Slovak Windows user locale automatically selects Slovak for the desktop UI, processing messages, and browser editor. For testing or an explicit override, set `PHOTOSPLITTER_LANG=en` or `PHOTOSPLITTER_LANG=sk` before starting the application. No environment variable is needed for automatic selection.

Preklady sú v `src/localization.json`. Overenie okna v oboch jazykoch: `powershell -STA -File tests/localization-ui.ps1`.
