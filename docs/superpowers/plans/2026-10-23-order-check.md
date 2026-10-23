# Order Check v1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A local Rust server that listens during a restaurant order, transcribes it with Whisper, extracts the spoken order using a menu file, verifies it against the POS ticket, and reports match / mismatch / couldn't-confirm. It comes with a mock POS, a restaurant-level dashboard, and an evaluation harness that produces accuracy numbers.

**Architecture:** One Rust crate (`order-check`): a library with focused modules (menu → normalize → matcher → extract → verify, plus stt, audio, session, records, server) and a binary with `serve`, `check`, and `eval` subcommands. The browser pages (POS, dashboard) are static files served by the binary and talk to it over HTTP and WebSocket. Offline Python in `tools/` generates the synthetic test set with macOS `say` and ffmpeg.

**Tech Stack:** Rust stable; `whisper-rs` (whisper.cpp bindings, `metal` feature); `axum` + `tokio` + `tower-http`; `serde`/`serde_json`/`toml`; `rusqlite` (`bundled`); `strsim`; `hound`; `cpal`; `clap`; `anyhow`/`thiserror`. Python 3 stdlib only + ffmpeg for tools. Add crates with `cargo add` (latest versions).

**Spec:** `docs/superpowers/specs/2026-10-23-order-check-design.md`

## Global Constraints

- Live mode never writes audio to disk. Transcripts live only in memory and are deleted on send, cancel, or a 10-minute abandon timeout.
- No network calls at runtime other than localhost. No cloud APIs, no cloud LLMs.
- No voiceprints or speaker enrollment of any kind.
- The system never modifies a ticket. It only reports.
- All restaurant knowledge (items, aliases, families, modifiers, meal rules, standards, LTO) lives in `menus/*.toml`. No menu item names are hard-coded in Rust (cue words like "actually" and "would you like" are language, not menu, and may live in Rust).
- Targets: Finalize → result ≤ 1 s; catches ≥ 90% of planted mistakes; false alarms on ≤ 5% of correct tickets.
- Mismatch message formats, exactly: `Said SPICY CHICKEN SANDWICH MEAL, rang CHICKEN SANDWICH MEAL` (variant: uppercase names) · `Said 3 Polynesian, rang 1` (count) · `Said Large Lemonade, rang Medium` (size) · `Said No Pickles on Chicken Sandwich, not on ticket` / `Rang No Pickles on Chicken Sandwich, guest didn't ask` (modifier) · `Said 2 Chick-fil-A Sauce, not on ticket` (missing) · `Rang 1 Cookie, guest didn't ask` (extra).
- Something the customer didn't specify (quantity, size, meal side, meal drink) is never a mismatch.
- Standards tracking is off by default; when on, it returns one boolean per standard per order.
- Stored order records contain no transcript text and no employee identity.
- English only. Live system is Rust; Python only under `tools/`.

## Review Focus

- **Declined upsell:** the employee offers the seasonal shake and the guest says "no thanks". The shake must not be added to the spoken order. Pinned in Task 4.
- **Read-back with a different count than the customer said:** "three Polynesian" … "one Polynesian, got it". The customer's original value wins. Pinned in Task 4.
- **Silent or near-silent order** (mic muffled, guest never heard): never a wall of "Rang X, guest didn't ask" alarms. The result is Unconfirmed. Pinned in Task 5.
- **Mic unavailable** (permission denied / no input device): the order still proceeds; finalize returns Unconfirmed with reason `"clip offline"`; the POS still lets the employee send. Pinned in Task 9.
- **Abandoned order** (started, never finalized): transcript dropped after 10 minutes, no record written. Pinned in Task 9.

---

## File Structure

```
order-accuracy-ai/
  Cargo.toml
  menus/chick-fil-a.toml        # all menu knowledge
  src/lib.rs                    # pub mod declarations
  src/menu.rs                   # Menu types + load + validate + vocabulary_prompt
  src/normalize.rs              # text → normalized tokens
  src/matcher.rs                # tokens → Mentions (items, sizes, qty, modifiers, cues)
  src/extract.rs                # utterances → SpokenOrder
  src/ticket.rs                 # Ticket types (POS JSON)
  src/verify.rs                 # SpokenOrder × Ticket → CheckResult
  src/standards.rs              # optional standards booleans
  src/audio.rs                  # WAV load/resample, energy VAD segmenter
  src/stt.rs                    # Transcriber trait + WhisperTranscriber
  src/pipeline.rs               # check_utterances / check_audio
  src/records.rs                # SQLite anonymous records + dashboard aggregates
  src/mic.rs                    # cpal capture → 16 kHz mono
  src/session.rs                # order lifecycle, in-memory transcripts, timeout
  src/server.rs                 # axum routes, WS events, static files
  src/eval.rs                   # eval harness + report
  src/main.rs                   # clap: serve | check | eval
  web/pos.html                  # mock POS client
  web/dashboard.html            # restaurant dashboard
  tools/conversations.py        # scripted test conversations + truth tickets
  tools/gen_testset.py          # say + ffmpeg → eval/cases/*
  tools/download_models.sh
  tests/                        # integration tests (fixtures under tests/fixtures/)
  README.md
```

`.gitignore`: `target/`, `models/`, `data/`, `.superpowers/`, `eval/cases/`, `eval/report*`.

---

### Task 1: Project scaffold + menu file + menu loader

**Files:**
- Create: `Cargo.toml`, `.gitignore`, `src/lib.rs`, `src/menu.rs`, `menus/chick-fil-a.toml`, `tools/download_models.sh`
- Test: `src/menu.rs` (unit tests module)

