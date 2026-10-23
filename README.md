# Order Check

**A second set of ears for the person taking your order.**

Order Check listens while a restaurant employee takes an order, compares what the guest **said** with what the employee **rang in**, and flags the difference before the ticket reaches the kitchen:

```
Said SPICY CHICKEN SANDWICH MEAL, rang CHICKEN SANDWICH MEAL
Said 3 Polynesian, rang 1
```

It targets the small, constant mistakes I saw working at Chick-fil-A: the button next to the right one, spicy vs. regular, 8-count vs. 12-count, the wrong number of sauces, a missed "no pickles."

**AI that keeps jobs, not takes them.** Chains have been testing AI that *replaces* the order-taker. Order Check *backs up* the order-taker. The human still takes the order and delivers the hospitality. The AI never changes a ticket; it only points at the mismatch so the person can fix it.

---

## How it works

```
[clip / mic] --audio--> [Order Check server (Rust)] <--ticket--> [POS]
                           1. speech-to-text (Whisper, on device)
                           2. menu-aware correction ("poly nesian" -> Polynesian)
                           3. spoken order extraction (meals, sizes, counts, modifiers,
                              corrections, read-backs, declined upsells)
                           4. ticket verification -> match / mismatch / couldn't confirm
                           5. anonymous record -> restaurant dashboard
```

- **Listening is tied to the order.** It starts when the employee enters the car (drive-thru) or the guest's name (counter), and stops at Finalize. In v1 the Mac microphone stands in for the wearable clip.
- **Three results, not two.** A confident *mismatch* buzzes the clip. *Couldn't confirm* (unclear audio, or something on the ticket that simply wasn't heard) shows a quiet note and no buzz, so false alarms don't teach employees to ignore the clip. Anything the guest didn't specify, like a size, is never counted as a mistake.
- **All restaurant knowledge lives in one file**, [`menus/chick-fil-a.toml`](menus/chick-fil-a.toml): items, spoken aliases, look-alike families, modifiers, meal rules, and service standards. Another restaurant adopts it by writing its own menu file.
- **Optional service standards.** When turned on, each order gets one true/false per standard ("My pleasure," offered the seasonal item). This is team-level coaching, not individual surveillance.

## Privacy, by design

