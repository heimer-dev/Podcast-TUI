# podi — Terminal Podcast Player

Ein TUI Podcast Player für das Terminal, gebaut mit Rust + Ratatui.

```
╔═══════════════════════════════════════════════════════════════════════════╗
║  ♪  PodcastTUI                                      2 feed(s) | [?] hilfe║
╠═══════════════════╦═══════════════════════════════════════════════════════╣
║ FEEDS             ║ EPISODES                                              ║
║                   ║                                                       ║
║ ▶ Darknet Diaries ║ ▶ Ep 158: Alone in the Dark   [NEW]  47min  Mar 10   ║
║   Lex Fridman     ║   Ep 157: Synthetic Souls            1h2m   Feb 28   ║
║   [+] Add Feed    ║   Ep 156: Kill Chain                 39min  Feb 14   ║
╠═══════════════════╩═══════════════════════════════════════════════════════╣
║ ▶  Ep 158: Alone in the Dark                                  [1.0x] ♪80%║
║    ████████████░░░░░░░░░░░░░░░░░░░░░  18:42 / 47:13                      ║
╚═══════════════════════════════════════════════════════════════════════════╝
```

## Voraussetzungen

- [mpv](https://mpv.io) — für Audiowiedergabe
- [Rust](https://rustup.rs) — zum Kompilieren (wird automatisch installiert)

## Installation

```bash
git clone https://github.com/yourname/podcast-tui
cd podcast-tui
./install.sh
```

Das Script:
1. Prüft ob `mpv` installiert ist
2. Installiert Rust automatisch falls nicht vorhanden
3. Baut ein optimiertes Release-Binary
4. Installiert es als `podi` nach `~/.local/bin/`

### PATH einrichten (einmalig)

**Fish:**
```fish
fish_add_path ~/.local/bin
```

**bash/zsh:**
```bash
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.bashrc
```

## Starten

```bash
podi
```

## Tastenkürzel

### Navigation

| Taste | Funktion |
|-------|----------|
| `Tab` | Fokus: Feeds ↔ Episoden |
| `j` / `↓` | Liste runter |
| `k` / `↑` | Liste hoch |
| `Enter` | Episode abspielen |
| `?` | Hilfe anzeigen |
| `q` | Beenden |

### Feed-Verwaltung

| Taste | Funktion |
|-------|----------|
| `a` | Feed hinzufügen (RSS-URL eingeben) |
| `d` | Feed löschen |
| `r` | Aktuellen Feed aktualisieren |
| `R` | Alle Feeds aktualisieren |

### Wiedergabe

| Taste | Funktion |
|-------|----------|
| `Space` | Play / Pause |
| `l` / `→` | 10 Sek. vor |
| `h` / `←` | 10 Sek. zurück |
| `L` | 1 Min. vor |
| `H` | 1 Min. zurück |
| `+` | Lautstärke +5% |
| `-` | Lautstärke -5% |
| `>` | Geschwindigkeit +0.25x |
| `<` | Geschwindigkeit -0.25x |
| `]` | Nächstes Kapitel |
| `[` | Voriges Kapitel |

### Download

| Taste | Funktion |
|-------|----------|
| `D` | Episode herunterladen (Offline-Wiedergabe) |

## Konfiguration

Feeds und Einstellungen werden gespeichert unter:

```
~/.config/podcast-tui/config.json
```

Heruntergeladene Episoden landen in:

```
~/Podcasts/
```

## Manuell bauen

```bash
cargo build --release
./target/release/podcast-tui
```

## Deinstallieren

```bash
rm ~/.local/bin/podi
rm -rf ~/.config/podcast-tui   # optional: Feeds löschen
rm -rf ~/Podcasts               # optional: Downloads löschen
```