**Interfaces:**
- Produces:
  - `pub enum Category { Entree, Side, Sauce, Drink, Dessert }` (serde lowercase)
  - `pub enum Size { Sm, Md, Lg }` (serde `"sm" | "md" | "lg"`), `impl Size { pub fn word(&self) -> &'static str /* "Small"|"Medium"|"Large" */ }`
  - `pub struct MenuItem { pub id: String, pub name: String, pub pos: String, pub category: Category, pub price: f32, pub sized: bool, pub family: String, pub meal: bool, pub aliases: Vec<String>, pub modifiers: Vec<String> }`
  - `pub struct Family { pub id: String, pub base: String /* item id */, pub aliases: Vec<String> }`. Generic words like "nuggets", "sauce", "lemonade" resolve to the family.
  - `pub struct Modifier { pub id: String, pub name: String, pub aliases: Vec<String> }`
  - `pub struct MealRule { pub default_side: String, pub default_size: Size, pub upcharge: f32 }`
  - `pub enum StandardKind { Phrase, OfferLto }`; `pub struct StandardDef { pub id: String, pub kind: StandardKind, pub phrases: Vec<String> }`
  - `pub struct Menu { pub restaurant: String, pub items: Vec<MenuItem>, pub families: Vec<Family>, pub modifiers: Vec<Modifier>, pub meal: MealRule, pub lto_items: Vec<String>, pub standards: Vec<StandardDef> }`
  - `impl Menu { pub fn load(path: &Path) -> anyhow::Result<Menu>; pub fn item(&self, id: &str) -> Option<&MenuItem>; pub fn family(&self, id: &str) -> Option<&Family>; pub fn modifier(&self, id: &str) -> Option<&Modifier>; pub fn vocabulary_prompt(&self) -> String }`. `load` calls a private `validate()`.

- [ ] **Step 1: Scaffold.** `git init`, `cargo init --lib --name order-check`, add a `[[bin]] name = "order-check" path = "src/main.rs"` with a stub `main`. Run `cargo add serde --features derive`, `cargo add serde_json toml anyhow thiserror`. Write `.gitignore` per File Structure. Write `tools/download_models.sh` to fetch `ggml-large-v3-turbo-q5_0.bin` and `ggml-base.en.bin` from `https://huggingface.co/ggerganov/whisper.cpp/resolve/main/` into `models/` (skip files already present).

- [ ] **Step 2: Write `menus/chick-fil-a.toml`.** Top-level `restaurant`, `lto_items = ["peach_shake"]`, `[meal] default_side = "waffle_fries", default_size = "md", upcharge = 3.80`, then `[[families]]`, `[[items]]`, `[[modifiers]]`, `[[standards]]`. Required content:
  - **Entrées** (`meal = true`): chicken_sandwich ("Chicken Sandwich", family sandwich, base), spicy_sandwich ("Spicy Chicken Sandwich"), deluxe_sandwich ("Deluxe Chicken Sandwich"), spicy_deluxe ("Spicy Deluxe Sandwich"), grilled_sandwich ("Grilled Chicken Sandwich"); nuggets_8/nuggets_12 ("8 ct Nuggets"/"12 ct Nuggets", family nuggets, base nuggets_8), grilled_nuggets_8/12 (family grilled_nuggets); strips_3/strips_4 (family strips); cool_wrap.
  - **Sides** (`sized` where real): waffle_fries, mac_and_cheese, fruit_cup, side_salad.
  - **Sauces** (family per sauce, price 0): cfa_sauce ("Chick-fil-A Sauce"), polynesian ("Polynesian"), honey_mustard, garden_herb_ranch, barbeque, zesty_buffalo, sriracha ("Sweet & Spicy Sriracha"), honey_roasted_bbq.
  - **Drinks** (`sized = true`): sweet_tea and unsweet_tea (family tea, base sweet_tea), lemonade and diet_lemonade (family lemonade), sunjoy, cola and diet_cola (family cola), lemon_lime, dr_pepper, water (not sized).
  - **Desserts:** shakes cookies_cream_shake, chocolate_shake, vanilla_shake, strawberry_shake, peach_shake ("Peach Milkshake", the LTO), all family shake; cookie ("Cookie"); frosted_lemonade.
  - **Aliases:** include the spoken forms ("spicy chicken sandwich", "spicy sandwich", "spicy chicken"; "twelve count nuggets" written as normalized `12 count nuggets`, `12 piece nuggets`, `12 nuggets`; "poly", "polynesian sauce"; "chick fil a sauce", "cfa sauce"; "sweet tea"; "mac and cheese", "mac"; "cookies and cream shake", "cookies and cream milkshake"). Aliases are written already normalized (Task 2 rules).
  - **Modifiers:** no_pickles, extra_pickles, add_cheese ("add cheese", "with cheese"), add_pepper_jack, no_lettuce, no_tomato, no_ice, light_ice, extra_ice, add_lemon, no_whipped_cream, no_cherry. List each on the items it applies to (sandwiches get the pickle/cheese/lettuce/tomato ones, drinks get ice/lemon, shakes get whipped cream/cherry).
  - **Standards:** `{ id = "my_pleasure", kind = "phrase", phrases = ["my pleasure"] }`, `{ id = "offered_lto", kind = "offer_lto", phrases = [] }`.

- [ ] **Step 3: Write failing tests** in `src/menu.rs`:
  - `loads_cfa_menu`: `Menu::load("menus/chick-fil-a.toml")` is Ok; `item("spicy_sandwich").unwrap().family == "sandwich"`; `family("nuggets").unwrap().base == "nuggets_8"`; `item("polynesian").unwrap().price == 0.0`.
  - `validate_rejects_unknown_refs`: a TOML string whose item lists modifier `"no_such_mod"` → `Menu::from_toml_str(..)` is Err with a message containing `no_such_mod`. Also covers a family `base` that isn't an item, a `lto_items` entry that isn't an item, and a duplicate id. (Add `pub fn from_toml_str(s: &str) -> anyhow::Result<Menu>`, which `load` uses.)
  - `vocabulary_prompt_contains_menu_terms`: the prompt contains `"Polynesian"`, `"Chick-fil-A Sauce"`, `"Spicy Deluxe"` and is ≤ 900 characters.

- [ ] **Step 4: Run** `cargo test menu`. Expected: FAIL (not implemented).
- [ ] **Step 5: Implement** the types, `from_toml_str`, `load`, `validate`, the lookups (a private `HashMap` index built in `from_toml_str`; skip it during serde), and `vocabulary_prompt` (comma-joined display names of items, families, and modifiers, deduped, truncated to 900 chars).
- [ ] **Step 6: Run** `cargo test menu`. Expected: PASS.
- [ ] **Step 7: Commit** `feat: menu file and loader`.

---

### Task 2: Text normalization

**Files:** Create `src/normalize.rs` (with tests)

**Interfaces:**
- Produces: `pub fn normalize(text: &str) -> Vec<String>` (tokens) and `pub fn split_sentences(text: &str) -> Vec<String>`.

