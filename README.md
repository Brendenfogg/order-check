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

Measured on synthetic test sets of scripted Chick-fil-A orders spoken by 16 macOS voices. Each order comes in two situations (face-to-face, where both voices are heard, and headset, where only the employee's read-back is heard) at three noise levels (clean, 15 dB, and 5 dB SNR, with pink noise plus background chatter). Every order is checked against its correct ticket (any alarm is a false alarm) and against one ticket per kind of planted mistake.

### Tuning set (177 orders, 1,107 checks)

The matching rules and thresholds were tuned on this set, so these numbers are optimistic.

| Model | Caught (all) | False alarms (all) | Caught / false alarms, **clean** | Caught / false alarms, 15 dB | Caught / false alarms, 5 dB | Finalize → result (p95) |
|---|---|---|---|---|---|---|
| Whisper base.en (142 MB) | 68.1% | 20.3% | 86.1% / 5.1% | 73.5% / 18.6% | 44.5% / 37.3% | 276 ms |
| **Whisper small.en (466 MB), default** | 79.4% | 18.6% | 96.1% / 3.4% | 84.8% / 13.6% | 57.1% / 39.0% | 512 ms |
| Whisper large-v3-turbo q5 (574 MB) | 81.5% | 11.3% | 97.1% / 1.7% | 87.4% / 10.2% | 60.0% / 22.0% | 2225 ms |

### Held-out set (69 orders, 405 checks): the honest number

These are 12 new orders written after tuning and never used to change the logic: declined meal and size upsells, "actually, can I also…", a free-item reward, two sizes of one drink, swaps, and big family orders.

| Model | Caught (all) | False alarms (all) | Caught / false alarms, **clean** | Caught / false alarms, 15 dB | Caught / false alarms, 5 dB | Finalize → result (p95) |
|---|---|---|---|---|---|---|
| **Whisper small.en, held-out set** | 70.2% | 30.4% | 81.2% / 34.8% | 75.9% / 43.5% | 53.6% / 13.0% | 617 ms |

**The gap is the finding.** On clean audio the check still catches most planted mistakes (81%), but it raises false alarms on about a third of correct tickets. Part of that is overfitting: phrasings the rules were never taught, like "one large and one small lemonade," "two chocolate shakes, one with no whipped cream," and "a 3-count strip" in the singular. Part is Whisper mishearing ("Chicken Sandwich, Grill with…" for "meal with"). A rule-based extractor can be patched phrase by phrase, but the held-out set shows that approach won't generalize on its own. That's the case for the next step: real recorded orders and a learned (small, local) language model for order extraction.

The full breakdown, including every miss and false alarm with what Whisper heard, is in [docs/results.md](docs/results.md).

**What the numbers say, honestly:**
- On the orders it was tuned on, with clear audio, it catches nearly every planted mistake with very few false alarms, and returns a result in about half a second.
- On new orders it never saw, false alarms jump to about a third of correct tickets. The rules are overfit to the phrasings they were tuned on. This is the most important result here, and it points to the next step.
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
python3 tools/gen_testset.py --out eval/cases --seed 7                    # tuning set, ~3 min
python3 tools/gen_testset.py --set holdout --out eval/holdout --seed 11  # held-out set
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