- Audio is never written to disk. Speech recognition runs **locally** with open-source Whisper, so nothing goes to the cloud.
- Transcripts live in memory only until the order is sent (or canceled, or abandoned for 10 minutes), then they're deleted.
- The only thing kept is an anonymous record per order: an order number, a time, what *kind* of mistake was caught, which menu items, and whether it was fixed. No transcript, no employee identity (a test enforces the table's exact columns).
- No voiceprints. The system never identifies who is speaking by their voice.

## Results

Measured on a synthetic test set: 32 scripted Chick-fil-A orders spoken by 16 different macOS voices, each in two situations (face-to-face, where both voices are heard, and headset, where only the employee's read-back is heard), at three noise levels (clean, 15 dB, and 5 dB SNR, with pink noise plus background chatter). Every order is checked against its correct ticket and against one ticket per planted mistake type: **177 orders, 1,107 checks.**

| Model | Caught (all) | False alarms (all) | Caught / false alarms, **clean** | Caught / false alarms, 15 dB | Caught / false alarms, 5 dB | Finalize → result (p95) |
|---|---|---|---|---|---|---|
| Whisper base.en (142 MB) | 71.2% | 24.3% | 88.1% / 6.8% | 75.5% / 22.0% | 50.0% / 44.1% | 276 ms |
| **Whisper small.en (466 MB), default** | 81.7% | 20.9% | 96.1% / 3.4% | 86.8% / 16.9% | 62.3% / 42.4% | 512 ms |
| Whisper large-v3-turbo q5 (574 MB) | 83.3% | 14.7% | 97.4% / 3.4% | 88.7% / 11.9% | 63.9% / 28.8% | 2225 ms |

Targets from the design: catch ≥ 90%, false alarms ≤ 5%, result ≤ 1 s. **On clean audio, small.en meets all three.** Across all noise levels, none of the models does yet.

The full breakdown, including every miss and false alarm with what Whisper heard, is in [docs/results.md](docs/results.md).

**What the numbers say, honestly:**
- On clear audio, the check catches nearly every planted mistake with very few false alarms, and returns a result in about half a second.
- Noise is the limit. At 15 dB and especially 5 dB (background as loud as the speaker), Whisper mishears menu words often enough that false alarms climb. A clip worn a few inches from the employee's mouth should hear much closer to the "clean" case than a far-field mic, but that's a claim to test with real recordings, not one these numbers prove.
- Headset mode is harder: when the clip only hears the employee's read-back, there's half as much speech to go on.

**Known limitations**
- In headset mode, if the employee mishears the guest and then reads back the same wrong thing they rang in, the tool can't catch it. It still catches fat-fingers (read back "spicy," rang regular).
- Saying the same item twice without "another" or "one more" is treated as a read-back, not a second order.
- The test voices are synthetic. Real recordings in a real restaurant are the next measurement.

## Run it

Requires macOS (Apple Silicon recommended), Homebrew, and about 1 GB of disk for models.

```bash
brew install rustup cmake ffmpeg
export PATH="$(brew --prefix rustup)/bin:$PATH"   # add this line to ~/.zshrc to keep it
rustup default stable
tools/download_models.sh          # Whisper models into models/
cargo build --release
```

**Live demo (POS + dashboard):**

```bash
./target/release/order-check serve --demo-transcript --seed-demo-data
# POS:       http://localhost:7080/pos.html
# Dashboard: http://localhost:7080/dashboard.html
```

Start an order (pick a car or type a name) and the mic starts listening. Speak the order, ring it in, and press **Finalize**. `--demo-transcript` streams what the clip hears to the demo panel (off by default). `--seed-demo-data` fills the dashboard with clearly labeled synthetic history. `--standards` turns on the service-standards check.

**Check one recording:**

```bash
./target/release/order-check check --audio order.wav --ticket ticket.json
```

**Run the evaluation:**

```bash
python3 tools/gen_testset.py --out eval/cases --seed 7     # ~3 min, uses `say` + ffmpeg
./target/release/order-check eval --model models/ggml-small.en.bin \
    --out eval/report-small.en --transcripts eval/transcripts-small.en.json
```

`--transcripts` caches what Whisper heard, so changes to the matching logic can be re-scored in seconds.

**Add a real recorded order to the test set:** record with QuickTime, convert with `ffmpeg -i rec.m4a -ar 16000 -ac 1 eval/cases/<id>/audio.wav`, and write `eval/cases/<id>/case.json` with the correct ticket plus any mistaken tickets (see any generated case for the format).

## Code map

| Path | What it does |
|---|---|
| `src/normalize.rs` | Transcript text → tokens (numbers, "8-piece" → `8 count`, "Strip's" → strips) |
| `src/matcher.rs` | Finds menu items, sizes, counts, modifiers, and cues like "actually" or "would you like," with fuzzy correction |
| `src/extract.rs` | Builds the spoken order: meals, corrections, removals, read-backs, declined upsells |
| `src/verify.rs` | Compares spoken order with ticket; writes the "Said X, rang Y" messages |
| `src/stt.rs`, `src/audio.rs` | Whisper via whisper.cpp (Metal GPU), WAV loading, pause-based utterance splitting |
| `src/session.rs`, `src/mic.rs`, `src/server.rs` | Live orders, mic capture only during an order, local HTTP/WebSocket API |
| `src/records.rs` | Anonymous order records and dashboard aggregates (SQLite) |
| `src/eval.rs`, `tools/` | Evaluation harness and synthetic test-set generator |
| `web/pos.html`, `web/dashboard.html` | Mock POS (modeled on the Chick-fil-A screen) and restaurant dashboard |

Built in Rust for speed: once the order ends, the check itself takes under a millisecond. The time that remains is Whisper transcribing the last few seconds of speech; earlier speech is transcribed while the order is still being taken.

## Next steps

- Record real orders (friends acting as guest and employee) and re-run the evaluation on them
- Fine-tune Whisper on those recordings
- ESP32 clip prototype: mic + vibration motor, streaming audio to the in-restaurant server over Wi-Fi
- Real "food ready" buzz from the kitchen expo screen
- POS integration research (Toast partner APIs)
- Employee-level coaching view and restaurants with no digital POS (the server builds the ticket)

---

*Personal portfolio project. Not affiliated with Chick-fil-A or Toast. The mock POS uses no brand logos.*