Rules, in order: lowercase; `chick-fil-a` / `chick fil a` / `chickfila` → tokens `chick fil a`; `&` → `and`; `8-count` / `8ct` / `8-piece` / `8pc` → `8 count`; `piece` → `count` when it follows a number; drop punctuation except apostrophes inside words; number words zero to twenty plus `a couple` → digits (`twelve` → `12`, `a couple` → `2`); collapse whitespace.

- [ ] **Step 1: Failing tests:**
  - `normalize("Can I get THREE Polynesian sauces?") == ["can","i","get","3","polynesian","sauces"]`
  - `normalize("a twelve-piece nugget") == ["a","12","count","nugget"]`
  - `normalize("Chick-fil-A sauce & a large lemonade.") == ["chick","fil","a","sauce","and","a","large","lemonade"]`
  - `normalize("a couple cookies") == ["2","cookies"]`
  - `split_sentences("Hi. Can I get a meal? Yes!") == ["Hi.","Can I get a meal?","Yes!"]`
- [ ] **Step 2: Run** `cargo test normalize`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test normalize`. Expected: PASS.
- [ ] **Step 5: Commit** `feat: transcript normalization`.

---

### Task 3: Mention matcher

**Files:** Create `src/matcher.rs` (with tests). `cargo add strsim`.

**Interfaces:**
- Consumes: `Menu`, `normalize`.
- Produces:
  - `pub enum Cue { Correction, Removal, Offer, Accept, Decline, Additive, Negation }`
  - `pub enum MentionKind { Item(String), Family(String), FamilyBase, Modifier(String), Size(Size), Quantity(u32), Article, Meal, Cue(Cue) }`
  - `pub struct Mention { pub kind: MentionKind, pub start: usize, pub end: usize, pub score: f32 }` (token span, `end` exclusive; `score` 1.0 for exact matches)
  - `pub struct Matcher` + `impl Matcher { pub fn new(menu: &Menu) -> Matcher; pub fn find(&self, tokens: &[String]) -> Vec<Mention> }` (sorted by `start`, non-overlapping)

Fixed language tables in `matcher.rs` (not menu knowledge):
- Correction: `actually`, `make that`, `make it`, `change that`, `change it`, `instead`, `no wait`, `i mean`
- Removal: `never mind`, `nevermind`, `take off`, `remove`, `cancel`, `scratch`, `forget`
- Offer: `would you like`, `do you want`, `want to try`, `like to try`, `can i interest`, `how about`, `try our`
- Accept: `yes`, `yeah`, `sure`, `sounds good`, `i'll take`, `i'll try`, `let's do`, `why not`
- Decline: `no thanks`, `no thank you`, `i'm good`, `not today`, `nope`
- Additive: `another`, `one more`, `also another`, `plus another`
- Negation: `not` (when followed by an item)
- Size: `small`→Sm, `medium`→Md, `large`/`big`→Lg. FamilyBase: `regular`, `original`, `normal`. Meal: `meal`, `combo`. Article: `a`, `an`.

Matching algorithm: walk left to right. At each position try the longest n-gram (n ≤ 6) found in a single exact table (item aliases + item names + family aliases + modifier aliases + the cue/size/meal tables; on a tie, item beats family). A bare digit token not consumed by an alias → `Quantity`. If nothing matches exactly, try fuzzy matching against item and modifier aliases of ≥ 5 characters: compare the n-gram (n ≤ 4) joined with spaces, and also with the spaces removed, against each alias using `strsim::normalized_damerau_levenshtein`. Accept the best match ≥ 0.85 with `score` = similarity. Plural `s` on the final token is tolerated (`sauces`, `nuggets`).

