# Claude Switch Desktop

Kleine macOS-App für zwei Claude-Desktop-Profile: **Arbeit** und **Privat**.
Die App startet die lokal installierte Claude-App über das unveränderte `cswitch`-CLI.
Claude Desktop wird separat benötigt. Die erste Version ist für macOS vorgesehen.

## Benutzen

`Claude Switch.app` aus dem DMG in den Programme-Ordner ziehen und öffnen.
Beim ersten Klick auf ein Profil führt ein Dialog durch die Anmeldung.
In Claude mit dem gewünschten Konto anmelden, bei Bedarf über ein privates Browserfenster.
Erst wenn die eigenen Chats sichtbar sind, **Ich bin angemeldet** bestätigen.
Danach öffnet der jeweilige Button das Profil direkt oder fokussiert sein Fenster.
Die App prüft dabei keine Desktop-Kontoidentität und liest keine Anmeldetokens.

Der macOS-Login des CLI schließt andere Claude-Fenster. Laufende Aufgaben vorher beenden.
Zuvor geöffnete, im CLI konfigurierte Profile stellt die Oberfläche nach Abschluss oder
Abbruch wieder her. Ein normales, nicht im CLI erfasstes Claude-Fenster muss selbst erneut
geöffnet werden. Nach einem App-Abbruch wird der Anmeldedialog beim nächsten Start wieder
angeboten. Ein Abbruch des Dialogs meldet das Konto nicht ab.

Bestehende Profile `work` und `personal` bleiben erhalten. Fehlende Profile werden mit
getrennten Datenordnern in `~/.cswitch/profiles/` ergänzt. Die bisherige Konfiguration wird
vor dem ersten Ergänzen als `config.toml.before-desktop` gesichert. Unbekannte TOML-Schlüssel
und Kommentare bleiben erhalten. Bei gemeinsamem Desktop-Speicherort stoppt die App mit
einer Fehlermeldung. `CSWITCH_HOME` wird wie beim CLI unterstützt.
Während der einmaligen Einrichtung die Konfiguration nicht gleichzeitig im CLI bearbeiten:
das unveränderte CLI kennt die Schreibsperre der Oberfläche nicht.

## Entwickeln und bauen

Voraussetzungen: macOS mit Xcode Command Line Tools, aktuelles stabiles Rust und Node.js 22.

```sh
cd desktop
npm ci
npm run desktop:dev
```

```sh
npm run verify
npm run prepare:cli
npm run test:cli
npm run test:sync
cargo test --locked --manifest-path src-tauri/Cargo.toml
npm run desktop:build
node scripts/verify-bundle.mjs
```

Das DMG liegt in `desktop/src-tauri/target/release/bundle/dmg/`.
Der lokale Build ist ad hoc signiert, ohne Apple-Notarisierung. Für externe Verteilung
müssen Apple-Signierung und Notarisierung im Build eingerichtet werden.
`npm run dev` zeigt ausschließlich eine gekennzeichnete Browser-Vorschau ohne Startfunktionen.

## Manuell prüfen

1. Claude Switch öffnen und **Arbeit → Claude öffnen** wählen.
2. Laufende Claude-Aufgaben vorher beenden. Im Dialog **Profil zur Anmeldung öffnen** wählen.
3. Im Claude-Fenster mit dem Arbeitskonto anmelden. Falls nötig den Anmeldelink in einem
   privaten Browserfenster öffnen. Danach in Switch **Ich bin angemeldet** bestätigen.
4. Dasselbe unter **Privat** mit dem privaten Konto durchführen.
5. Beide Profile nacheinander öffnen. In jedem Claude-Fenster das Konto kontrollieren.
6. Einen Startbutton erneut anklicken: Das vorhandene Fenster soll fokussiert werden.
7. Switch schließen und neu starten: Beide Buttons sollen die Profile ohne erneute
   Einrichtung öffnen. Das Schließen von Switch beendet Claude nicht.

Für diese Prüfung ist keine Freigabe der Computersteuerung nötig. Kontologins führt
der Benutzer selbst durch. Falls macOS eine Freigabe für den Start der lokal gebauten,
nicht notarisierten App verlangt, die geltenden Geräte- und Unternehmensregeln beachten.

## CLI mit dem Original synchronisieren

Der Fork ergänzt ausschließlich `desktop/`, dieses Prüfprotokoll und den Desktop-Workflow.
`src/`, `Cargo.toml` und `Cargo.lock` des CLI werden nicht angepasst.
`desktop/upstream.json` enthält den geprüften CLI-Stand. Beim Bauen wird geprüft,
dass die CLI-Quellen exakt diesem Commit entsprechen. Das CLI wird aus diesen Quellen
gebaut und mit der App ausgeliefert. Eine separat installierte CLI-Version wird nicht überschrieben.

Remotes:

```sh
git remote add upstream https://github.com/samrand96/claude-switch.git
```

Nur aus einem sauberen Arbeitsverzeichnis:

```sh
cd desktop
npm run sync:cli
npm run prepare:cli
npm run test:cli
npm run verify
cargo test --locked --manifest-path src-tauri/Cargo.toml
npm run desktop:build
```

Der Sync lädt den Originalbranch und führt ihn per Git-Merge zusammen. Anschließend
aktualisiert er den geprüften Commit in `upstream.json`. Änderungen prüfen, bei Konflikten
den Merge auflösen oder mit `git merge --abort` abbrechen. Nach erfolgreichen Prüfungen
`upstream.json` committen. Eine neue App-Version enthält dann die aktualisierte CLI.
Es werden keine CLI-Updates ungeprüft während der Nutzung heruntergeladen.

Der Workflow **Desktop macOS** baut und prüft Änderungen am Branch `desktop-ui` und
stellt das DMG als GitHub-Actions-Artefakt bereit.

## Herkunft

CLI: [samrand96/claude-switch](https://github.com/samrand96/claude-switch), MIT.
Die ursprüngliche Lizenz liegt im Repository und wird als `CLI-LICENSE` mitgeliefert.
