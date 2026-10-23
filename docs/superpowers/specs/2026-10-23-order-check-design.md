# Order Check: Software Design (v1)

**Date:** 2026-10-23
**Owner:** Brenden Fogg
**Status:** Draft for approval

---

## 1. Intent and why

Order Check is the software behind a wearable clip that listens while a restaurant employee takes an order. It compares what the customer **said** with what the employee **rang in**, and flags any mismatch **before the ticket reaches the kitchen**.

**Positioning (Brenden's):** AI that *keeps* jobs instead of taking them. The human still takes the order and delivers the hospitality. The AI acts as a second set of ears that closes the gap left by human error. Large chains have tested AI that *replaces* the order-taker. This product *backs up* the order-taker.

**Why it exists:** It's a portfolio project for LinkedIn, aimed at **operations / process-improvement, business consulting, and data analytics** internships. The audience cares about measurable improvement in a restaurant's numbers. The project has to show (a) a real, working system and (b) measured results.

**Origin:** Brenden's time working at Chick-fil-A. v1 is built around Chick-fil-A's menu and ordering flow, but the design has to carry over to any restaurant.

## 2. The mistakes it catches (from Brenden's experience)

The target is small, frequent **near-miss** mistakes, not wholesale wrong orders:

- **Look-alike items:** spicy vs. regular chicken sandwich; 8 ct vs. 12 ct nuggets; grilled vs. breaded
- **Wrong counts:** especially sauces ("3 Polynesian" rung as 1)
- **Fat-fingering:** tapping the button next to the right one
- **Sizes:** medium vs. large drinks or fries
- **Modifiers** *(in v1, Brenden's decision)*: "no pickles", "add cheese", "no ice", "extra lemon", "sub mac & cheese"
- **Missing or extra items**

## 3. Goals

1. **Live speech-to-text on the device, open source, never in the cloud.** Audio never leaves the restaurant.
2. **Menu-aware transcription:** the system is biased toward the restaurant's menu words and corrects near-misses to real menu items.
3. **Order extraction:** turn a transcript into a structured order (items, quantities, sizes, meal components, sauces, modifiers).
4. **Ticket verification:** compare the spoken order with the POS ticket and return a list of mismatches in plain language, e.g. `Said SPICY CHICKEN SANDWICH MEAL, rang CHICKEN SANDWICH MEAL` and `Said 3 Polynesian, rang 1`.
   - **Three outcomes, not two** *(my call)*: **match** (send it), **mismatch** (confident: red light, buzz, alert), and **couldn't confirm** (part of the audio was unclear or an item can't be pinned down: shown quietly on the POS, **no buzz**). Only confident mismatches interrupt the employee, because false alarms train people to ignore the clip.
   - **Things the customer didn't specify aren't mismatches** *(my call)*. If the customer says "a lemonade" without a size, whatever size was rung is accepted.
5. **Speed:** the check result is ready **within ~1 second** of the employee hitting Finalize.
6. **Measurable:** a test harness that reports accuracy numbers (see §9). This is the analytics story for the portfolio.
7. **Optional service-standards module:** off by default. When on, it returns one **boolean per required phrase per order** (e.g. `my_pleasure: true`, `offered_lto: false`).
8. **Restaurant-level dashboard** *(Brenden's decision)*: a simple page built from the anonymous order records, showing mistakes caught per shift/hour, by mistake type, and by menu item (which look-alikes get mixed up most), plus fix rate (caught → fixed vs. sent as rung) and, if standards tracking is on, team-level standards rates. **No employee names or IDs in v1.** This is the screen the operations/analytics audience will look at.

## 4. Non-goals (v1)

- Building the physical clip. A **Mac microphone stands in for the clip.** The software's input interface should be something a future clip (ESP32-class, Wi-Fi) could stream into.
- Real Toast or Chick-fil-A POS integration. v1 uses a **mock POS** (see §7). Real POS integration is a "next step" in the pitch.
- The kitchen expo-screen "food ready" buzz. It's simulated in the mock POS only.
- Retraining / fine-tuning the speech model. That's a later phase once real recorded orders exist (see §11).
- Languages other than English.
- Restaurants with no POS (where the system builds the ticket itself). It's in the vision, not in v1.

## 5. Anti-goals (failure even if it technically works)

- **It must not be or feel like a recording / spying device.**
  - No audio is ever written to disk.
  - Transcripts live only in memory, **only until the order is verified**, then they're deleted.
  - What's kept long-term is an anonymous record per order with no conversation content, e.g. `{order: 1001, caught: 2, types: [item_variant, count], fixed: true, ts: ...}`.
  - It listens **only during an order** (see §6), never all the time.
- **It must never change a ticket by itself.** The AI flags; the human fixes. That's the positioning.
- **No voiceprints / biometric speaker identification.** Illinois BIPA and similar laws make this a legal and trust problem. If speakers need to be told apart, use non-biometric signals (mic proximity/loudness, channel, turn-taking), not enrolled voice profiles.
- **Standards tracking must not read as surveillance.** It's framed as coaching (employees see their own results; managers see team trends), and it's off unless the restaurant turns it on.

## 6. When it listens (Brenden's decision)

Listening is triggered by steps the employee already takes on the POS:

- **Drive-thru:** listening **starts when the employee selects the car's make/type** (e.g. "Navy SUV").
- **Counter / Face2Face:** listening **starts when the guest's name is entered**.
- **Drive-thru has two audio situations** *(Brenden's answer: "both, depends on time")*:
  - **Face-to-face outside** (employees with tablets at the car): the clip hears **both** customer and employee.
  - **Speaker box / headset:** the customer's voice goes into the employee's earpiece, so the clip mostly hears the **employee**, including the employee repeating the order back.
  - The extractor must work in both. Read-backs are **evidence, not noise**: they confirm items without double-counting them, and when the read-back is the only record of an item, it's used.
  - **Known limitation, to state honestly in the pitch:** in headset mode, if the employee *mishears* the customer and reads back the same wrong thing they rang, the tool can't catch it. It still catches fat-fingers (read back "spicy", rang regular). Tapping the headset audio feed directly is a phase 2 idea.
  - The test set must include both situations.
- **Stops:** when the employee hits **Finalize**. Then the check runs. *(My call.)*
- If the ticket is fixed after a mismatch and Finalized again, the check re-runs against the same transcript. The transcript is deleted when the order is sent to the kitchen (or abandoned / timed out). *(My call.)*

## 7. System shape

```
[Mic / future clip] --audio--> [Order Check server (Rust)] <--ticket + events--> [Mock POS (browser)]
                                   |  speech-to-text (whisper.cpp)
                                   |  menu-aware correction
                                   |  order extraction
                                   |  ticket verification
                                   |  standards check (optional)
                                   '--> anonymous order records (SQLite)
```

### 7.1 Order Check server: **Rust** *(Brenden asked for a speed-optimized language; Rust is my call)*
- Runs inside the restaurant (in v1: on the Mac).
- Speech-to-text: **Whisper via whisper.cpp**, called from Rust (e.g. the `whisper-rs` bindings), using Apple Metal GPU acceleration on the M1 Pro.
- Model: start with **large-v3-turbo** (quantized). Fall back to a smaller English model if latency misses the target. The builder picks based on measured latency/accuracy.
- Live mode: audio is captured from the mic while an order is open, held in memory only, and transcribed incrementally (chunked) so the final transcript is ready almost immediately at Finalize.
- File mode: the same pipeline runs on WAV files, for tests and evaluation.
- Exposes a local API (HTTP + WebSocket) that the mock POS talks to: `start order`, `ticket updated`, `finalize → check result`, `sent`, `cancel`.

### 7.2 Mock POS (browser)
- Based on the throwaway spike Brenden reviewed (`.superpowers/brainstorm/.../mock-pos.html`), which copies the Chick-fil-A POS layout. **Brenden said styling differences don't matter at this point.**
- Rebuilt as a proper client of the server. The order flow stays the same: start (car or name) → ring items (category column, item tiles, S/M/L, meal toggle, quantity, cancel item, **plus a way to add modifiers to a ticket line**, which the spike didn't have) → Finalize → mismatch alert with "Fix it" / "Send as rung" → sent → simulated "food ready."
- Shows a simulated clip indicator (off / listening / checking / mismatch / ok / food ready).
- No real brand logos (trademark-safe for posting publicly).

### 7.3 Menu file: the thing that makes it portable
- All restaurant knowledge lives in one config file (e.g. `menus/chick-fil-a.toml`). Swapping that file is how another restaurant would adopt it.
- Per item: id, display name, POS abbreviation, category, price (approximate is fine), sizes if sized, **look-alike family** (spicy/regular/deluxe sandwiches are one family; 8/12 ct nuggets are one family), and **spoken aliases** (e.g. "spicy", "spicy chicken", "spicy sandwich"; "poly", "Polynesian"; "twelve count", "twelve piece").
- Meal rules (entrée + default side + drink).
- **Modifiers** allowed per item, with spoken aliases ("no pickle", "hold the pickles"; "add pepper jack"; "light ice", "no ice"; side substitutions in meals).
- Required standards phrases and their accepted variants (for the optional module), plus the current limited-time item.
- The menu covers the core Chick-fil-A lunch/dinner menu: entrées, sides, sauces, drinks, desserts. Breakfast and catering are out of scope for v1.

## 8. "Training on the menu" (how menu words get recognized) *(my call)*

Brenden asked to "train the product on the words within the menu." v1 does this **without retraining the model**, because retraining needs hours of labeled order audio that doesn't exist yet:

1. **Vocabulary biasing:** feed Whisper a prompt made of menu terms (e.g. "Polynesian, Chick-fil-A sauce, Zesty Buffalo, Sunjoy, 8 count, 12 count nuggets...") so it prefers those spellings.
2. **Menu-aware correction:** after transcription, phrases are matched against the menu's names and aliases with fuzzy/phonetic matching, so near-misses map to real items ("poly nesian" → Polynesian; "chick fil a sauce" → CFA Sauce).
3. **Number and size normalization:** "three" → 3, "a couple" → 2, "twelve piece" → 12 ct, "large" / "big" → LG.

Fine-tuning Whisper on recorded Chick-fil-A-style orders is the **phase 2** upgrade, and the eval harness (§9) will show whether it's needed.

## 9. Measurement: the analytics story

A test set of orders with known correct answers, used for both automated tests and portfolio numbers:

- **Synthetic audio:** generate spoken orders with macOS text-to-speech (multiple voices), optionally mixed with restaurant/drive-thru background noise at several levels.
- **Real audio:** Brenden records a set of orders himself (and friends) acting as customer and employee, to keep the system honest beyond synthetic voices.
- Each test case = audio + the true order + one or more "rung" tickets (some correct, some with planted near-miss mistakes).

Reported metrics:
- **Menu-item accuracy** of the extracted order (item, quantity, size all correct)
- **Mismatch detection:** of planted mistakes, how many were caught (recall), and how often it raised a false alarm on a correct ticket (false-alarm rate). False alarms matter a lot: an employee who gets buzzed wrongly too often will ignore the clip.
- **Latency:** Finalize → result, in milliseconds
- Optionally, a model comparison (2–3 Whisper sizes, or another open model) on the same test set

## 10. Order extraction: the hard part (constraints for the builder)

Real orders aren't one clean sentence. The extractor must handle at least:

- Quantities before items ("three Polynesian", "two twelve-count nuggets")
- Meals ("spicy chicken sandwich meal with a medium sweet tea"), where the meal's drink and size attach to the meal
- **Corrections** ("actually make that a large", "no wait, regular not spicy")
- **The employee repeating the order back** ("three Polynesian, got it"). This must **not** double-count. Without voiceprints the system can't reliably say who spoke, so the extractor needs a strategy for read-backs and confirmations. The builder decides the approach (e.g. confirmation phrases / repeat-detection heuristics, or non-biometric speaker separation), but it has to be covered by test cases.
- Chatter that isn't ordering (greetings, the upsell offer, "pull forward")

Approach is the builder's call, with constraints: runs locally, deterministic enough to unit-test, and fast. A rule/grammar-based extractor driven by the menu file is the expected starting point. A small local language model is acceptable later if rules hit a wall, but no cloud LLM calls (privacy anti-goal).

## 11. Phases

- **Phase 1 (this design):** Rust server with file-mode and live-mic transcription, menu file, correction, extraction (including modifiers), verification, standards booleans, anonymous records; mock POS wired to it; restaurant-level dashboard; eval harness with synthetic + recorded audio and a metrics report.
- **No hard deadline** *(Brenden's answer)*. Build in whatever order makes the most technical sense; quality over speed.
- **Accuracy bar for "done"** *(my call, on the synthetic + recorded test set)*: catches **≥ 90%** of planted mistakes, false alarms on **≤ 5%** of correct tickets, Finalize → result **≤ 1 s**. If rule-based extraction can't reach that, that's the trigger to consider a small local language model.
- **Phase 2 (later, not designed here):** fine-tune on recorded orders; employee-level coaching view; tapping the drive-thru headset feed; non-biometric speaker separation; ESP32 clip prototype streaming audio over Wi-Fi with vibration; real "food ready" signal; POS integration research; no-POS mode.

## 12. Environment

- Brenden's machine: Apple M1 Pro, 16 GB RAM, macOS.
- Not yet installed: Rust toolchain, CMake, ffmpeg (needed for the build / audio tooling). Homebrew and Python 3 are present. Python is fine for **offline tooling only** (generating test audio, plotting the metrics). The live server is Rust.
- Project lives at `~/Projects/order-accuracy-ai/`.

## 13. Left to the builder

- Crate choices, module layout, API shapes, menu file format details
- Whisper model size and quantization (based on measured latency/accuracy)
- Chunking / streaming strategy for live mode
- Extraction algorithm and read-back handling (within §10's constraints)
- Fuzzy-matching method
- Test-set size and noise sources (a few dozen cases minimum, with planted errors covering every mistake type in §2, in both drive-thru audio situations from §6)
- How many orders run at once (plan for one, keyed so more can be added later), timeout for abandoned orders, whether the POS reads its menu from the same menu file
- Test audio fixtures may live on disk. The no-audio-on-disk rule applies to live use, not the eval set.
- Dashboard tech and look
- Exact mock POS rebuild (functional parity with the spike; looks don't matter)