- [ ] **Step 1: Failing tests:**
  - `exact_items_and_quantity`: tokens of "can i get 3 polynesian sauces" → contains `Quantity(3)` then `Item("polynesian")`.
  - `longest_match_wins`: "spicy deluxe sandwich" → a single `Item("spicy_deluxe")`, not `spicy_sandwich`.
  - `count_alias`: "a 12 count nugget meal" → `Article`, `Item("nuggets_12")`, `Meal`.
  - `fuzzy_split_word`: "poly nesian" → `Item("polynesian")` with `0.85 <= score < 1.0`.
  - `fuzzy_rejects_far`: "pineapple" → no Item mention.
  - `cues`: "actually make that a large" → `Cue(Correction)` (from "actually"), `Cue(Correction)` ("make that"), `Article`, `Size(Lg)`.
  - `offer`: "would you like to try our peach milkshake" → `Cue(Offer)` … `Item("peach_shake")`.
  - `family_generic`: "nuggets" alone → `Family("nuggets")`.
  - `modifier`: "no pickles" → `Modifier("no_pickles")` (it must beat the `Decline` cue `no`, which isn't in the table; `no thanks` is).
- [ ] **Step 2: Run** `cargo test matcher`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test matcher`. Expected: PASS.
- [ ] **Step 5: Commit** `feat: menu-aware mention matcher with fuzzy correction`.

---

### Task 4: Order extraction

**Files:** Create `src/extract.rs` (with tests)

**Interfaces:**
- Consumes: `Menu`, `Matcher`, `normalize`, `split_sentences`.
- Produces:
  - `pub struct Utterance { pub text: String, pub confidence: f32 }`
  - `pub struct SpokenMeal { pub side: Option<SpokenComponent>, pub drink: Option<SpokenComponent> }`
  - `pub struct SpokenComponent { pub item: Option<String>, pub family: String, pub size: Option<Size>, pub modifiers: Vec<String>, pub confidence: f32 }`
  - `pub struct SpokenLine { pub item: Option<String>, pub family: String, pub qty: Option<u32>, pub size: Option<Size>, pub modifiers: Vec<String>, pub meal: Option<SpokenMeal>, pub meal_size: Option<Size>, pub confidence: f32 }` (`item == None` means only the family was said)
  - `pub struct SpokenOrder { pub lines: Vec<SpokenLine>, pub offered: Vec<String> }`
  - `pub fn extract(menu: &Menu, matcher: &Matcher, utts: &[Utterance]) -> SpokenOrder`

Rules. Each `Utterance` is split into sentences; each sentence is one clause for cue scope.
1. **Quantity:** `Quantity(n)` directly before an item/family (only a size word may sit between) sets `qty = Some(n)`. `Article` directly before it → `Some(1)`. Otherwise `qty = None` (unspecified).
2. **Size:** a `Size` directly before an item, or directly after it, sets the size on sized items. A size directly before `Meal` (or before an entrée that is followed by `Meal`) sets `meal_size`.
3. **Meal:** an entrée followed within 1 token by `Meal` → `meal = Some(SpokenMeal{side: None, drink: None})`. The next drink mention in the same sentence or the next sentence, before another entrée appears, becomes the meal's drink. A side mention after `with`, `sub`, or `substitute` attached to a meal becomes the meal's side.
4. **Modifiers** attach to the most recent line (or its meal drink/side) whose item lists that modifier, searching back at most 2 lines. If nothing allows it, drop it.
5. **Offers:** after a `Cue(Offer)`, items in the rest of that sentence go to `offered` and to a pending list, not to `lines`. If the next sentence starts with `Accept`, the pending items become lines. Anything else clears the pending list.
6. **Corrections:** after `Cue(Correction)` in a sentence, the following mentions in that sentence edit the last line instead of adding one: `Size` → size; `Quantity` → qty; `Meal` → make it a meal; `Item` → replace the item (keep qty, size, modifiers); `FamilyBase` → replace with the last line's family base. `Negation` followed by an item: that item mention is ignored ("regular not spicy").
7. **Removal:** `Cue(Removal)` followed by an item removes the most recent line with that item. With no item, it removes the last line.
8. **Read-back / duplicates:** a new item mention that's compatible with an existing line is merged, not added, unless an `Additive` cue precedes it in the same sentence. Compatible means the same item, or one side is family-only in the same family, and qty/size don't conflict where both are `Some`. Merge fills `None` fields from the new mention but never overwrites a `Some` (the first statement wins). A mention that conflicts only on qty or size with an earlier line of the same item is also merged by the same rule (first statement wins). Corrections are the only way to change a value.
9. **Confidence:** `line.confidence = min(utterance.confidence, mention.score)`. A merge keeps the max.

- [ ] **Step 1: Failing tests.** Helper `fn ex(lines: &[&str]) -> SpokenOrder` (each line is an `Utterance` with confidence 0.95, using the real CFA menu). Assertions on `lines` (item, qty, size, meal, modifiers):
  - `spicy_meal_and_sauces`: ["Welcome! What can I get started for you?", "Can I get a spicy chicken sandwich meal with a medium sweet tea?", "Would you like to try our seasonal peach milkshake today?", "No thanks. Oh, and can I get three Polynesian sauces?", "Three Polynesian, got it. Anything else?", "That's it.", "My pleasure!"] → exactly 2 lines: spicy_sandwich qty 1 with meal drink sweet_tea Md; polynesian qty 3. `offered == ["peach_shake"]`. **(Review Focus: declined upsell, read-back not double-counted)**
  - `accepted_offer`: ["Would you like to try our peach milkshake?", "Sure!"] → 1 line peach_shake.
  - `readback_conflict_first_wins`: ["Can I get three Polynesian sauces", "One Polynesian, got it"] → polynesian qty 3. **(Review Focus)**
  - `headset_readback_only`: ["So that's a 12 count nugget meal with a large lemonade and two chick fil a sauces?", "Pull forward please."] → nuggets_12 meal with drink lemonade Lg; cfa_sauce qty 2.
  - `correction_size`: ["a medium lemonade", "actually make that a large"] → lemonade Lg, 1 line.
  - `correction_variant_regular`: ["a spicy chicken sandwich", "no wait, regular not spicy"] → chicken_sandwich.
  - `make_it_a_meal`: ["8 count nuggets", "actually make it a meal"] → nuggets_8 with meal.
  - `removal`: ["a cookie and a lemonade", "never mind the cookie"] → only lemonade.
  - `modifiers`: ["a chicken sandwich with no pickles and a large sweet tea no ice"] → chicken_sandwich mods [no_pickles]; sweet_tea Lg mods [no_ice].
  - `additive_duplicate`: ["a cookie", "and another cookie"] → cookie qty 1 twice (or one line qty 2; either is accepted by Task 5's aggregation, so assert the summed qty == 2).
  - `unspecified`: ["some nuggets and a lemonade"] → family-only nuggets with qty None; lemonade with size None.
- [ ] **Step 2: Run** `cargo test extract`. Expected: FAIL.
- [ ] **Step 3: Implement** `extract` per rules 1–9.
- [ ] **Step 4: Run** `cargo test extract`. Expected: PASS.
- [ ] **Step 5: Commit** `feat: spoken order extraction`.

---

### Task 5: Ticket model + verification

**Files:** Create `src/ticket.rs`, `src/verify.rs` (with tests)

**Interfaces:**
- Consumes: `Menu`, `SpokenOrder`.
- Produces:
  - `pub struct TicketComponent { pub item: String, pub size: Option<Size>, pub modifiers: Vec<String> }`
  - `pub struct TicketMeal { pub side: TicketComponent, pub drink: Option<TicketComponent> }`
  - `pub struct TicketLine { pub item: String, pub qty: u32, pub size: Option<Size>, pub modifiers: Vec<String>, pub meal: Option<TicketMeal> }`
  - `pub struct Ticket { pub lines: Vec<TicketLine> }` (all serde; this JSON is what the POS sends) + `impl Ticket { pub fn validate(&self, menu: &Menu) -> anyhow::Result<()> }` (unknown item or modifier ids → Err)
  - `pub enum MismatchKind { Variant, Count, Size, Modifier, Missing, Extra }` (serde snake_case)
  - `pub struct Mismatch { pub kind: MismatchKind, pub message: String, pub said: Option<String>, pub rang: Option<String> }` (`said`/`rang` are item ids)
  - `pub enum Outcome { Match, Mismatch, Unconfirmed }` (serde snake_case)
  - `pub struct CheckResult { pub outcome: Outcome, pub mismatches: Vec<Mismatch>, pub unconfirmed: Vec<Mismatch>, pub reason: Option<String> }`
  - `pub fn verify(menu: &Menu, spoken: &SpokenOrder, ticket: &Ticket, transcript_confidence: f32) -> CheckResult` (`transcript_confidence` = the minimum utterance confidence, or 0.0 if there were no utterances)
  - `pub const MIN_CONFIDENCE: f32 = 0.6;`

Algorithm:
1. **Flatten** both sides into units `{item: Option<id>, family, qty: Option<u32>, size, modifiers, role, confidence}`, where role is `Entree{meal: bool}` / `Component` (meal side, meal drink, and standalone non-entrées are all `Component`). Meal components inherit the line's qty. A ticket meal side equal to `meal.default_side` at `default_size` is marked `default_side = true`.
2. **Aggregate** each side by (item, size, sorted modifiers, role) and sum qty. On the spoken side, a `None` in any part makes the aggregate qty `None`.
3. **Pass A (same item):** pair spoken and ticket units with the same item id (prefer the same size, then the same modifiers). For a pair: if the meal flag differs → Variant; if spoken qty is `Some(a)` and a ≠ ticket qty → Count; if spoken size is `Some` and differs → Size; each spoken modifier missing on the ticket → Modifier ("Said … not on ticket"); each ticket modifier not spoken → Modifier ("Rang … guest didn't ask").
4. **Pass B (same family):** leftover spoken units pair with leftover ticket units of the same family. Spoken item `None` → no Variant (unspecified); otherwise → Variant. Qty, size, and modifiers are then compared as in pass A.
5. **Pass C:** leftover spoken → Missing. Leftover ticket → Extra, except (a) components whose meal had no spoken side/drink, and (b) `default_side` units.
6. **Confidence:** a mismatch whose spoken unit has confidence < `MIN_CONFIDENCE` goes to `unconfirmed`. **Extra** mismatches go to `unconfirmed` when `transcript_confidence < MIN_CONFIDENCE`. If the spoken order is empty, then every Extra goes to `unconfirmed` and `reason = Some("nothing heard")`.
7. **Outcome:** `Mismatch` if `mismatches` is non-empty; else `Unconfirmed` if `unconfirmed` is non-empty or `reason` is set; else `Match`.
8. **Names in messages:** display name with a size word prefix for sized items (`Large Lemonade`) and the suffix ` Meal` for meal entrées. Variant messages uppercase both names. Formats per Global Constraints.

- [ ] **Step 1: Failing tests.** Build a `SpokenOrder` with `extract` from short utterances and a `Ticket` from JSON literals:
  - `match_exact`: spoken spicy meal + sweet tea Md + 3 Polynesian; the ticket has the same (with the default fries side) → `Outcome::Match`, no mismatches.
  - `variant_and_count` (the spec example): ticket has chicken_sandwich meal + polynesian qty 1 → messages are exactly `"Said SPICY CHICKEN SANDWICH MEAL, rang CHICKEN SANDWICH MEAL"` and `"Said 3 Polynesian, rang 1"`; outcome Mismatch.
  - `size`: spoken "a large lemonade", ticket lemonade Md → `"Said Large Lemonade, rang Medium"`.
  - `modifier_missing_and_extra`: spoken "chicken sandwich no pickles", ticket chicken_sandwich with [add_cheese] → `"Said No Pickles on Chicken Sandwich, not on ticket"` and `"Rang Add Cheese on Chicken Sandwich, guest didn't ask"`.
  - `missing_and_extra`: spoken "two chick fil a sauces", ticket has a cookie only → `"Said 2 Chick-fil-A Sauce, not on ticket"` and `"Rang 1 Cookie, guest didn't ask"`.
  - `unspecified_is_not_mismatch`: spoken "some nuggets and a lemonade", ticket nuggets_12 qty 1 + lemonade Lg → Match.
  - `meal_drink_unspecified`: spoken "a spicy chicken sandwich meal", ticket meal with drink sweet_tea Md → Match.
  - `low_confidence_goes_unconfirmed`: same as variant_and_count but utterance confidence 0.4 → outcome Unconfirmed, the mismatches are in `unconfirmed`.
  - `silence_is_unconfirmed` **(Review Focus)**: empty utterances, a 3-line ticket → outcome Unconfirmed, `reason == Some("nothing heard")`, `mismatches` empty.
  - `ticket_validate_rejects_unknown`: an item `"big_mac"` → Err.
- [ ] **Step 2: Run** `cargo test verify`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test verify`. Expected: PASS.
- [ ] **Step 5: Commit** `feat: ticket verification`.

---

### Task 6: Optional standards check

**Files:** Create `src/standards.rs` (with tests)

**Interfaces:**
- Produces: `pub fn check_standards(menu: &Menu, utts: &[Utterance], spoken: &SpokenOrder) -> BTreeMap<String, bool>`. `Phrase`: true if any phrase (normalized) occurs as a contiguous token run in any utterance. `OfferLto`: true if `spoken.offered` contains any of `menu.lto_items`.

- [ ] **Step 1: Failing tests:** using the `spicy_meal_and_sauces` utterances → `{"my_pleasure": true, "offered_lto": true}`; ["Hi, a lemonade please", "Thanks, we'll bring it out"] → both false.
- [ ] **Step 2: Run** `cargo test standards`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test standards`. Expected: PASS.
- [ ] **Step 5: Commit** `feat: optional service standards booleans`.

---

### Task 7: Audio, Whisper STT, pipeline, and the `check` command

**Files:** Create `src/audio.rs`, `src/stt.rs`, `src/pipeline.rs`, `src/main.rs`; test `tests/check_audio.rs`, fixture `tests/fixtures/spicy_meal.ticket.json`. `cargo add hound clap --features clap/derive`, `cargo add whisper-rs --features metal`.

**Interfaces:**
- Produces:
  - `audio.rs`: `pub fn load_wav_16k_mono(path: &Path) -> anyhow::Result<Vec<f32>>` (downmix, linear resample to 16 kHz); `pub struct Segmenter` + `impl Segmenter { pub fn new() -> Self; pub fn push(&mut self, pcm: &[f32]) -> Vec<Vec<f32>>; pub fn flush(&mut self) -> Option<Vec<f32>> }`. Energy VAD on 30 ms frames: speech if RMS > max(0.01, 3 × the running noise floor); an utterance ends after ≥ 600 ms of silence; utterances < 300 ms are dropped; each emitted utterance is padded with 150 ms of its own leading/trailing audio.
  - `stt.rs`: `pub trait Transcriber: Send + Sync { fn transcribe(&self, pcm: &[f32], prompt: &str) -> anyhow::Result<Utterance>; }`; `pub struct WhisperTranscriber` with `pub fn new(model: &Path) -> anyhow::Result<Self>`. Settings: English, greedy, no timestamps, `initial_prompt = prompt`, threads = 4. `Utterance.text` = joined segment texts, trimmed. `confidence` = mean token probability over non-special tokens (0.0 if no tokens). Also `pub struct FakeTranscriber(pub Vec<Utterance>)`, which returns the queued utterances in order (for tests).
  - `pipeline.rs`: `pub struct CheckReport { pub result: CheckResult, pub standards: Option<BTreeMap<String,bool>>, pub transcript: Vec<Utterance>, pub verify_ms: u32 }`; `pub fn check_utterances(menu: &Menu, matcher: &Matcher, utts: Vec<Utterance>, ticket: &Ticket, standards_on: bool) -> CheckReport`; `pub fn transcribe_segments(t: &dyn Transcriber, menu: &Menu, segments: &[Vec<f32>]) -> anyhow::Result<Vec<Utterance>>`. The prompt for each segment = `menu.vocabulary_prompt()` + the last 200 characters of earlier transcript text.
  - `main.rs`: clap subcommands. `check --menu <toml> --model <bin> --audio <wav> --ticket <json> [--standards]` prints `CheckReport` as pretty JSON (the CLI is the only place transcripts are printed, for debugging fixtures).

- [ ] **Step 1: Create the fixture audio** (not committed, generated by the test itself): the integration test `check_audio_spicy_meal` runs `say -v Samantha -o <tmp>.aiff "Can I get a spicy chicken sandwich meal with a medium sweet tea, and three Polynesian sauces?"`, then `ffmpeg -y -i <tmp>.aiff -ar 16000 -ac 1 <tmp>.wav`. It is skipped (with a printed reason) if `models/ggml-base.en.bin` or `say` is missing. The ticket fixture holds a regular chicken_sandwich meal + polynesian qty 1.
- [ ] **Step 2: Write failing tests:**
  - unit `segmenter_splits_on_silence`: synthetic 16 kHz PCM = 0.5 s of 440 Hz sine at amplitude 0.3, 1.0 s of silence, 0.5 s of sine → `push` + `flush` yields 2 segments.
  - unit `segmenter_drops_blips`: a 100 ms tone between silences → 0 segments.
  - integration `check_audio_spicy_meal`: load the wav, segment, transcribe with `WhisperTranscriber` (base.en), then `check_utterances` → outcome Mismatch with both spec messages present.
- [ ] **Step 3: Run** `cargo test --test check_audio && cargo test audio`. Expected: FAIL.
- [ ] **Step 4: Implement** audio, stt, pipeline, and the `check` subcommand.
- [ ] **Step 5: Run** the same tests. Expected: PASS. Then run `cargo run --release -- check --menu menus/chick-fil-a.toml --model models/ggml-large-v3-turbo-q5_0.bin --audio <the wav> --ticket tests/fixtures/spicy_meal.ticket.json` and confirm the JSON shows both mismatches.
- [ ] **Step 6: Commit** `feat: whisper transcription and check command`.

---

### Task 8: Anonymous records + dashboard aggregates

**Files:** Create `src/records.rs` (with tests). `cargo add rusqlite --features bundled`.

**Interfaces:**
- Produces:
  - `pub struct OrderRecord { pub order_num: u32, pub mode: String /* "drive_thru"|"face2face" */, pub ts: i64, pub outcome: Outcome, pub caught: u32, pub kinds: Vec<MismatchKind>, pub pairs: Vec<(Option<String>, Option<String>)> /* (said, rang) item ids */, pub fixed: bool, pub sent_as_rung: bool, pub standards: Option<BTreeMap<String,bool>>, pub latency_ms: u32 }`
  - `pub struct Dashboard { pub orders: u32, pub orders_with_catch: u32, pub caught_total: u32, pub fix_rate: f32, pub by_kind: BTreeMap<String,u32>, pub by_hour: Vec<(u32 /*0-23*/, u32 /*orders*/, u32 /*caught*/)>, pub top_pairs: Vec<(String,String,u32)> /* said name, rang name, count; top 10 */, pub standards_rates: BTreeMap<String,f32>, pub p50_latency_ms: u32 }`
  - `pub struct Records` + `impl Records { pub fn open(path: &Path) -> anyhow::Result<Self>; pub fn open_in_memory() -> anyhow::Result<Self>; pub fn insert(&self, r: &OrderRecord) -> anyhow::Result<()>; pub fn dashboard(&self, menu: &Menu, since_ts: i64) -> anyhow::Result<Dashboard> }`
  - Table `orders` columns exactly: `id, order_num, mode, ts, outcome, caught, kinds, pairs, fixed, sent_as_rung, standards, latency_ms` (JSON in the TEXT columns). `fix_rate` = fixed / orders_with_catch (0 if none).

- [ ] **Step 1: Failing tests:** `schema_has_no_text_content` (`PRAGMA table_info(orders)` column names == the exact list above); `dashboard_aggregates` (insert 3 records: a clean one, one caught+fixed variant spicy→regular at hour 12, one sent_as_rung count) → orders 3, orders_with_catch 2, fix_rate 0.5, `by_kind["variant"] == 1`, top_pairs[0] == ("Spicy Chicken Sandwich","Chicken Sandwich",1).
- [ ] **Step 2: Run** `cargo test records`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test records`. Expected: PASS.
- [ ] **Step 5: Commit** `feat: anonymous order records and dashboard aggregates`.

---

### Task 9: Sessions, live mic, and the server

**Files:** Create `src/session.rs`, `src/mic.rs`, `src/server.rs`; modify `src/main.rs` (add `serve`). Test `tests/session.rs`, `tests/server.rs`. `cargo add tokio --features full`, `cargo add axum --features ws`, `cargo add tower-http --features fs`, `cargo add cpal`. Dev: `cargo add --dev reqwest --features json` (or drive axum with `tower::ServiceExt::oneshot`).

**Interfaces:**
- Consumes: everything above.
- Produces:
  - `mic.rs`: `pub trait AudioSource: Send { fn start(&mut self, sink: std::sync::mpsc::Sender<Vec<f32>>) -> anyhow::Result<()>; fn stop(&mut self); }`; `pub struct CpalMic` (default input device → 16 kHz mono chunks; `start` errors if there is no device or permission is denied; `stop` drops the stream so the OS mic indicator turns off); `pub struct NoMic` (always errors on start, for tests).
  - `session.rs`: `pub enum ClipState { Off, Listening, Checking, Mismatch, Unconfirmed, Ok, Ready, Offline }`; `pub struct Sessions` + `impl Sessions { pub fn new(menu: Arc<Menu>, stt: Arc<dyn Transcriber>, records: Records, mic: Box<dyn AudioSource>, standards_on: bool, events: tokio::sync::broadcast::Sender<Event>) -> Self; pub fn start(&self, mode: &str, label: &str) -> anyhow::Result<u32 /*order_num*/>; pub fn push_audio(&self, order_num: u32, pcm: &[f32]); pub fn finalize(&self, order_num: u32, ticket: &Ticket) -> anyhow::Result<CheckReport>; pub fn send(&self, order_num: u32, as_rung: bool) -> anyhow::Result<OrderRecord>; pub fn cancel(&self, order_num: u32); pub fn expire(&self, now: Instant) -> Vec<u32>; pub fn has_transcript(&self, order_num: u32) -> bool }`. Order numbers start at 1001. One active order at a time is enough, but state is keyed by order_num. Completed segments are transcribed on a worker thread as they arrive. `finalize` flushes, transcribes only the remainder, then verifies. `send` writes the record (`fixed` = the first finalize had mismatches && the last finalize was Match && !as_rung; `caught`/`kinds`/`pairs` come from the first finalize) and drops the transcript. `cancel`/`expire` drop it with no record. The abandon timeout is 10 minutes. If the mic fails on `start`, the order still starts, the clip state is `Offline`, and `finalize` returns outcome Unconfirmed with `reason = Some("clip offline")`.
  - `pub enum Event { Clip { order_num: u32, state: ClipState }, Transcript { order_num: u32, text: String }, Ready { order_num: u32 } }` (serde, tagged `type`). `Transcript` events are emitted only when `--demo-transcript` is on.
  - `server.rs`: `pub fn router(state: AppState) -> axum::Router`. Routes:
    - `GET /api/menu` → the Menu JSON
    - `POST /api/orders` `{mode, label}` → `{order_num}`
    - `POST /api/orders/:n/finalize` `Ticket` → `CheckReport` without `transcript` (the `transcript` field is skipped in HTTP responses)
    - `POST /api/orders/:n/send` `{as_rung}` → 204, then a `Ready` event after `--ready-after` seconds (default 20)
    - `POST /api/orders/:n/cancel` → 204
    - `GET /api/dashboard?hours=24` → `Dashboard`
    - `GET /ws` → the event stream
    - static `web/` at `/` (`/pos.html`, `/dashboard.html`)
    - A ticket that fails `validate` → 400 with the error text. An unknown order → 404.
  - `main.rs serve --menu --model --db data/records.db --port 7080 [--standards] [--demo-transcript] [--ready-after 20]`; spawns a 30 s `expire` ticker.

- [ ] **Step 1: Failing tests** (`tests/session.rs`, with `FakeTranscriber` + `NoMic` or a fake `AudioSource` that pushes nothing; feed audio via `push_audio` with synthetic tone segments so the segmenter emits segments, while the fake transcriber supplies text):
  - `finalize_then_send_deletes_transcript`: start → push → finalize (Mismatch) → `has_transcript` true → finalize with the fixed ticket (Match) → send → `has_transcript` false; the record has `fixed == true`, `caught == 2`.
  - `cancel_writes_no_record`: start → cancel → the dashboard has 0 orders.
  - `abandoned_order_expires` **(Review Focus)**: start → `expire(now + 11 min)` returns [1001] → `has_transcript(1001)` false → 0 records.
  - `mic_offline_still_works` **(Review Focus)**: `NoMic` → start Ok → finalize → outcome Unconfirmed, `reason == Some("clip offline")` → send Ok.
  - `tests/server.rs`: `finalize_rejects_unknown_item` → 400; `menu_endpoint` → 200 with `items` non-empty; `finalize_response_has_no_transcript` → the JSON has no `transcript` key.
- [ ] **Step 2: Run** `cargo test --test session --test server`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** the same tests. Expected: PASS.
- [ ] **Step 5: Manual check:** `cargo run --release -- serve --menu menus/chick-fil-a.toml --model models/ggml-large-v3-turbo-q5_0.bin --demo-transcript`, then `curl -X POST localhost:7080/api/orders -H 'content-type: application/json' -d '{"mode":"face2face","label":"Test"}'` returns `{"order_num":1001}` and the macOS mic indicator turns on; `.../cancel` turns it off.
- [ ] **Step 6: Commit** `feat: live order sessions and local API server`.

---

### Task 10: Mock POS client

**Files:** Create `web/pos.html` (single file, vanilla JS)

**Interfaces:**
- Consumes: the Task 9 API and `/ws` events; the menu from `/api/menu`.

Port the spike (`.superpowers/brainstorm/*/content/mock-pos.html`) and replace its hard-coded `MENU`, `SCENARIOS`, and in-page `compare()` with the server. Keep: the start flow (car color/type → `POST /api/orders {mode:"drive_thru", label:"Navy SUV"}`; guest name → `face2face`), categories built from `menu.items[].category`, S/M/L, the MEAL toggle (meal = entrée + default side + the next drink tapped), quantity ▲▼, cancel item, Finalize → `POST finalize` with the ticket JSON (Task 5 shape), the mismatch alert with "Fix it" / "Send as rung", **a quiet yellow notice for `unconfirmed` (no alert modal)**, send → `POST send`, the clip indicator driven by `Clip` events, a "food ready" buzz on `Ready`, and the live transcript panel shown only when `Transcript` events arrive. **New:** a "Modifiers" button that opens the selected ticket line's allowed modifiers (from `menu.items[].modifiers`, plus the meal drink's modifiers when a meal is selected); chosen modifiers appear as indented red sub-lines like the real POS. No brand logos.

- [ ] **Step 1:** Build the page.
- [ ] **Step 2: Verify in a browser** (Playwright) against `serve` running with `--model models/ggml-base.en.bin`: start a drive-thru order, ring a chicken sandwich meal + sweet tea + 1 Polynesian + No Pickles, Finalize → the page shows a result (with silence, expect the quiet "couldn't confirm" notice rather than an alert), send → the clip state goes to Ok, then Ready. There are no console errors.
- [ ] **Step 3: Commit** `feat: mock POS client`.

---

### Task 11: Restaurant dashboard page

**Files:** Create `web/dashboard.html`

**Interfaces:** Consumes `GET /api/dashboard?hours=N`.

Shows: KPI tiles (orders, % orders with a catch, mistakes caught, fix rate, p50 latency); mistakes by hour (bars); mistakes by type (bars); "most mixed-up items" table (said → rang, count); and, when present, standards rates. A 24h / 7d toggle. No employee data anywhere. Add `serve --seed-demo-data` to insert ~300 plausible synthetic records over the past 7 days (clearly flagged `mode = "demo"`) so the dashboard can be screenshotted before real use; the dashboard shows a "demo data" badge when any demo records are present.

- [ ] **Step 1: Failing test** (`records.rs`): `seed_demo_marks_mode` → after `seed_demo(&records, 300)`, the dashboard `orders == 300` and every record has `mode == "demo"`.
- [ ] **Step 2: Run** `cargo test records`. Expected: FAIL. **Step 3:** implement `pub fn seed_demo(records: &Records, n: u32) -> anyhow::Result<()>` and the page. **Step 4:** run the tests (PASS), and check in the browser that the page renders with seeded data and has no console errors.
- [ ] **Step 5: Commit** `feat: restaurant dashboard`.

---

### Task 12: Test-set generator + eval harness + tuning to the bar

**Files:** Create `tools/conversations.py`, `tools/gen_testset.py`, `src/eval.rs`; modify `src/main.rs` (add `eval`).

**Interfaces:**
- `eval/cases/<id>/case.json`: `{ "id", "situation": "face_to_face"|"headset", "noise": "clean"|"snr15"|"snr5", "truth": Ticket, "variants": [ { "name": "correct"|<kind>, "ticket": Ticket, "planted": [MismatchKind] } ] }` + `audio.wav` (16 kHz mono).
- `pub struct EvalReport { pub cases: u32, pub extraction_accuracy: f32, pub catch_rate: f32, pub false_alarm_rate: f32, pub unconfirmed_rate: f32, pub p50_finalize_ms: u32, pub p95_finalize_ms: u32, pub by_kind: BTreeMap<String,(u32,u32)> /* caught, planted */, pub by_situation: BTreeMap<String,(f32,f32)> /* catch, false alarm */, pub by_noise: BTreeMap<String,(f32,f32)> }`
- `pub fn run_eval(menu: &Menu, stt: &dyn Transcriber, cases_dir: &Path) -> anyhow::Result<EvalReport>`. Definitions: extraction correct = `verify(spoken, truth)` is Match. Caught = outcome Mismatch AND at least one mismatch of the planted kind. False alarm = outcome Mismatch on a `correct` variant. Finalize ms = time to transcribe the **last** segment + extract + verify (earlier segments are transcribed during the order in live mode).
- `main.rs eval --menu --model --cases eval/cases --out eval/report` writes `report.json` and `report.md` (tables + a one-paragraph summary, plus the model name).

`tools/conversations.py`: at least 30 scripted conversations as data (`id`, `lines: [("c"|"t", text)]`, `truth` ticket dict, `has_readback: bool`), covering: meals with drink sizes; sauce counts 1–4; every look-alike family (sandwich variants, 8/12 ct, grilled vs breaded, tea/lemonade variants, shakes); modifiers on sandwiches and drinks; corrections (size, variant, make-it-a-meal); a removal; the LTO offer accepted and declined; unspecified qty/size; multi-item orders of 4+ lines; and employee read-backs (required in ≥ 20 conversations so headset cases exist).

`tools/gen_testset.py`: for each conversation × situation (`face_to_face` = all lines; `headset` = employee lines only, only for `has_readback`) × noise (`clean`, `snr15`, `snr5`): render each line with `say -v <voice>` (rotate customer voices across Samantha, Daniel, Karen, Moira, Fred; employee voices across Alex, Victoria, Tessa; skip voices not installed, per `say -v ?`), join them with 400–900 ms random gaps, convert to 16 kHz mono with ffmpeg, and mix in noise (ffmpeg `anoisesrc=color=pink` blended with a quiet overlay of unrelated `say` speech as "babble") at the target SNR. Variants: `correct` + one planted mutation per kind that applies to the truth ticket (variant: swap to another item in the family; count: ±1 on a qty line, preferring sauces; size: change a sized item's size; modifier: drop a truth modifier or add an allowed one; missing: drop a line; extra: add a cookie or a sauce not in the truth). Seeded RNG (`--seed 7`). Writes `eval/cases/`.

- [ ] **Step 1: Failing test** `eval::tests::eval_on_fake_cases`: two hand-written cases in `tests/fixtures/eval_cases/` (case.json + a 1 s silent wav) with a `FakeTranscriber` returning the scripted text → `catch_rate == 1.0`, `false_alarm_rate == 0.0`.
- [ ] **Step 2: Run** `cargo test eval`. Expected: FAIL. **Step 3:** implement `run_eval` + the `eval` subcommand. **Step 4:** run the tests (PASS).
- [ ] **Step 5: Generate the set:** `python3 tools/gen_testset.py --out eval/cases --seed 7`. Expected: ≥ 150 cases written; spot-check 3 wavs by running `check` on them.
- [ ] **Step 6: Run the eval** with both models: `cargo run --release -- eval ... --model models/ggml-base.en.bin --out eval/report-base` and `... ggml-large-v3-turbo-q5_0.bin --out eval/report-turbo`.
- [ ] **Step 7: Tune to the bar** (catch ≥ 90%, false alarms ≤ 5%, p95 finalize ≤ 1000 ms on the chosen model). For each failure category in the report, fix the cause in the matcher/extractor/menu aliases/verify, with a regression unit test added in the owning module's tests for every fix. Re-run Step 6. Stop once the bar is met, or after the remaining failures are documented in `report.md` with their cause (e.g., "headset mishearing is undetectable by design").
- [ ] **Step 8: Commit** `feat: synthetic test set and eval harness` (cases and audio are gitignored; commit the scripts and the final `report.md` copied to `docs/results.md`).

---

### Task 13: README + demo instructions

**Files:** Create `README.md`

Contents: the pitch (AI that keeps jobs), how it works (diagram from the spec §7), the privacy guarantees (spec §5), setup (`brew install rustup cmake ffmpeg`, `tools/download_models.sh`, `cargo build --release`), `serve` / `check` / `eval` usage, the results table from `docs/results.md`, known limitations (the headset mishearing case, the duplicate-without-"another" case, synthetic voices), and next steps (spec phase 2). Also how to add a real recorded case (record with QuickTime → `ffmpeg -ar 16000 -ac 1` → write `case.json`).

- [ ] **Step 1:** Write it. **Step 2:** Follow the setup section verbatim in a fresh shell to confirm the commands work. **Step 3: Commit** `docs: README with results and demo instructions`.
