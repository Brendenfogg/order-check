# Order Check: evaluation results

Measured 2026-10-23 on an Apple M1 Pro (16 GB), Whisper running locally on the GPU through whisper.cpp.

**Test set:** 32 scripted Chick-fil-A orders (tools/conversations.py) spoken by 16 macOS voices. Each order comes in two situations: face-to-face (guest and employee both heard) and headset (only the employee's read-back is heard). Each situation is recorded at three noise levels: clean, 15 dB SNR, and 5 dB SNR, using pink noise plus background chatter. That's 177 orders. Every order is checked against its correct ticket (any alarm there is a false alarm) and against one ticket per kind of planted mistake (variant, count, size, modifier, missing, extra) that applies to it, for **1,107 checks**. Regenerate with `python3 tools/gen_testset.py --seed 7`.

**Caught** means the check raised a confident mismatch of the planted kind. "Couldn't confirm" counts as not caught. **Finalize → result** is the time to transcribe the last utterance plus the check itself. Earlier utterances are transcribed while the order is still being taken.

## Results

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

The turbo latency was measured while other jobs shared the GPU. Run alone, it was about 1.4 s median and 1.9 s p95, which is still over the 1 s target.

## What limits it

- **Noise, not logic.** On clean audio the spoken order is read correctly most of the time, and the remaining misses are mostly Whisper mishearing ("Strip's Wheel" for "strips meal"). At 5 dB the background is nearly as loud as the speaker, and whole items get lost.
- **Headset mode** has half the speech to work with, because only the employee's read-back is heard.
- **Not hearing something is weak evidence.** When the ticket has something that wasn't heard ("Rang 1 Cookie, guest didn't ask") or a meal whose "meal" wasn't heard, the check only buzzes when the whole transcript came through clearly (confidence ≥ 0.75). Otherwise it says "couldn't confirm." That trade keeps clean-audio false alarms near 3%, at the cost of some catches in noise.
- **Overfitting.** See the held-out set above.
- **Synthetic voices.** The next measurement is real recorded orders.

## Full report (default model, tuning set)


177 orders, 1107 ticket checks. The check caught **79.4%** of planted mistakes, raised a false alarm on **18.6%** of correct tickets, and returned a result in **381 ms** (median; p95 512 ms) after Finalize.

| Metric | Value | Target |
|---|---|---|
| Planted mistakes caught | 79.4% | ≥ 90% |
| False alarms on correct tickets | 18.6% | ≤ 5% |
| Finalize → result, p95 | 512 ms | ≤ 1000 ms |
| Spoken order read exactly right | 66.1% | — |
| Checks returned "couldn't confirm" | 13.6% | — |

## By mistake type

| Type | Caught | Planted | Rate |
|---|---|---|---|
| count | 143 | 177 | 80.8% |
| extra | 120 | 177 | 67.8% |
| missing | 138 | 150 | 92.0% |
| modifier | 110 | 141 | 78.0% |
| size | 100 | 135 | 74.1% |
| variant | 127 | 150 | 84.7% |

## By audio situation

| | Caught | False alarms |
|---|---|---|
| face_to_face | 88.6% | 14.6% |
| headset | 68.8% | 23.5% |

## By background noise

| | Caught | False alarms |
|---|---|---|
| clean | 96.1% | 3.4% |
| snr15 | 84.8% | 13.6% |
| snr5 | 57.1% | 39.0% |

## Misses and false alarms (285)

- big-sauce-count--headset--snr5: extraction differs from truth (Said 1 Chick-fil-A Sauce, rang 4; Said 1 Polynesian, rang 2) — heard "Anything Else, Thought Chick-fil-A Sauce, Deep Polynesian"
- big-sauce-count--headset--snr5: missed planted count (Unconfirmed) — heard "Anything Else, Thought Chick-fil-A Sauce, Deep Polynesian"
- big-sauce-count--headset--snr5: missed planted extra (Unconfirmed) — heard "Anything Else, Thought Chick-fil-A Sauce, Deep Polynesian"
- correction-size--face_to_face--snr5: extraction differs from truth (Rang 1 Medium Waffle Fries, guest didn't ask) — heard "What night that started for me? Night that honeymoon, Sunjoy, but to move with the love | of love"
- correction-size--face_to_face--snr5: missed planted count (Unconfirmed) — heard "What night that started for me? Night that honeymoon, Sunjoy, but to move with the love | of love"
- correction-size--face_to_face--snr5: missed planted size (Unconfirmed) — heard "What night that started for me? Night that honeymoon, Sunjoy, but to move with the love | of love"
- correction-size--face_to_face--snr5: missed planted modifier (Unconfirmed) — heard "What night that started for me? Night that honeymoon, Sunjoy, but to move with the love | of love"
- correction-size--face_to_face--snr5: missed planted extra (Unconfirmed) — heard "What night that started for me? Night that honeymoon, Sunjoy, but to move with the love | of love"
- correction-size--headset--snr5: extraction differs from truth (Rang 1 Medium Waffle Fries, guest didn't ask) — heard "What do I get started for you? A large sunjoy? Anything else? | A large sunjoy? Anything else?"
- correction-size--headset--snr5: false alarm on correct ticket (Rang 1 Medium Waffle Fries, guest didn't ask) — heard "What do I get started for you? A large sunjoy? Anything else? | A large sunjoy? Anything else?"
- correction-size--headset--snr5: missed planted size (Mismatch) — heard "What do I get started for you? A large sunjoy? Anything else? | A large sunjoy? Anything else?"
- correction-size--headset--snr5: missed planted extra (Unconfirmed) — heard "What do I get started for you? A large sunjoy? Anything else? | A large sunjoy? Anything else?"
- correction-variant--face_to_face--snr15: extraction differs from truth (Said 1 Spicy Chicken Sandwich, not on ticket) — heard "Hi, What can I get for you today? Let me get a Spicy Chicken Sandwich, No Wave, Regular, Not Spicy | Okay, a Regular Chicken Sandwich | and 4 Polynesian Sauces | My pleasure"
- correction-variant--face_to_face--snr15: false alarm on correct ticket (Said 1 Spicy Chicken Sandwich, not on ticket) — heard "Hi, What can I get for you today? Let me get a Spicy Chicken Sandwich, No Wave, Regular, Not Spicy | Okay, a Regular Chicken Sandwich | and 4 Polynesian Sauces | My pleasure"
- correction-variant--face_to_face--snr5: extraction differs from truth (Said 1 Spicy Chicken Sandwich, not on ticket; Rang 4 Polynesian, guest didn't ask) — heard "Honey, What Can I Get For You Today? Let Me Get A Spicy Chicken Sandwich, No Wave, Regular, Not Spicy | Okay, A regular Chicken Sandwich | I'm full for the new Hymn Saucer"
- correction-variant--face_to_face--snr5: false alarm on correct ticket (Said 1 Spicy Chicken Sandwich, not on ticket; Rang 4 Polynesian, guest didn't ask) — heard "Honey, What Can I Get For You Today? Let Me Get A Spicy Chicken Sandwich, No Wave, Regular, Not Spicy | Okay, A regular Chicken Sandwich | I'm full for the new Hymn Saucer"
- correction-variant--face_to_face--snr5: missed planted count (Mismatch) — heard "Honey, What Can I Get For You Today? Let Me Get A Spicy Chicken Sandwich, No Wave, Regular, Not Spicy | Okay, A regular Chicken Sandwich | I'm full for the new Hymn Saucer"
- correction-variant--face_to_face--snr5: missed planted extra (Mismatch) — heard "Honey, What Can I Get For You Today? Let Me Get A Spicy Chicken Sandwich, No Wave, Regular, Not Spicy | Okay, A regular Chicken Sandwich | I'm full for the new Hymn Saucer"
- correction-variant--headset--snr5: extraction differs from truth (Said 1 Polynesian, rang 4) — heard "Hi, what can I get for you today? Okay, a regular chicken sandwich, a chicken sandwich and full Polynesian, my pleasure"
- correction-variant--headset--snr5: false alarm on correct ticket (Said 1 Polynesian, rang 4) — heard "Hi, what can I get for you today? Okay, a regular chicken sandwich, a chicken sandwich and full Polynesian, my pleasure"
- deluxe-meal-large--face_to_face--snr5: extraction differs from truth (Said DELUXE CHICKEN SANDWICH, rang DELUXE CHICKEN SANDWICH MEAL; Rang No Tomato on Deluxe Chicken Sandwich Meal, guest didn't ask; Rang 1 Large Waffle Fries, guest didn't ask) — heard "Add, Letty Milkhead Cream, Deluxe Chicken Sandwich, Grilled with Box of Pepper, Milk Tomato, | Lost Whip, No Dr Pepper, Milk Tomato,"
- deluxe-meal-large--face_to_face--snr5: false alarm on correct ticket (Said DELUXE CHICKEN SANDWICH, rang DELUXE CHICKEN SANDWICH MEAL; Rang No Tomato on Deluxe Chicken Sandwich Meal, guest didn't ask; Rang 1 Large Waffle Fries, guest didn't ask) — heard "Add, Letty Milkhead Cream, Deluxe Chicken Sandwich, Grilled with Box of Pepper, Milk Tomato, | Lost Whip, No Dr Pepper, Milk Tomato,"
- deluxe-meal-large--face_to_face--snr5: missed planted size (Mismatch) — heard "Add, Letty Milkhead Cream, Deluxe Chicken Sandwich, Grilled with Box of Pepper, Milk Tomato, | Lost Whip, No Dr Pepper, Milk Tomato,"
- deluxe-meal-large--face_to_face--snr5: missed planted modifier (Mismatch) — heard "Add, Letty Milkhead Cream, Deluxe Chicken Sandwich, Grilled with Box of Pepper, Milk Tomato, | Lost Whip, No Dr Pepper, Milk Tomato,"
- deluxe-meal-large--headset--snr15: extraction differs from truth (Rang 1 Deluxe Chicken Sandwich Meal, guest didn't ask; Rang 1 Large Waffle Fries, guest didn't ask) — heard "Hi, what time I get for you? | Watch what's new with Dr Pepper, No Tomato."
- deluxe-meal-large--headset--snr15: missed planted variant (Unconfirmed) — heard "Hi, what time I get for you? | Watch what's new with Dr Pepper, No Tomato."
- deluxe-meal-large--headset--snr15: missed planted size (Unconfirmed) — heard "Hi, what time I get for you? | Watch what's new with Dr Pepper, No Tomato."
- deluxe-meal-large--headset--snr15: missed planted modifier (Unconfirmed) — heard "Hi, what time I get for you? | Watch what's new with Dr Pepper, No Tomato."
- deluxe-meal-large--headset--snr15: missed planted extra (Unconfirmed) — heard "Hi, what time I get for you? | Watch what's new with Dr Pepper, No Tomato."
- deluxe-meal-large--headset--snr5: extraction differs from truth (Rang 1 Deluxe Chicken Sandwich Meal, guest didn't ask; Rang 1 Large Waffle Fries, guest didn't ask; Rang 1 Large Dr Pepper, guest didn't ask) — heard "Ah, What's in my head for me? Watch what you know you did not forever, No Tomato"
- deluxe-meal-large--headset--snr5: missed planted variant (Unconfirmed) — heard "Ah, What's in my head for me? Watch what you know you did not forever, No Tomato"
- deluxe-meal-large--headset--snr5: missed planted count (Unconfirmed) — heard "Ah, What's in my head for me? Watch what you know you did not forever, No Tomato"
- deluxe-meal-large--headset--snr5: missed planted size (Unconfirmed) — heard "Ah, What's in my head for me? Watch what you know you did not forever, No Tomato"
- deluxe-meal-large--headset--snr5: missed planted modifier (Unconfirmed) — heard "Ah, What's in my head for me? Watch what you know you did not forever, No Tomato"
- deluxe-meal-large--headset--snr5: missed planted extra (Unconfirmed) — heard "Ah, What's in my head for me? Watch what you know you did not forever, No Tomato"
- deluxe-no-tomato--headset--snr15: missed planted variant (Match) — heard "What can I get started for you? A Deluxe Sandwich, No Tomago, and a Small One's Meat Tea | Anything Else? | My Pleasure"
- deluxe-no-tomato--headset--snr15: missed planted size (Match) — heard "What can I get started for you? A Deluxe Sandwich, No Tomago, and a Small One's Meat Tea | Anything Else? | My Pleasure"
- deluxe-no-tomato--headset--snr5: missed planted variant (Match) — heard "What do I get started for you? A Deluxe Sandwich, No Tomato, and a Small One's Eat Tea"
- deluxe-no-tomato--headset--snr5: missed planted size (Match) — heard "What do I get started for you? A Deluxe Sandwich, No Tomato, and a Small One's Eat Tea"
- deluxe-no-tomato--headset--snr5: missed planted extra (Unconfirmed) — heard "What do I get started for you? A Deluxe Sandwich, No Tomato, and a Small One's Eat Tea"
- family-order--face_to_face--snr5: extraction differs from truth (Rang 1 12 ct Nuggets, guest didn't ask; Rang 3 Honey Mustard, guest didn't ask) — heard "No Pinta, Pinta Whoo, What Can I Get For You? Ok, We Need 2 Spicy Deluxe Meals, Both With Large Lemonades | Then a 12-Countain Ovets, A Large Mac & Cheese | Definitely a 20-Countain Ovets"
- family-order--face_to_face--snr5: missed planted count (Unconfirmed) — heard "No Pinta, Pinta Whoo, What Can I Get For You? Ok, We Need 2 Spicy Deluxe Meals, Both With Large Lemonades | Then a 12-Countain Ovets, A Large Mac & Cheese | Definitely a 20-Countain Ovets"
- family-order--face_to_face--snr5: missed planted modifier (Unconfirmed) — heard "No Pinta, Pinta Whoo, What Can I Get For You? Ok, We Need 2 Spicy Deluxe Meals, Both With Large Lemonades | Then a 12-Countain Ovets, A Large Mac & Cheese | Definitely a 20-Countain Ovets"
- family-order--face_to_face--snr5: missed planted extra (Unconfirmed) — heard "No Pinta, Pinta Whoo, What Can I Get For You? Ok, We Need 2 Spicy Deluxe Meals, Both With Large Lemonades | Then a 12-Countain Ovets, A Large Mac & Cheese | Definitely a 20-Countain Ovets"
- family-order--headset--snr15: extraction differs from truth (Said SPICY CHICKEN SANDWICH MEAL, rang SPICY DELUXE SANDWICH MEAL) — heard "Welcome to Chick-fil-A, what can I get for you? | 2 spicy looks meals with large lemonade, 12 ct Nuggets, large mac & cheese, 3 honey mustard,"
- family-order--headset--snr15: false alarm on correct ticket (Said SPICY CHICKEN SANDWICH MEAL, rang SPICY DELUXE SANDWICH MEAL) — heard "Welcome to Chick-fil-A, what can I get for you? | 2 spicy looks meals with large lemonade, 12 ct Nuggets, large mac & cheese, 3 honey mustard,"
- family-order--headset--snr5: extraction differs from truth (Said 1 Fruit Cup, not on ticket; Said 1 Lemonade, rang 2; Said 1 Honey Mustard, rang 3; Rang 2 Spicy Deluxe Sandwich Meal, guest didn't ask; Rang 1 12 ct Nuggets, guest didn't ask) — heard "No Pinky, Chick-fil-A, What's Am I Getting For You? Good Fighting, What's New With Large Lemonade, Well-Somed Murder, Large Mac & Cheese, Fruit Honey Mustard,"
- family-order--headset--snr5: false alarm on correct ticket (Said 1 Fruit Cup, not on ticket) — heard "No Pinky, Chick-fil-A, What's Am I Getting For You? Good Fighting, What's New With Large Lemonade, Well-Somed Murder, Large Mac & Cheese, Fruit Honey Mustard,"
- family-order--headset--snr5: missed planted variant (Mismatch) — heard "No Pinky, Chick-fil-A, What's Am I Getting For You? Good Fighting, What's New With Large Lemonade, Well-Somed Murder, Large Mac & Cheese, Fruit Honey Mustard,"
- family-order--headset--snr5: missed planted count (Mismatch) — heard "No Pinky, Chick-fil-A, What's Am I Getting For You? Good Fighting, What's New With Large Lemonade, Well-Somed Murder, Large Mac & Cheese, Fruit Honey Mustard,"
- family-order--headset--snr5: missed planted modifier (Mismatch) — heard "No Pinky, Chick-fil-A, What's Am I Getting For You? Good Fighting, What's New With Large Lemonade, Well-Somed Murder, Large Mac & Cheese, Fruit Honey Mustard,"
- family-order--headset--snr5: missed planted extra (Mismatch) — heard "No Pinky, Chick-fil-A, What's Am I Getting For You? Good Fighting, What's New With Large Lemonade, Well-Somed Murder, Large Mac & Cheese, Fruit Honey Mustard,"
- frosted-lemonade--face_to_face--snr5: extraction differs from truth (Said Cola, not on ticket) — heard "What, like that soda for me? 2 Frosted Lemonades and Vanilla Milkshake with no Cherry 2 Frosted Lemonades"
- frosted-lemonade--face_to_face--snr5: false alarm on correct ticket (Said Cola, not on ticket) — heard "What, like that soda for me? 2 Frosted Lemonades and Vanilla Milkshake with no Cherry 2 Frosted Lemonades"
- frosted-lemonade--face_to_face--snr5: missed planted count (Mismatch) — heard "What, like that soda for me? 2 Frosted Lemonades and Vanilla Milkshake with no Cherry 2 Frosted Lemonades"
- frosted-lemonade--face_to_face--snr5: missed planted extra (Mismatch) — heard "What, like that soda for me? 2 Frosted Lemonades and Vanilla Milkshake with no Cherry 2 Frosted Lemonades"
- frosted-lemonade--headset--snr15: extraction differs from truth (Rang 1 Vanilla Milkshake, guest didn't ask) — heard "What can I get started for you? | 2 Frosted Lemonades & Vanilla Say, No Cherry"
- frosted-lemonade--headset--snr15: false alarm on correct ticket (Rang 1 Vanilla Milkshake, guest didn't ask) — heard "What can I get started for you? | 2 Frosted Lemonades & Vanilla Say, No Cherry"
- frosted-lemonade--headset--snr15: missed planted variant (Mismatch) — heard "What can I get started for you? | 2 Frosted Lemonades & Vanilla Say, No Cherry"
- frosted-lemonade--headset--snr15: missed planted count (Mismatch) — heard "What can I get started for you? | 2 Frosted Lemonades & Vanilla Say, No Cherry"
- frosted-lemonade--headset--snr15: missed planted modifier (Mismatch) — heard "What can I get started for you? | 2 Frosted Lemonades & Vanilla Say, No Cherry"
- frosted-lemonade--headset--snr15: missed planted missing (Match) — heard "What can I get started for you? | 2 Frosted Lemonades & Vanilla Say, No Cherry"
- frosted-lemonade--headset--snr15: missed planted extra (Unconfirmed) — heard "What can I get started for you? | 2 Frosted Lemonades & Vanilla Say, No Cherry"
- frosted-lemonade--headset--snr5: extraction differs from truth (Said 1 Lemonade, not on ticket; Rang 2 Frosted Lemonade, guest didn't ask; Rang 1 Vanilla Milkshake, guest didn't ask) — heard "What do I get started for you? Who draws the lemonade into the new assay? No Cherry"
- frosted-lemonade--headset--snr5: false alarm on correct ticket (Said 1 Lemonade, not on ticket) — heard "What do I get started for you? Who draws the lemonade into the new assay? No Cherry"
- frosted-lemonade--headset--snr5: missed planted variant (Mismatch) — heard "What do I get started for you? Who draws the lemonade into the new assay? No Cherry"
- frosted-lemonade--headset--snr5: missed planted count (Mismatch) — heard "What do I get started for you? Who draws the lemonade into the new assay? No Cherry"
- frosted-lemonade--headset--snr5: missed planted modifier (Mismatch) — heard "What do I get started for you? Who draws the lemonade into the new assay? No Cherry"
- frosted-lemonade--headset--snr5: missed planted extra (Mismatch) — heard "What do I get started for you? Who draws the lemonade into the new assay? No Cherry"
- grilled-nuggets-8-ranch--face_to_face--snr15: extraction differs from truth (Said 1 Waffle Fries, not on ticket) — heard "Fry, What Can I Get For You? | An 8-pound Grilled Nuggets with a Garden Herb Ranch. | 8-pound Grilled Nuggets and 1 Garden Herb Ranch. | Anything for drink? | Just a medium diet lemonade. | You got it."
- grilled-nuggets-8-ranch--face_to_face--snr15: false alarm on correct ticket (Said 1 Waffle Fries, not on ticket) — heard "Fry, What Can I Get For You? | An 8-pound Grilled Nuggets with a Garden Herb Ranch. | 8-pound Grilled Nuggets and 1 Garden Herb Ranch. | Anything for drink? | Just a medium diet lemonade. | You got it."
- grilled-nuggets-8-ranch--face_to_face--snr15: missed planted modifier (Mismatch) — heard "Fry, What Can I Get For You? | An 8-pound Grilled Nuggets with a Garden Herb Ranch. | 8-pound Grilled Nuggets and 1 Garden Herb Ranch. | Anything for drink? | Just a medium diet lemonade. | You got it."
- grilled-nuggets-8-ranch--face_to_face--snr15: missed planted extra (Mismatch) — heard "Fry, What Can I Get For You? | An 8-pound Grilled Nuggets with a Garden Herb Ranch. | 8-pound Grilled Nuggets and 1 Garden Herb Ranch. | Anything for drink? | Just a medium diet lemonade. | You got it."
- grilled-nuggets-8-ranch--face_to_face--snr5: extraction differs from truth (Rang 1 Garden Herb Ranch, guest didn't ask; Rang 1 Medium Diet Lemonade, guest didn't ask) — heard "Ice, What Can I Get For You? 8-pound grilled nuggets in the pie-dough pan, 8-pound grilled nuggets in the one-dog-in-her-hand, | 8-pound grilled nuggets in the one-dog-in-her-hand,"
- grilled-nuggets-8-ranch--face_to_face--snr5: missed planted count (Unconfirmed) — heard "Ice, What Can I Get For You? 8-pound grilled nuggets in the pie-dough pan, 8-pound grilled nuggets in the one-dog-in-her-hand, | 8-pound grilled nuggets in the one-dog-in-her-hand,"
- grilled-nuggets-8-ranch--face_to_face--snr5: missed planted size (Unconfirmed) — heard "Ice, What Can I Get For You? 8-pound grilled nuggets in the pie-dough pan, 8-pound grilled nuggets in the one-dog-in-her-hand, | 8-pound grilled nuggets in the one-dog-in-her-hand,"
- grilled-nuggets-8-ranch--face_to_face--snr5: missed planted modifier (Unconfirmed) — heard "Ice, What Can I Get For You? 8-pound grilled nuggets in the pie-dough pan, 8-pound grilled nuggets in the one-dog-in-her-hand, | 8-pound grilled nuggets in the one-dog-in-her-hand,"
- grilled-nuggets-8-ranch--face_to_face--snr5: missed planted missing (Unconfirmed) — heard "Ice, What Can I Get For You? 8-pound grilled nuggets in the pie-dough pan, 8-pound grilled nuggets in the one-dog-in-her-hand, | 8-pound grilled nuggets in the one-dog-in-her-hand,"
- grilled-nuggets-8-ranch--face_to_face--snr5: missed planted extra (Unconfirmed) — heard "Ice, What Can I Get For You? 8-pound grilled nuggets in the pie-dough pan, 8-pound grilled nuggets in the one-dog-in-her-hand, | 8-pound grilled nuggets in the one-dog-in-her-hand,"
- grilled-nuggets-8-ranch--headset--snr5: extraction differs from truth (Rang 1 8 ct Grilled Nuggets, guest didn't ask; Rang 1 Garden Herb Ranch, guest didn't ask) — heard "Ah, what can I get for you? It sounds so bloody, someone got in your hand. Anything to drink? Medium Diet Lemonade, you got it."
- grilled-nuggets-8-ranch--headset--snr5: missed planted variant (Unconfirmed) — heard "Ah, what can I get for you? It sounds so bloody, someone got in your hand. Anything to drink? Medium Diet Lemonade, you got it."
- grilled-nuggets-8-ranch--headset--snr5: missed planted count (Unconfirmed) — heard "Ah, what can I get for you? It sounds so bloody, someone got in your hand. Anything to drink? Medium Diet Lemonade, you got it."
- grilled-nuggets-8-ranch--headset--snr5: missed planted modifier (Unconfirmed) — heard "Ah, what can I get for you? It sounds so bloody, someone got in your hand. Anything to drink? Medium Diet Lemonade, you got it."
- grilled-nuggets-8-ranch--headset--snr5: missed planted missing (Unconfirmed) — heard "Ah, what can I get for you? It sounds so bloody, someone got in your hand. Anything to drink? Medium Diet Lemonade, you got it."
- grilled-nuggets-8-ranch--headset--snr5: missed planted extra (Unconfirmed) — heard "Ah, what can I get for you? It sounds so bloody, someone got in your hand. Anything to drink? Medium Diet Lemonade, you got it."
- grilled-sandwich-meal--face_to_face--snr15: extraction differs from truth (Rang Add Lemon on Unsweet Tea, guest didn't ask; Said 1 Lemonade, not on ticket) — heard "A Grilled Chicken Sandwich Meal, but Sub Mac & Cheese for the price, and a medium unsweet tea with lemonade. | Grilled Chicken Sandwich Meal with Mac & Cheese and a medium unsweet tea with lemonade."
- grilled-sandwich-meal--face_to_face--snr15: false alarm on correct ticket (Rang Add Lemon on Unsweet Tea, guest didn't ask; Said 1 Lemonade, not on ticket) — heard "A Grilled Chicken Sandwich Meal, but Sub Mac & Cheese for the price, and a medium unsweet tea with lemonade. | Grilled Chicken Sandwich Meal with Mac & Cheese and a medium unsweet tea with lemonade."
- grilled-sandwich-meal--face_to_face--snr15: missed planted modifier (Mismatch) — heard "A Grilled Chicken Sandwich Meal, but Sub Mac & Cheese for the price, and a medium unsweet tea with lemonade. | Grilled Chicken Sandwich Meal with Mac & Cheese and a medium unsweet tea with lemonade."
- grilled-sandwich-meal--face_to_face--snr5: extraction differs from truth (Rang Add Lemon on Unsweet Tea, guest didn't ask; Said 1 Lemonade, not on ticket) — heard "Welcome, What Can I Get For You? A Grilled Chicken Sandwich Meal, But Sub Mac & Cheese for the Fries, And a Medium Unsweet Tea with Lemonade | Grilled Chicken Sandwich Meal with Mac & Cheese"
- grilled-sandwich-meal--face_to_face--snr5: false alarm on correct ticket (Rang Add Lemon on Unsweet Tea, guest didn't ask; Said 1 Lemonade, not on ticket) — heard "Welcome, What Can I Get For You? A Grilled Chicken Sandwich Meal, But Sub Mac & Cheese for the Fries, And a Medium Unsweet Tea with Lemonade | Grilled Chicken Sandwich Meal with Mac & Cheese"
- grilled-sandwich-meal--face_to_face--snr5: missed planted modifier (Mismatch) — heard "Welcome, What Can I Get For You? A Grilled Chicken Sandwich Meal, But Sub Mac & Cheese for the Fries, And a Medium Unsweet Tea with Lemonade | Grilled Chicken Sandwich Meal with Mac & Cheese"
- grilled-sandwich-meal--headset--snr5: extraction differs from truth (Rang Add Lemon on Unsweet Tea, guest didn't ask; Said 1 Lemonade, not on ticket) — heard "Welcome, what can I get for you? Grilled Chicken Sandwich meal with Mac & Cheese and a medium-unsweet tea with lemonade"
- grilled-sandwich-meal--headset--snr5: false alarm on correct ticket (Rang Add Lemon on Unsweet Tea, guest didn't ask; Said 1 Lemonade, not on ticket) — heard "Welcome, what can I get for you? Grilled Chicken Sandwich meal with Mac & Cheese and a medium-unsweet tea with lemonade"
- grilled-sandwich-meal--headset--snr5: missed planted modifier (Mismatch) — heard "Welcome, what can I get for you? Grilled Chicken Sandwich meal with Mac & Cheese and a medium-unsweet tea with lemonade"
- honey-roasted--face_to_face--snr5: extraction differs from truth (Said 8 CT NUGGETS, rang 8 CT NUGGETS MEAL) — heard "What do I get started for you? And 8 ct nugget me with a medium Lemonade and 2 Honey Roasted BBQ Sauces"
- honey-roasted--face_to_face--snr5: false alarm on correct ticket (Said 8 CT NUGGETS, rang 8 CT NUGGETS MEAL) — heard "What do I get started for you? And 8 ct nugget me with a medium Lemonade and 2 Honey Roasted BBQ Sauces"
- honey-roasted--headset--clean: extraction differs from truth (Rang 1 8 ct Nuggets Meal, guest didn't ask) — heard "What can I get started for you? | 8 Combs Knocking Meal, Meaty Lemonade, 2 Honey Roasted BBQ"
- honey-roasted--headset--clean: false alarm on correct ticket (Rang 1 8 ct Nuggets Meal, guest didn't ask) — heard "What can I get started for you? | 8 Combs Knocking Meal, Meaty Lemonade, 2 Honey Roasted BBQ"
- honey-roasted--headset--clean: missed planted size (Mismatch) — heard "What can I get started for you? | 8 Combs Knocking Meal, Meaty Lemonade, 2 Honey Roasted BBQ"
- honey-roasted--headset--snr15: missed planted variant (Match) — heard "What can I get started for you? | 8 ct Nugget Meal, Lead-Eweminate, 2 Honey Roasted BBQ,"
- honey-roasted--headset--snr15: missed planted size (Match) — heard "What can I get started for you? | 8 ct Nugget Meal, Lead-Eweminate, 2 Honey Roasted BBQ,"
- honey-roasted--headset--snr15: missed planted extra (Unconfirmed) — heard "What can I get started for you? | 8 ct Nugget Meal, Lead-Eweminate, 2 Honey Roasted BBQ,"
- honey-roasted--headset--snr5: missed planted extra (Unconfirmed) — heard "Look, I got started for you. 8 ct Nugget Meal, medium Lemonade, 2 Honey Roasted BBQ,"
- kids-shakes--face_to_face--snr5: extraction differs from truth (Said 1 Cookies & Cream Milkshake, rang 2; Rang 1 Strawberry Milkshake, guest didn't ask) — heard "Welcome, I am Dr. Smallbun Wilford and the Fucking Lincoln Wilford | is probably taken to the cookies and cream here | The one flocking"
- kids-shakes--face_to_face--snr5: false alarm on correct ticket (Said 1 Cookies & Cream Milkshake, rang 2; Rang 1 Strawberry Milkshake, guest didn't ask) — heard "Welcome, I am Dr. Smallbun Wilford and the Fucking Lincoln Wilford | is probably taken to the cookies and cream here | The one flocking"
- kids-shakes--face_to_face--snr5: missed planted variant (Mismatch) — heard "Welcome, I am Dr. Smallbun Wilford and the Fucking Lincoln Wilford | is probably taken to the cookies and cream here | The one flocking"
- kids-shakes--face_to_face--snr5: missed planted modifier (Mismatch) — heard "Welcome, I am Dr. Smallbun Wilford and the Fucking Lincoln Wilford | is probably taken to the cookies and cream here | The one flocking"
- kids-shakes--face_to_face--snr5: missed planted missing (Mismatch) — heard "Welcome, I am Dr. Smallbun Wilford and the Fucking Lincoln Wilford | is probably taken to the cookies and cream here | The one flocking"
- kids-shakes--face_to_face--snr5: missed planted extra (Mismatch) — heard "Welcome, I am Dr. Smallbun Wilford and the Fucking Lincoln Wilford | is probably taken to the cookies and cream here | The one flocking"
- kids-shakes--headset--snr15: missed planted modifier (Unconfirmed) — heard "Welcome | A Strawberry Shake & The Cookies & Cream Shake | So One Strawberry & Two Cookies & Cream"
- kids-shakes--headset--snr15: missed planted extra (Unconfirmed) — heard "Welcome | A Strawberry Shake & The Cookies & Cream Shake | So One Strawberry & Two Cookies & Cream"
- kids-shakes--headset--snr5: extraction differs from truth (Said 1 Cookie, not on ticket; Rang 1 Strawberry Milkshake, guest didn't ask; Rang 2 Cookies & Cream Milkshake, guest didn't ask) — heard "Golden, A Swabbering Chicken with Cookie and Cream too, The last Swabbering and Good Cookie and Cream"
- kids-shakes--headset--snr5: false alarm on correct ticket (Said 1 Cookie, not on ticket) — heard "Golden, A Swabbering Chicken with Cookie and Cream too, The last Swabbering and Good Cookie and Cream"
- kids-shakes--headset--snr5: missed planted variant (Mismatch) — heard "Golden, A Swabbering Chicken with Cookie and Cream too, The last Swabbering and Good Cookie and Cream"
- kids-shakes--headset--snr5: missed planted count (Mismatch) — heard "Golden, A Swabbering Chicken with Cookie and Cream too, The last Swabbering and Good Cookie and Cream"
- kids-shakes--headset--snr5: missed planted modifier (Mismatch) — heard "Golden, A Swabbering Chicken with Cookie and Cream too, The last Swabbering and Good Cookie and Cream"
- kids-shakes--headset--snr5: missed planted extra (Unconfirmed) — heard "Golden, A Swabbering Chicken with Cookie and Cream too, The last Swabbering and Good Cookie and Cream"
- lemonade-variant--headset--clean: missed planted modifier (Unconfirmed) — heard "Hi, What Can I Get For You? | One Diet Lemonade and one Regular Lemonade, both medium."
- lemonade-variant--headset--clean: missed planted extra (Unconfirmed) — heard "Hi, What Can I Get For You? | One Diet Lemonade and one Regular Lemonade, both medium."
- lemonade-variant--headset--snr5: missed planted modifier (Unconfirmed) — heard "Ah, what can I get for you? One Diet Lemonade and one regular Lemonade, both medium"
- lemonade-variant--headset--snr5: missed planted extra (Unconfirmed) — heard "Ah, what can I get for you? One Diet Lemonade and one regular Lemonade, both medium"
- lto-accepted--face_to_face--snr5: missed planted size (Match) — heard "Hi, would you like to try our Seasoned Peach Milkshake today? Sure, I'll try it! Great, anything else? | And a 3-count Strip's meal with a small sweetie! | Peach Milkshake and a 3-count | Strip's meal with a small sweetie!"
- lto-accepted--headset--snr5: extraction differs from truth (Rang 1 3 ct Strips Meal, guest didn't ask) — heard "Fine, Would You Like To Try Our Seasonal Peach Milkshake Today? Great, Anything Else? A Peach Milkshake and a 3 pound Stip Smell with a Smell Sweet Tea, My Pleasure"
- lto-accepted--headset--snr5: missed planted variant (Unconfirmed) — heard "Fine, Would You Like To Try Our Seasonal Peach Milkshake Today? Great, Anything Else? A Peach Milkshake and a 3 pound Stip Smell with a Smell Sweet Tea, My Pleasure"
- lto-accepted--headset--snr5: missed planted size (Unconfirmed) — heard "Fine, Would You Like To Try Our Seasonal Peach Milkshake Today? Great, Anything Else? A Peach Milkshake and a 3 pound Stip Smell with a Smell Sweet Tea, My Pleasure"
- lto-accepted--headset--snr5: missed planted modifier (Unconfirmed) — heard "Fine, Would You Like To Try Our Seasonal Peach Milkshake Today? Great, Anything Else? A Peach Milkshake and a 3 pound Stip Smell with a Smell Sweet Tea, My Pleasure"
- lto-accepted--headset--snr5: missed planted extra (Unconfirmed) — heard "Fine, Would You Like To Try Our Seasonal Peach Milkshake Today? Great, Anything Else? A Peach Milkshake and a 3 pound Stip Smell with a Smell Sweet Tea, My Pleasure"
- mac-and-sandwich--headset--snr5: extraction differs from truth (Rang 1 Medium Mac & Cheese, guest didn't ask) — heard "Fire, Flipping or Good For You, Chicken Sandwich, Add Cheese, No Pickles, and the Medium-Muggin Cream"
- mac-and-sandwich--headset--snr5: missed planted count (Unconfirmed) — heard "Fire, Flipping or Good For You, Chicken Sandwich, Add Cheese, No Pickles, and the Medium-Muggin Cream"
- mac-and-sandwich--headset--snr5: missed planted size (Unconfirmed) — heard "Fire, Flipping or Good For You, Chicken Sandwich, Add Cheese, No Pickles, and the Medium-Muggin Cream"
- mac-and-sandwich--headset--snr5: missed planted missing (Match) — heard "Fire, Flipping or Good For You, Chicken Sandwich, Add Cheese, No Pickles, and the Medium-Muggin Cream"
- mac-and-sandwich--headset--snr5: missed planted extra (Unconfirmed) — heard "Fire, Flipping or Good For You, Chicken Sandwich, Add Cheese, No Pickles, and the Medium-Muggin Cream"
- make-it-a-meal--face_to_face--snr5: missed planted size (Match) — heard "Elephant, What's in my book for me? Can I get an 8 ct nugget? Cool, Would you like to make it a meal? | Yeah, actually make it a meal, with a medium diet."
- make-it-a-meal--headset--snr15: extraction differs from truth (Rang 1 8 ct Nuggets Meal, guest didn't ask) — heard "Welcome, What Can I Get For You? | Sure, Would You Like To Make It A Meal? | It Sounds Like A Meal With The Medium Dr Pepper"
- make-it-a-meal--headset--snr15: false alarm on correct ticket (Rang 1 8 ct Nuggets Meal, guest didn't ask) — heard "Welcome, What Can I Get For You? | Sure, Would You Like To Make It A Meal? | It Sounds Like A Meal With The Medium Dr Pepper"
- make-it-a-meal--headset--snr15: missed planted variant (Mismatch) — heard "Welcome, What Can I Get For You? | Sure, Would You Like To Make It A Meal? | It Sounds Like A Meal With The Medium Dr Pepper"
- make-it-a-meal--headset--snr15: missed planted extra (Unconfirmed) — heard "Welcome, What Can I Get For You? | Sure, Would You Like To Make It A Meal? | It Sounds Like A Meal With The Medium Dr Pepper"
- make-it-a-meal--headset--snr5: extraction differs from truth (Rang 1 8 ct Nuggets Meal, guest didn't ask) — heard "Nothing, What's the matter with you? Do you like to make it a meal? It's how to make it meal with a meal in Dr Pepper"
- make-it-a-meal--headset--snr5: missed planted variant (Unconfirmed) — heard "Nothing, What's the matter with you? Do you like to make it a meal? It's how to make it meal with a meal in Dr Pepper"
- make-it-a-meal--headset--snr5: missed planted count (Unconfirmed) — heard "Nothing, What's the matter with you? Do you like to make it a meal? It's how to make it meal with a meal in Dr Pepper"
- make-it-a-meal--headset--snr5: missed planted size (Unconfirmed) — heard "Nothing, What's the matter with you? Do you like to make it a meal? It's how to make it meal with a meal in Dr Pepper"
- make-it-a-meal--headset--snr5: missed planted extra (Unconfirmed) — heard "Nothing, What's the matter with you? Do you like to make it a meal? It's how to make it meal with a meal in Dr Pepper"
- nugget12-lemonade-cfa--face_to_face--snr5: extraction differs from truth (Said 1 Chick-fil-A Sauce, rang 2) — heard "I Will, What Can I Get For You Today? Hi, Can I Get A 12 Count Nugget Meal With A Large Lemonade? Sure, Any Talk Anytime? | Do Chick-fil-A Sauce | Ok, Ok | Chick-fil-A Sauce"
- nugget12-lemonade-cfa--face_to_face--snr5: missed planted count (Match) — heard "I Will, What Can I Get For You Today? Hi, Can I Get A 12 Count Nugget Meal With A Large Lemonade? Sure, Any Talk Anytime? | Do Chick-fil-A Sauce | Ok, Ok | Chick-fil-A Sauce"
- nugget12-lemonade-cfa--face_to_face--snr5: missed planted extra (Unconfirmed) — heard "I Will, What Can I Get For You Today? Hi, Can I Get A 12 Count Nugget Meal With A Large Lemonade? Sure, Any Talk Anytime? | Do Chick-fil-A Sauce | Ok, Ok | Chick-fil-A Sauce"
- nugget12-lemonade-cfa--headset--snr15: extraction differs from truth (Said 12 CT NUGGETS, rang 12 CT NUGGETS MEAL) — heard "Hi there, what can I get for you today? | Sure, any sauces with that? | Okay, a 12-pound nugget New York large lemonade, and 2 Chick-fil-A sauces."
- nugget12-lemonade-cfa--headset--snr15: false alarm on correct ticket (Said 12 CT NUGGETS, rang 12 CT NUGGETS MEAL) — heard "Hi there, what can I get for you today? | Sure, any sauces with that? | Okay, a 12-pound nugget New York large lemonade, and 2 Chick-fil-A sauces."
- nugget12-lemonade-cfa--headset--snr5: extraction differs from truth (Said 1 Cup of Water, not on ticket; Rang 1 12 ct Nuggets Meal, guest didn't ask; Rang 2 Chick-fil-A Sauce, guest didn't ask) — heard "Fire, What's the most difficult thing? Whoo, Any water with that? Oh, Dude, the world out-looted me with flogged lemonade | and you took so many costs"
- nugget12-lemonade-cfa--headset--snr5: false alarm on correct ticket (Said 1 Cup of Water, not on ticket) — heard "Fire, What's the most difficult thing? Whoo, Any water with that? Oh, Dude, the world out-looted me with flogged lemonade | and you took so many costs"
- nugget12-lemonade-cfa--headset--snr5: missed planted variant (Mismatch) — heard "Fire, What's the most difficult thing? Whoo, Any water with that? Oh, Dude, the world out-looted me with flogged lemonade | and you took so many costs"
- nugget12-lemonade-cfa--headset--snr5: missed planted count (Mismatch) — heard "Fire, What's the most difficult thing? Whoo, Any water with that? Oh, Dude, the world out-looted me with flogged lemonade | and you took so many costs"
- nugget12-lemonade-cfa--headset--snr5: missed planted size (Mismatch) — heard "Fire, What's the most difficult thing? Whoo, Any water with that? Oh, Dude, the world out-looted me with flogged lemonade | and you took so many costs"
- nugget12-lemonade-cfa--headset--snr5: missed planted extra (Mismatch) — heard "Fire, What's the most difficult thing? Whoo, Any water with that? Oh, Dude, the world out-looted me with flogged lemonade | and you took so many costs"
- removal--face_to_face--snr5: missed planted modifier (Unconfirmed) — heard "Musty, I guess, sorry before you. A Cookie, a Fruit Cup, and a Medium Lemonade. Actually, never mind the cookie."
- removal--face_to_face--snr5: missed planted extra (Unconfirmed) — heard "Musty, I guess, sorry before you. A Cookie, a Fruit Cup, and a Medium Lemonade. Actually, never mind the cookie."
- removal--headset--snr15: missed planted count (Unconfirmed) — heard "What can I get started for you? | Okay, the Fruit Cup and the medium Lemonade."
- removal--headset--snr15: missed planted modifier (Unconfirmed) — heard "What can I get started for you? | Okay, the Fruit Cup and the medium Lemonade."
- removal--headset--snr15: missed planted extra (Unconfirmed) — heard "What can I get started for you? | Okay, the Fruit Cup and the medium Lemonade."
- removal--headset--snr5: extraction differs from truth (Rang 1 Medium Fruit Cup, guest didn't ask) — heard "What, like that starter for you? Okay, a Brewed Chocolate and Ewing Lemonade"
- removal--headset--snr5: false alarm on correct ticket (Rang 1 Medium Fruit Cup, guest didn't ask) — heard "What, like that starter for you? Okay, a Brewed Chocolate and Ewing Lemonade"
- removal--headset--snr5: missed planted count (Mismatch) — heard "What, like that starter for you? Okay, a Brewed Chocolate and Ewing Lemonade"
- removal--headset--snr5: missed planted size (Mismatch) — heard "What, like that starter for you? Okay, a Brewed Chocolate and Ewing Lemonade"
- removal--headset--snr5: missed planted missing (Match) — heard "What, like that starter for you? Okay, a Brewed Chocolate and Ewing Lemonade"
- removal--headset--snr5: missed planted extra (Unconfirmed) — heard "What, like that starter for you? Okay, a Brewed Chocolate and Ewing Lemonade"
- sandwich-add-cheese--face_to_face--snr5: extraction differs from truth (Said CHICKEN SANDWICH, rang CHICKEN SANDWICH MEAL) — heard "Hi, What can I get for you? The Chick-fil-A Sandwich, You Will Get the Tag, and a large bowl | of Chicken Sandwich Moo, Add Pepper Jack, Large Coke,"
- sandwich-add-cheese--face_to_face--snr5: missed planted count (Unconfirmed) — heard "Hi, What can I get for you? The Chick-fil-A Sandwich, You Will Get the Tag, and a large bowl | of Chicken Sandwich Moo, Add Pepper Jack, Large Coke,"
- sandwich-add-cheese--face_to_face--snr5: missed planted extra (Unconfirmed) — heard "Hi, What can I get for you? The Chick-fil-A Sandwich, You Will Get the Tag, and a large bowl | of Chicken Sandwich Moo, Add Pepper Jack, Large Coke,"
- sandwich-add-cheese--headset--snr5: extraction differs from truth (Said CHICKEN SANDWICH, rang CHICKEN SANDWICH MEAL; Rang 1 Large Cola, guest didn't ask) — heard "Hi, What can I get for you? Chicken Sandwich Mule, Add Pepper Jack, Large Cake"
- sandwich-add-cheese--headset--snr5: false alarm on correct ticket (Said CHICKEN SANDWICH, rang CHICKEN SANDWICH MEAL; Rang 1 Large Cola, guest didn't ask) — heard "Hi, What can I get for you? Chicken Sandwich Mule, Add Pepper Jack, Large Cake"
- sandwich-add-cheese--headset--snr5: missed planted size (Mismatch) — heard "Hi, What can I get for you? Chicken Sandwich Mule, Add Pepper Jack, Large Cake"
- sandwich-add-cheese--headset--snr5: missed planted extra (Mismatch) — heard "Hi, What can I get for you? Chicken Sandwich Mule, Add Pepper Jack, Large Cake"
- sauces-mix--face_to_face--snr5: extraction differs from truth (Said 1 Honey Mustard, not on ticket) — heard "Honey Mustard, Can I get 2 barbecue sauces and 1 zesty buffalo? 2 bar zesty, 1 zesty buffalo | and a medium waffle fries"
- sauces-mix--face_to_face--snr5: false alarm on correct ticket (Said 1 Honey Mustard, not on ticket) — heard "Honey Mustard, Can I get 2 barbecue sauces and 1 zesty buffalo? 2 bar zesty, 1 zesty buffalo | and a medium waffle fries"
- sauces-mix--face_to_face--snr5: missed planted extra (Mismatch) — heard "Honey Mustard, Can I get 2 barbecue sauces and 1 zesty buffalo? 2 bar zesty, 1 zesty buffalo | and a medium waffle fries"
- sauces-mix--headset--clean: extraction differs from truth (Rang 2 Barbeque, guest didn't ask; Rang 1 Medium Waffle Fries, guest didn't ask) — heard "Anything else for you today? | Super Bacon 1 Zesty Buffalo"
- sauces-mix--headset--clean: missed planted size (Unconfirmed) — heard "Anything else for you today? | Super Bacon 1 Zesty Buffalo"
- sauces-mix--headset--clean: missed planted extra (Unconfirmed) — heard "Anything else for you today? | Super Bacon 1 Zesty Buffalo"
- sauces-mix--headset--snr15: extraction differs from truth (Rang 2 Barbeque, guest didn't ask; Rang 1 Medium Waffle Fries, guest didn't ask) — heard "Anything else for you today? | 2 more seconds, 1 zest in Buffalo"
- sauces-mix--headset--snr15: missed planted size (Unconfirmed) — heard "Anything else for you today? | 2 more seconds, 1 zest in Buffalo"
- sauces-mix--headset--snr15: missed planted extra (Unconfirmed) — heard "Anything else for you today? | 2 more seconds, 1 zest in Buffalo"
- sauces-mix--headset--snr5: extraction differs from truth (Rang 2 Barbeque, guest didn't ask; Rang 1 Zesty Buffalo, guest didn't ask; Rang 1 Medium Waffle Fries, guest didn't ask) — heard "Anything else for me today? Who are getting one gift in both of us?"
- sauces-mix--headset--snr5: missed planted count (Unconfirmed) — heard "Anything else for me today? Who are getting one gift in both of us?"
- sauces-mix--headset--snr5: missed planted size (Unconfirmed) — heard "Anything else for me today? Who are getting one gift in both of us?"
- sauces-mix--headset--snr5: missed planted missing (Unconfirmed) — heard "Anything else for me today? Who are getting one gift in both of us?"
- sauces-mix--headset--snr5: missed planted extra (Unconfirmed) — heard "Anything else for me today? Who are getting one gift in both of us?"
- spicy-deluxe-extra-pickles--face_to_face--snr5: extraction differs from truth (Said No Tomato on Spicy Deluxe Sandwich, not on ticket; Rang No Lettuce on Spicy Deluxe Sandwich, guest didn't ask; Rang Light Ice on Sweet Tea, guest didn't ask) — heard "What can I give for you today? Like that, Spicy Deluxe is excellent for small lettuce | Spicy Deluxe, Extra Pickles, No Tomato | I'm looking forward to it | Large Sweet Tea, Rice"
- spicy-deluxe-extra-pickles--face_to_face--snr5: false alarm on correct ticket (Said No Tomato on Spicy Deluxe Sandwich, not on ticket) — heard "What can I give for you today? Like that, Spicy Deluxe is excellent for small lettuce | Spicy Deluxe, Extra Pickles, No Tomato | I'm looking forward to it | Large Sweet Tea, Rice"
- spicy-deluxe-extra-pickles--face_to_face--snr5: missed planted count (Mismatch) — heard "What can I give for you today? Like that, Spicy Deluxe is excellent for small lettuce | Spicy Deluxe, Extra Pickles, No Tomato | I'm looking forward to it | Large Sweet Tea, Rice"
- spicy-deluxe-extra-pickles--face_to_face--snr5: missed planted extra (Mismatch) — heard "What can I give for you today? Like that, Spicy Deluxe is excellent for small lettuce | Spicy Deluxe, Extra Pickles, No Tomato | I'm looking forward to it | Large Sweet Tea, Rice"
- spicy-meal-poly--face_to_face--snr5: extraction differs from truth (Said Cola, not on ticket) — heard "Welcome to Chick-fil-A, what can I get started for you? Can I get a Spicy Chicken Sandwich meal with a medium sweet tea? Absolutely, would you like to try our seasonal peach milk? | No fans, oh, and can I get 3 Polynesian Soda? | 3 Polynesian | Yes, that's it | Please put it further"
- spicy-meal-poly--face_to_face--snr5: false alarm on correct ticket (Said Cola, not on ticket) — heard "Welcome to Chick-fil-A, what can I get started for you? Can I get a Spicy Chicken Sandwich meal with a medium sweet tea? Absolutely, would you like to try our seasonal peach milk? | No fans, oh, and can I get 3 Polynesian Soda? | 3 Polynesian | Yes, that's it | Please put it further"
- spicy-meal-poly--headset--snr5: extraction differs from truth (Said SPICY CHICKEN SANDWICH, rang SPICY CHICKEN SANDWICH MEAL; Said 1 Side Salad, not on ticket; Rang 1 Medium Sweet Tea, guest didn't ask; Rang 3 Polynesian, guest didn't ask) — heard "Welcome to Chick-fil-A, what can I get started for you? Absolutely, would you like to try our seasonal Peach Milkshake today? | So that's a spicy chicken salad. | I'm going to try it. | Please pull forward."
- spicy-meal-poly--headset--snr5: false alarm on correct ticket (Said SPICY CHICKEN SANDWICH, rang SPICY CHICKEN SANDWICH MEAL; Said 1 Side Salad, not on ticket) — heard "Welcome to Chick-fil-A, what can I get started for you? Absolutely, would you like to try our seasonal Peach Milkshake today? | So that's a spicy chicken salad. | I'm going to try it. | Please pull forward."
- spicy-meal-poly--headset--snr5: missed planted count (Mismatch) — heard "Welcome to Chick-fil-A, what can I get started for you? Absolutely, would you like to try our seasonal Peach Milkshake today? | So that's a spicy chicken salad. | I'm going to try it. | Please pull forward."
- spicy-meal-poly--headset--snr5: missed planted size (Mismatch) — heard "Welcome to Chick-fil-A, what can I get started for you? Absolutely, would you like to try our seasonal Peach Milkshake today? | So that's a spicy chicken salad. | I'm going to try it. | Please pull forward."
- spicy-meal-poly--headset--snr5: missed planted extra (Mismatch) — heard "Welcome to Chick-fil-A, what can I get started for you? Absolutely, would you like to try our seasonal Peach Milkshake today? | So that's a spicy chicken salad. | I'm going to try it. | Please pull forward."
- spicy-nuggets-mix--face_to_face--snr5: extraction differs from truth (Said 1 Garden Herb Ranch, rang 3) — heard "Welcome, I'll get a Spicy Chicken Sandwich and an 8 ct Nuggets, and a large Waffle Fries, Spicy Chicken Sandwich, 8 ct Nuggets, Large Waffle Fries, | Garden Herb Ranch,"
- spicy-nuggets-mix--face_to_face--snr5: false alarm on correct ticket (Said 1 Garden Herb Ranch, rang 3) — heard "Welcome, I'll get a Spicy Chicken Sandwich and an 8 ct Nuggets, and a large Waffle Fries, Spicy Chicken Sandwich, 8 ct Nuggets, Large Waffle Fries, | Garden Herb Ranch,"
- spicy-nuggets-mix--headset--snr5: extraction differs from truth (Said 1 Garden Herb Ranch, rang 3) — heard "Welcome, Spicy Chicken Sandwich, 8 ct Nuggets, Large Fries, Brew Garden Herb Ranch,"
- spicy-nuggets-mix--headset--snr5: false alarm on correct ticket (Said 1 Garden Herb Ranch, rang 3) — heard "Welcome, Spicy Chicken Sandwich, 8 ct Nuggets, Large Fries, Brew Garden Herb Ranch,"
- strips-4-sriracha--face_to_face--clean: missed planted extra (Unconfirmed) — heard "Hi there! | Hi, a 4 count Chicken Strips and 2 Sweet & Spicy Sriracha Sauces | 4 counts Trits & 2 Sriracha | and a small Fruit Cup"
- strips-4-sriracha--face_to_face--snr15: missed planted extra (Unconfirmed) — heard "Hi there! | Hi, a 4 count Chicken Strips and 2 Sweet & Spicy Sriracha Sauces | 4 count Strips and 2 Sriracha | and a small Fruit Cup"
- strips-4-sriracha--face_to_face--snr5: extraction differs from truth (Rang 1 Small Fruit Cup, guest didn't ask) — heard "Harder, Huh, The 4-count Chicken Strips and 2-Sweet & Spicy Sriracha Sauces, 4-count Chicken Pizza, | and a small group"
- strips-4-sriracha--face_to_face--snr5: missed planted size (Unconfirmed) — heard "Harder, Huh, The 4-count Chicken Strips and 2-Sweet & Spicy Sriracha Sauces, 4-count Chicken Pizza, | and a small group"
- strips-4-sriracha--face_to_face--snr5: missed planted extra (Unconfirmed) — heard "Harder, Huh, The 4-count Chicken Strips and 2-Sweet & Spicy Sriracha Sauces, 4-count Chicken Pizza, | and a small group"
- strips-4-sriracha--headset--clean: extraction differs from truth (Rang 1 Small Fruit Cup, guest didn't ask) — heard "Hi there! | 4 ct Strips & 2 Sriracha"
- strips-4-sriracha--headset--clean: false alarm on correct ticket (Rang 1 Small Fruit Cup, guest didn't ask) — heard "Hi there! | 4 ct Strips & 2 Sriracha"
- strips-4-sriracha--headset--clean: missed planted size (Mismatch) — heard "Hi there! | 4 ct Strips & 2 Sriracha"
- strips-4-sriracha--headset--snr15: extraction differs from truth (Rang 1 4 ct Strips, guest didn't ask; Rang 2 Sweet & Spicy Sriracha, guest didn't ask; Rang 1 Small Fruit Cup, guest didn't ask) — heard "Hi there! | 4 ct Stripes & Sousa Radishes,"
- strips-4-sriracha--headset--snr15: missed planted variant (Unconfirmed) — heard "Hi there! | 4 ct Stripes & Sousa Radishes,"
- strips-4-sriracha--headset--snr15: missed planted count (Unconfirmed) — heard "Hi there! | 4 ct Stripes & Sousa Radishes,"
- strips-4-sriracha--headset--snr15: missed planted size (Unconfirmed) — heard "Hi there! | 4 ct Stripes & Sousa Radishes,"
- strips-4-sriracha--headset--snr15: missed planted missing (Unconfirmed) — heard "Hi there! | 4 ct Stripes & Sousa Radishes,"
- strips-4-sriracha--headset--snr15: missed planted extra (Unconfirmed) — heard "Hi there! | 4 ct Stripes & Sousa Radishes,"
- strips-4-sriracha--headset--snr5: extraction differs from truth (Rang 1 4 ct Strips, guest didn't ask; Rang 2 Sweet & Spicy Sriracha, guest didn't ask; Rang 1 Small Fruit Cup, guest didn't ask) — heard "Fire, 4/6 Ct Super Rachis,"
- strips-4-sriracha--headset--snr5: missed planted variant (Unconfirmed) — heard "Fire, 4/6 Ct Super Rachis,"
- strips-4-sriracha--headset--snr5: missed planted count (Unconfirmed) — heard "Fire, 4/6 Ct Super Rachis,"
- strips-4-sriracha--headset--snr5: missed planted size (Unconfirmed) — heard "Fire, 4/6 Ct Super Rachis,"
- strips-4-sriracha--headset--snr5: missed planted missing (Unconfirmed) — heard "Fire, 4/6 Ct Super Rachis,"
- strips-4-sriracha--headset--snr5: missed planted extra (Unconfirmed) — heard "Fire, 4/6 Ct Super Rachis,"
- strips-meal-ice--face_to_face--snr5: extraction differs from truth (Rang 1 4 ct Strips Meal, guest didn't ask; Rang 2 Zesty Buffalo, guest didn't ask) — heard "What do I get me? A 4 pound Spritz meal with a large Unsweet Tea, No Ice 4 pound Spritz meal"
- strips-meal-ice--face_to_face--snr5: missed planted count (Unconfirmed) — heard "What do I get me? A 4 pound Spritz meal with a large Unsweet Tea, No Ice 4 pound Spritz meal"
- strips-meal-ice--face_to_face--snr5: missed planted extra (Unconfirmed) — heard "What do I get me? A 4 pound Spritz meal with a large Unsweet Tea, No Ice 4 pound Spritz meal"
- strips-meal-ice--headset--clean: extraction differs from truth (Said 4 CT STRIPS, rang 4 CT STRIPS MEAL; Rang 2 Zesty Buffalo, guest didn't ask) — heard "What can I get you? | 4 ct Strip's Wheel, Large Uzzly Tea, No Ice"
- strips-meal-ice--headset--clean: missed planted variant (Unconfirmed) — heard "What can I get you? | 4 ct Strip's Wheel, Large Uzzly Tea, No Ice"
- strips-meal-ice--headset--clean: missed planted count (Unconfirmed) — heard "What can I get you? | 4 ct Strip's Wheel, Large Uzzly Tea, No Ice"
- strips-meal-ice--headset--clean: missed planted size (Unconfirmed) — heard "What can I get you? | 4 ct Strip's Wheel, Large Uzzly Tea, No Ice"
- strips-meal-ice--headset--clean: missed planted extra (Unconfirmed) — heard "What can I get you? | 4 ct Strip's Wheel, Large Uzzly Tea, No Ice"
- strips-meal-ice--headset--snr15: extraction differs from truth (Said 4 CT STRIPS, rang 4 CT STRIPS MEAL; Rang 2 Zesty Buffalo, guest didn't ask) — heard "What Like At You? | 4 Count Strip's Leel, Large Unsneaked Tea, No Ice"
- strips-meal-ice--headset--snr15: missed planted variant (Unconfirmed) — heard "What Like At You? | 4 Count Strip's Leel, Large Unsneaked Tea, No Ice"
- strips-meal-ice--headset--snr15: missed planted count (Unconfirmed) — heard "What Like At You? | 4 Count Strip's Leel, Large Unsneaked Tea, No Ice"
- strips-meal-ice--headset--snr15: missed planted size (Unconfirmed) — heard "What Like At You? | 4 Count Strip's Leel, Large Unsneaked Tea, No Ice"
- strips-meal-ice--headset--snr15: missed planted extra (Unconfirmed) — heard "What Like At You? | 4 Count Strip's Leel, Large Unsneaked Tea, No Ice"
- strips-meal-ice--headset--snr5: extraction differs from truth (Said 4 CT STRIPS, rang 4 CT STRIPS MEAL; Rang 2 Zesty Buffalo, guest didn't ask) — heard "What like that ewe, 4 ct Strip's Ewe, Mar a dozen tea, No Ice"
- strips-meal-ice--headset--snr5: missed planted variant (Unconfirmed) — heard "What like that ewe, 4 ct Strip's Ewe, Mar a dozen tea, No Ice"
- strips-meal-ice--headset--snr5: missed planted count (Unconfirmed) — heard "What like that ewe, 4 ct Strip's Ewe, Mar a dozen tea, No Ice"
- strips-meal-ice--headset--snr5: missed planted size (Unconfirmed) — heard "What like that ewe, 4 ct Strip's Ewe, Mar a dozen tea, No Ice"
- strips-meal-ice--headset--snr5: missed planted extra (Unconfirmed) — heard "What like that ewe, 4 ct Strip's Ewe, Mar a dozen tea, No Ice"
- twelve-grilled--face_to_face--snr5: extraction differs from truth (Rang 1 Large Diet Cola, guest didn't ask) — heard "No Tumper Soup, No Race, Can I have a 12-pound grilled nugget and a large guard-shell? 12-pound grilled nugget and a large-sized bird-shell? | I would like to fly with it, no"
- twelve-grilled--face_to_face--snr5: missed planted size (Unconfirmed) — heard "No Tumper Soup, No Race, Can I have a 12-pound grilled nugget and a large guard-shell? 12-pound grilled nugget and a large-sized bird-shell? | I would like to fly with it, no"
- twelve-grilled--face_to_face--snr5: missed planted modifier (Unconfirmed) — heard "No Tumper Soup, No Race, Can I have a 12-pound grilled nugget and a large guard-shell? 12-pound grilled nugget and a large-sized bird-shell? | I would like to fly with it, no"
- twelve-grilled--face_to_face--snr5: missed planted missing (Match) — heard "No Tumper Soup, No Race, Can I have a 12-pound grilled nugget and a large guard-shell? 12-pound grilled nugget and a large-sized bird-shell? | I would like to fly with it, no"
- twelve-grilled--face_to_face--snr5: missed planted extra (Unconfirmed) — heard "No Tumper Soup, No Race, Can I have a 12-pound grilled nugget and a large guard-shell? 12-pound grilled nugget and a large-sized bird-shell? | I would like to fly with it, no"
- twelve-grilled--headset--snr15: extraction differs from truth (Said Shake, not on ticket; Said COLA, rang DIET COLA; Said 1 Side Salad, not on ticket) — heard "Milkshake, Chick-fil-A | Quail-Towel Grilled Nuggets with a Large Salad Coke, Would You Like To Fly with Each Milkshake?"
- twelve-grilled--headset--snr15: false alarm on correct ticket (Said Shake, not on ticket) — heard "Milkshake, Chick-fil-A | Quail-Towel Grilled Nuggets with a Large Salad Coke, Would You Like To Fly with Each Milkshake?"
- twelve-grilled--headset--snr15: missed planted variant (Mismatch) — heard "Milkshake, Chick-fil-A | Quail-Towel Grilled Nuggets with a Large Salad Coke, Would You Like To Fly with Each Milkshake?"
- twelve-grilled--headset--snr15: missed planted count (Mismatch) — heard "Milkshake, Chick-fil-A | Quail-Towel Grilled Nuggets with a Large Salad Coke, Would You Like To Fly with Each Milkshake?"
- twelve-grilled--headset--snr15: missed planted size (Mismatch) — heard "Milkshake, Chick-fil-A | Quail-Towel Grilled Nuggets with a Large Salad Coke, Would You Like To Fly with Each Milkshake?"
- twelve-grilled--headset--snr15: missed planted modifier (Mismatch) — heard "Milkshake, Chick-fil-A | Quail-Towel Grilled Nuggets with a Large Salad Coke, Would You Like To Fly with Each Milkshake?"
- twelve-grilled--headset--snr15: missed planted extra (Mismatch) — heard "Milkshake, Chick-fil-A | Quail-Towel Grilled Nuggets with a Large Salad Coke, Would You Like To Fly with Each Milkshake?"
- twelve-grilled--headset--snr5: extraction differs from truth (Rang 1 12 ct Grilled Nuggets, guest didn't ask; Rang 1 Large Diet Cola, guest didn't ask) — heard "No Tender Keeps to Here, Those Are Good Muggies & The Muggs Try & Slow, Will Be Like The Try & The Keeps Muggies"
- twelve-grilled--headset--snr5: missed planted variant (Unconfirmed) — heard "No Tender Keeps to Here, Those Are Good Muggies & The Muggs Try & Slow, Will Be Like The Try & The Keeps Muggies"
- twelve-grilled--headset--snr5: missed planted count (Unconfirmed) — heard "No Tender Keeps to Here, Those Are Good Muggies & The Muggs Try & Slow, Will Be Like The Try & The Keeps Muggies"
- twelve-grilled--headset--snr5: missed planted size (Unconfirmed) — heard "No Tender Keeps to Here, Those Are Good Muggies & The Muggs Try & Slow, Will Be Like The Try & The Keeps Muggies"
- twelve-grilled--headset--snr5: missed planted modifier (Unconfirmed) — heard "No Tender Keeps to Here, Those Are Good Muggies & The Muggs Try & Slow, Will Be Like The Try & The Keeps Muggies"
- twelve-grilled--headset--snr5: missed planted missing (Unconfirmed) — heard "No Tender Keeps to Here, Those Are Good Muggies & The Muggs Try & Slow, Will Be Like The Try & The Keeps Muggies"
- twelve-grilled--headset--snr5: missed planted extra (Unconfirmed) — heard "No Tender Keeps to Here, Those Are Good Muggies & The Muggs Try & Slow, Will Be Like The Try & The Keeps Muggies"
- two-cookies-shake--face_to_face--snr5: extraction differs from truth (Rang No Whipped Cream on Chocolate Milkshake, guest didn't ask) — heard "Welcome, What Can I Get For You? Can I Get 2 Cookies & A Chocolate Milkshave? 2 Cookies & A Chocolate Milkshave, Would You Like Whippeam On The Shave? | No Whippeam"
- two-cookies-shake--face_to_face--snr5: false alarm on correct ticket (Rang No Whipped Cream on Chocolate Milkshake, guest didn't ask) — heard "Welcome, What Can I Get For You? Can I Get 2 Cookies & A Chocolate Milkshave? 2 Cookies & A Chocolate Milkshave, Would You Like Whippeam On The Shave? | No Whippeam"
- two-cookies-shake--face_to_face--snr5: missed planted modifier (Match) — heard "Welcome, What Can I Get For You? Can I Get 2 Cookies & A Chocolate Milkshave? 2 Cookies & A Chocolate Milkshave, Would You Like Whippeam On The Shave? | No Whippeam"
- two-cookies-shake--headset--snr5: extraction differs from truth (Rang 2 Cookie, guest didn't ask) — heard "Welcome, What Can I Get For You? 2 Pickles & A Chocolate Milkshake, Would You Like Whipped Cream On The Shake? | Got It? 2 Pickles & A Chocolate Shake, No Whipped Cream"
- two-cookies-shake--headset--snr5: false alarm on correct ticket (Rang 2 Cookie, guest didn't ask) — heard "Welcome, What Can I Get For You? 2 Pickles & A Chocolate Milkshake, Would You Like Whipped Cream On The Shake? | Got It? 2 Pickles & A Chocolate Shake, No Whipped Cream"
- two-cookies-shake--headset--snr5: missed planted extra (Unconfirmed) — heard "Welcome, What Can I Get For You? 2 Pickles & A Chocolate Milkshake, Would You Like Whipped Cream On The Shake? | Got It? 2 Pickles & A Chocolate Shake, No Whipped Cream"
- two-meals-different--headset--clean: missed planted size (Match) — heard "What can I get for you? A Chicken Sandwich Meal is Leaky & A Spicy Chicken Sandwich Meal is Lemonade, both medium"
- two-meals-different--headset--snr15: extraction differs from truth (Said CHICKEN SANDWICH, rang CHICKEN SANDWICH MEAL; Said SPICY CHICKEN SANDWICH, rang SPICY CHICKEN SANDWICH MEAL) — heard "What can I get for you? A Chicken Sandwich Lee will eat tea and a Spicy Chicken Sandwich Lee will lemonade, both lead you."
- two-meals-different--headset--snr15: missed planted size (Unconfirmed) — heard "What can I get for you? A Chicken Sandwich Lee will eat tea and a Spicy Chicken Sandwich Lee will lemonade, both lead you."
- two-meals-different--headset--snr15: missed planted modifier (Unconfirmed) — heard "What can I get for you? A Chicken Sandwich Lee will eat tea and a Spicy Chicken Sandwich Lee will lemonade, both lead you."
- two-meals-different--headset--snr15: missed planted extra (Unconfirmed) — heard "What can I get for you? A Chicken Sandwich Lee will eat tea and a Spicy Chicken Sandwich Lee will lemonade, both lead you."
- two-meals-different--headset--snr5: extraction differs from truth (Said SPICY CHICKEN SANDWICH, rang SPICY CHICKEN SANDWICH MEAL; Rang 1 Chicken Sandwich Meal, guest didn't ask; Rang 1 Medium Sweet Tea, guest didn't ask; Rang 1 Medium Lemonade, guest didn't ask) — heard "What do I get for you? A thickly sandwich really lives neatly with a spicy thickly sandwich really well made, hopefully dear."
- two-meals-different--headset--snr5: missed planted variant (Unconfirmed) — heard "What do I get for you? A thickly sandwich really lives neatly with a spicy thickly sandwich really well made, hopefully dear."
- two-meals-different--headset--snr5: missed planted count (Unconfirmed) — heard "What do I get for you? A thickly sandwich really lives neatly with a spicy thickly sandwich really well made, hopefully dear."
- two-meals-different--headset--snr5: missed planted size (Unconfirmed) — heard "What do I get for you? A thickly sandwich really lives neatly with a spicy thickly sandwich really well made, hopefully dear."
- two-meals-different--headset--snr5: missed planted modifier (Unconfirmed) — heard "What do I get for you? A thickly sandwich really lives neatly with a spicy thickly sandwich really well made, hopefully dear."
- two-meals-different--headset--snr5: missed planted missing (Mismatch) — heard "What do I get for you? A thickly sandwich really lives neatly with a spicy thickly sandwich really well made, hopefully dear."
- two-meals-different--headset--snr5: missed planted extra (Unconfirmed) — heard "What do I get for you? A thickly sandwich really lives neatly with a spicy thickly sandwich really well made, hopefully dear."
- unspecified-nuggets--face_to_face--snr15: missed planted count (Unconfirmed) — heard "What's been like that for you? | Some Nuggets and Lemonade please. | My Flagger."
- unspecified-nuggets--face_to_face--snr15: missed planted modifier (Unconfirmed) — heard "What's been like that for you? | Some Nuggets and Lemonade please. | My Flagger."
- unspecified-nuggets--face_to_face--snr15: missed planted extra (Unconfirmed) — heard "What's been like that for you? | Some Nuggets and Lemonade please. | My Flagger."
- unspecified-nuggets--face_to_face--snr5: extraction differs from truth (Rang 1 8 ct Nuggets, guest didn't ask) — heard "What do I get for you? Some Novets and Lemonade please My pleasure"
- unspecified-nuggets--face_to_face--snr5: missed planted count (Unconfirmed) — heard "What do I get for you? Some Novets and Lemonade please My pleasure"
- unspecified-nuggets--face_to_face--snr5: missed planted modifier (Unconfirmed) — heard "What do I get for you? Some Novets and Lemonade please My pleasure"
- unspecified-nuggets--face_to_face--snr5: missed planted extra (Unconfirmed) — heard "What do I get for you? Some Novets and Lemonade please My pleasure"
- water-and-cookie--face_to_face--snr5: missed planted count (Unconfirmed) — heard "What do my cats argue for you? Just to kindly turn cookie into fetch of water Like weather"
- water-and-cookie--face_to_face--snr5: missed planted modifier (Unconfirmed) — heard "What do my cats argue for you? Just to kindly turn cookie into fetch of water Like weather"
- water-and-cookie--face_to_face--snr5: missed planted extra (Unconfirmed) — heard "What do my cats argue for you? Just to kindly turn cookie into fetch of water Like weather"


## Full report (default model, held-out set)


69 orders, 405 ticket checks. The check caught **70.2%** of planted mistakes, raised a false alarm on **30.4%** of correct tickets, and returned a result in **389 ms** (median; p95 617 ms) after Finalize.

| Metric | Value | Target |
|---|---|---|
| Planted mistakes caught | 70.2% | ≥ 90% |
| False alarms on correct tickets | 30.4% | ≤ 5% |
| Finalize → result, p95 | 617 ms | ≤ 1000 ms |
| Spoken order read exactly right | 49.3% | — |
| Checks returned "couldn't confirm" | 18.3% | — |

## By mistake type

| Type | Caught | Planted | Rate |
|---|---|---|---|
| count | 50 | 69 | 72.5% |
| extra | 37 | 69 | 53.6% |
| missing | 28 | 45 | 62.2% |
| modifier | 33 | 45 | 73.3% |
| size | 41 | 51 | 80.4% |
| variant | 47 | 57 | 82.5% |

## By audio situation

| | Caught | False alarms |
|---|---|---|
| face_to_face | 78.0% | 27.8% |
| headset | 61.6% | 33.3% |

## By background noise

| | Caught | False alarms |
|---|---|---|
| clean | 81.2% | 34.8% |
| snr15 | 75.9% | 43.5% |
| snr5 | 53.6% | 13.0% |

## Misses and false alarms (156)

- ho-actually-also--face_to_face--snr5: extraction differs from truth (Said CHICKEN SANDWICH, rang CHICKEN SANDWICH MEAL) — heard "I will look like that for you A Chicken Sandwich near the large Dr Pepper Oh actually, can I also get a cookie?"
- ho-actually-also--face_to_face--snr5: missed planted modifier (Unconfirmed) — heard "I will look like that for you A Chicken Sandwich near the large Dr Pepper Oh actually, can I also get a cookie?"
- ho-actually-also--face_to_face--snr5: missed planted extra (Unconfirmed) — heard "I will look like that for you A Chicken Sandwich near the large Dr Pepper Oh actually, can I also get a cookie?"
- ho-actually-also--headset--clean: extraction differs from truth (Said CHICKEN SANDWICH, rang CHICKEN SANDWICH MEAL) — heard "Hi there, what can I get for you? | Chicken Sandwich, Grill with a large Dr Pepper and a cookie."
- ho-actually-also--headset--clean: false alarm on correct ticket (Said CHICKEN SANDWICH, rang CHICKEN SANDWICH MEAL) — heard "Hi there, what can I get for you? | Chicken Sandwich, Grill with a large Dr Pepper and a cookie."
- ho-actually-also--headset--snr15: extraction differs from truth (Said CHICKEN SANDWICH, rang CHICKEN SANDWICH MEAL) — heard "Hi there, what can I get for you? | Chicken Sandwich, with a large Dr Pepper and a cookie."
- ho-actually-also--headset--snr15: false alarm on correct ticket (Said CHICKEN SANDWICH, rang CHICKEN SANDWICH MEAL) — heard "Hi there, what can I get for you? | Chicken Sandwich, with a large Dr Pepper and a cookie."
- ho-actually-also--headset--snr5: extraction differs from truth (Said CHICKEN SANDWICH, rang CHICKEN SANDWICH MEAL) — heard "Hi there, what can I get for you? Chicken Sandwich with a large Dr Pepper and a cookie"
- ho-actually-also--headset--snr5: missed planted modifier (Unconfirmed) — heard "Hi there, what can I get for you? Chicken Sandwich with a large Dr Pepper and a cookie"
- ho-actually-also--headset--snr5: missed planted extra (Unconfirmed) — heard "Hi there, what can I get for you? Chicken Sandwich with a large Dr Pepper and a cookie"
- ho-correction-count--face_to_face--snr15: missed planted extra (Unconfirmed) — heard "Any Sources For You? | 2 Polynesian, Actually Make The 3 | Polynesian Sources"
- ho-correction-count--face_to_face--snr5: missed planted extra (Unconfirmed) — heard "Any sources for him? 2 Polynesian, Actually Night Defray, 3 Polynesian sources"
- ho-correction-count--headset--snr15: missed planted extra (Unconfirmed) — heard "Any Sources For You? | Free Polynesian Sources"
- ho-correction-count--headset--snr5: extraction differs from truth (Rang 3 Polynesian, guest didn't ask) — heard "Any Sources For You, You Have An Unions Sources"
- ho-correction-count--headset--snr5: missed planted count (Unconfirmed) — heard "Any Sources For You, You Have An Unions Sources"
- ho-correction-count--headset--snr5: missed planted extra (Unconfirmed) — heard "Any Sources For You, You Have An Unions Sources"
- ho-declined-size--face_to_face--clean: extraction differs from truth (Rang 1 3 ct Strips, guest didn't ask) — heard "What can I get started for you? | Can I have a medium sweet tea and a 3 ct Strip? | Do you want to make that tea large for just a little more? | Nah, medium's fine. | Medium's sweet tea and a 3 ct Strip."
- ho-declined-size--face_to_face--clean: false alarm on correct ticket (Rang 1 3 ct Strips, guest didn't ask) — heard "What can I get started for you? | Can I have a medium sweet tea and a 3 ct Strip? | Do you want to make that tea large for just a little more? | Nah, medium's fine. | Medium's sweet tea and a 3 ct Strip."
- ho-declined-size--face_to_face--clean: missed planted count (Mismatch) — heard "What can I get started for you? | Can I have a medium sweet tea and a 3 ct Strip? | Do you want to make that tea large for just a little more? | Nah, medium's fine. | Medium's sweet tea and a 3 ct Strip."
- ho-declined-size--face_to_face--clean: missed planted missing (Match) — heard "What can I get started for you? | Can I have a medium sweet tea and a 3 ct Strip? | Do you want to make that tea large for just a little more? | Nah, medium's fine. | Medium's sweet tea and a 3 ct Strip."
- ho-declined-size--face_to_face--clean: missed planted extra (Unconfirmed) — heard "What can I get started for you? | Can I have a medium sweet tea and a 3 ct Strip? | Do you want to make that tea large for just a little more? | Nah, medium's fine. | Medium's sweet tea and a 3 ct Strip."
- ho-declined-size--face_to_face--snr15: extraction differs from truth (Rang 1 3 ct Strips, guest didn't ask) — heard "What can I get started for you? | Can I have a medium sweet tea and a 3 ct Spritz? | Do you want to make that tea large for just a little more? | Nah, mediums fun | Medium sweet tea and a 3 ct Strip"
- ho-declined-size--face_to_face--snr15: false alarm on correct ticket (Rang 1 3 ct Strips, guest didn't ask) — heard "What can I get started for you? | Can I have a medium sweet tea and a 3 ct Spritz? | Do you want to make that tea large for just a little more? | Nah, mediums fun | Medium sweet tea and a 3 ct Strip"
- ho-declined-size--face_to_face--snr15: missed planted count (Mismatch) — heard "What can I get started for you? | Can I have a medium sweet tea and a 3 ct Spritz? | Do you want to make that tea large for just a little more? | Nah, mediums fun | Medium sweet tea and a 3 ct Strip"
- ho-declined-size--face_to_face--snr15: missed planted missing (Match) — heard "What can I get started for you? | Can I have a medium sweet tea and a 3 ct Spritz? | Do you want to make that tea large for just a little more? | Nah, mediums fun | Medium sweet tea and a 3 ct Strip"
- ho-declined-size--face_to_face--snr15: missed planted extra (Unconfirmed) — heard "What can I get started for you? | Can I have a medium sweet tea and a 3 ct Spritz? | Do you want to make that tea large for just a little more? | Nah, mediums fun | Medium sweet tea and a 3 ct Strip"
- ho-declined-size--face_to_face--snr5: extraction differs from truth (Rang 1 3 ct Strips, guest didn't ask) — heard "What can I get started with? Can I have a medium Sweet Tea in the 3-pound space? Do you want to make that feel art for just a little more? | Medium Sweet Tea in the 3-pound space?"
- ho-declined-size--face_to_face--snr5: missed planted count (Unconfirmed) — heard "What can I get started with? Can I have a medium Sweet Tea in the 3-pound space? Do you want to make that feel art for just a little more? | Medium Sweet Tea in the 3-pound space?"
- ho-declined-size--face_to_face--snr5: missed planted modifier (Unconfirmed) — heard "What can I get started with? Can I have a medium Sweet Tea in the 3-pound space? Do you want to make that feel art for just a little more? | Medium Sweet Tea in the 3-pound space?"
- ho-declined-size--face_to_face--snr5: missed planted missing (Match) — heard "What can I get started with? Can I have a medium Sweet Tea in the 3-pound space? Do you want to make that feel art for just a little more? | Medium Sweet Tea in the 3-pound space?"
- ho-declined-size--face_to_face--snr5: missed planted extra (Unconfirmed) — heard "What can I get started with? Can I have a medium Sweet Tea in the 3-pound space? Do you want to make that feel art for just a little more? | Medium Sweet Tea in the 3-pound space?"
- ho-declined-size--headset--clean: extraction differs from truth (Rang 1 3 ct Strips, guest didn't ask) — heard "What can I get started for you? | Do you want to make that tea large for just a little more? | Medium-Sweet Tea of the 3-Counce Trips"
- ho-declined-size--headset--clean: false alarm on correct ticket (Rang 1 3 ct Strips, guest didn't ask) — heard "What can I get started for you? | Do you want to make that tea large for just a little more? | Medium-Sweet Tea of the 3-Counce Trips"
- ho-declined-size--headset--clean: missed planted count (Mismatch) — heard "What can I get started for you? | Do you want to make that tea large for just a little more? | Medium-Sweet Tea of the 3-Counce Trips"
- ho-declined-size--headset--clean: missed planted missing (Match) — heard "What can I get started for you? | Do you want to make that tea large for just a little more? | Medium-Sweet Tea of the 3-Counce Trips"
- ho-declined-size--headset--clean: missed planted extra (Unconfirmed) — heard "What can I get started for you? | Do you want to make that tea large for just a little more? | Medium-Sweet Tea of the 3-Counce Trips"
- ho-declined-size--headset--snr15: extraction differs from truth (Rang 1 3 ct Strips, guest didn't ask) — heard "What can I get started for you? | Do you want to make that tea large for just a little more? | Medium Sweet Tea of the 3-ounce Strip"
- ho-declined-size--headset--snr15: false alarm on correct ticket (Rang 1 3 ct Strips, guest didn't ask) — heard "What can I get started for you? | Do you want to make that tea large for just a little more? | Medium Sweet Tea of the 3-ounce Strip"
- ho-declined-size--headset--snr15: missed planted count (Mismatch) — heard "What can I get started for you? | Do you want to make that tea large for just a little more? | Medium Sweet Tea of the 3-ounce Strip"
- ho-declined-size--headset--snr15: missed planted missing (Match) — heard "What can I get started for you? | Do you want to make that tea large for just a little more? | Medium Sweet Tea of the 3-ounce Strip"
- ho-declined-size--headset--snr15: missed planted extra (Unconfirmed) — heard "What can I get started for you? | Do you want to make that tea large for just a little more? | Medium Sweet Tea of the 3-ounce Strip"
- ho-declined-size--headset--snr5: extraction differs from truth (Rang 1 3 ct Strips, guest didn't ask) — heard "What's the most exciting thing? Do you want to make a tea and water for just a little more? Me and Sweet Tea are the three pound chips."
- ho-declined-size--headset--snr5: missed planted count (Unconfirmed) — heard "What's the most exciting thing? Do you want to make a tea and water for just a little more? Me and Sweet Tea are the three pound chips."
- ho-declined-size--headset--snr5: missed planted size (Unconfirmed) — heard "What's the most exciting thing? Do you want to make a tea and water for just a little more? Me and Sweet Tea are the three pound chips."
- ho-declined-size--headset--snr5: missed planted modifier (Unconfirmed) — heard "What's the most exciting thing? Do you want to make a tea and water for just a little more? Me and Sweet Tea are the three pound chips."
- ho-declined-size--headset--snr5: missed planted missing (Match) — heard "What's the most exciting thing? Do you want to make a tea and water for just a little more? Me and Sweet Tea are the three pound chips."
- ho-declined-size--headset--snr5: missed planted extra (Unconfirmed) — heard "What's the most exciting thing? Do you want to make a tea and water for just a little more? Me and Sweet Tea are the three pound chips."
- ho-deluxe-meal-sub--face_to_face--snr15: extraction differs from truth (Said 2 Fruit Cup, rang 1) — heard "What can I get for you? | A deluxe chicken sandwich meal with no pickles, sub a fruit cup for the fries, and a medium sunjoy. | What's meal? No pickles, fruit cup, medium sunjoy."
- ho-deluxe-meal-sub--face_to_face--snr15: false alarm on correct ticket (Said 2 Fruit Cup, rang 1) — heard "What can I get for you? | A deluxe chicken sandwich meal with no pickles, sub a fruit cup for the fries, and a medium sunjoy. | What's meal? No pickles, fruit cup, medium sunjoy."
- ho-deluxe-meal-sub--face_to_face--snr5: extraction differs from truth (Said DELUXE CHICKEN SANDWICH, rang DELUXE CHICKEN SANDWICH MEAL; Rang 1 Medium Fruit Cup, guest didn't ask) — heard "Add some of the Cucumber, A Deluxe Chicken Sandwich near with No Pickles, Sothe Food Cup for the Fries, and a medium Sunjoy | So what's new for the people?"
- ho-deluxe-meal-sub--face_to_face--snr5: missed planted extra (Unconfirmed) — heard "Add some of the Cucumber, A Deluxe Chicken Sandwich near with No Pickles, Sothe Food Cup for the Fries, and a medium Sunjoy | So what's new for the people?"
- ho-deluxe-meal-sub--headset--snr15: extraction differs from truth (Rang 1 Deluxe Chicken Sandwich Meal, guest didn't ask; Rang 1 Medium Fruit Cup, guest didn't ask) — heard "What am I get for you? | Watch me or slow pickles, food pups, medium sun joy."
- ho-deluxe-meal-sub--headset--snr15: missed planted variant (Unconfirmed) — heard "What am I get for you? | Watch me or slow pickles, food pups, medium sun joy."
- ho-deluxe-meal-sub--headset--snr15: missed planted count (Unconfirmed) — heard "What am I get for you? | Watch me or slow pickles, food pups, medium sun joy."
- ho-deluxe-meal-sub--headset--snr15: missed planted modifier (Unconfirmed) — heard "What am I get for you? | Watch me or slow pickles, food pups, medium sun joy."
- ho-deluxe-meal-sub--headset--snr15: missed planted extra (Unconfirmed) — heard "What am I get for you? | Watch me or slow pickles, food pups, medium sun joy."
- ho-deluxe-meal-sub--headset--snr5: extraction differs from truth (Rang 1 Deluxe Chicken Sandwich Meal, guest didn't ask; Rang 1 Medium Fruit Cup, guest didn't ask; Rang 1 Medium Sunjoy, guest didn't ask) — heard "No Tomato, No Moon, No Me, No Pepper, No Salt, Me & Summer"
- ho-deluxe-meal-sub--headset--snr5: missed planted variant (Unconfirmed) — heard "No Tomato, No Moon, No Me, No Pepper, No Salt, Me & Summer"
- ho-deluxe-meal-sub--headset--snr5: missed planted count (Unconfirmed) — heard "No Tomato, No Moon, No Me, No Pepper, No Salt, Me & Summer"
- ho-deluxe-meal-sub--headset--snr5: missed planted size (Unconfirmed) — heard "No Tomato, No Moon, No Me, No Pepper, No Salt, Me & Summer"
- ho-deluxe-meal-sub--headset--snr5: missed planted modifier (Unconfirmed) — heard "No Tomato, No Moon, No Me, No Pepper, No Salt, Me & Summer"
- ho-deluxe-meal-sub--headset--snr5: missed planted extra (Unconfirmed) — heard "No Tomato, No Moon, No Me, No Pepper, No Salt, Me & Summer"
- ho-grilled-meal-swap--face_to_face--snr15: extraction differs from truth (Said 1 12 ct Nuggets, not on ticket) — heard "Hi, what can I get for you? | Could I get a 12-count grilled nub at me with a large unsweet tea, and 3 Honey Mustard? | 12-count grilled nugget meal, large unsweet tea, 3 Honey Mustard,"
- ho-grilled-meal-swap--face_to_face--snr15: false alarm on correct ticket (Said 1 12 ct Nuggets, not on ticket) — heard "Hi, what can I get for you? | Could I get a 12-count grilled nub at me with a large unsweet tea, and 3 Honey Mustard? | 12-count grilled nugget meal, large unsweet tea, 3 Honey Mustard,"
- ho-grilled-meal-swap--face_to_face--snr5: extraction differs from truth (Said 12 CT GRILLED NUGGETS, rang 12 CT GRILLED NUGGETS MEAL) — heard "Huh, What can I get for you? Could I get a 12-pound Grilled Nugget Me with a large unsweet tea, and 3 Honey Mustard | 12-pound Grilled Nugget Me with a large unsweet tea,"
- ho-grilled-meal-swap--face_to_face--snr5: missed planted extra (Unconfirmed) — heard "Huh, What can I get for you? Could I get a 12-pound Grilled Nugget Me with a large unsweet tea, and 3 Honey Mustard | 12-pound Grilled Nugget Me with a large unsweet tea,"
- ho-grilled-meal-swap--headset--snr5: extraction differs from truth (Said SWEET TEA, rang UNSWEET TEA) — heard "Hi, What can I get for you? 12 ct Grilled Nugget Meal, Large & Sweet Tea, 3 Honey Mustard"
- ho-grilled-meal-swap--headset--snr5: false alarm on correct ticket (Said SWEET TEA, rang UNSWEET TEA) — heard "Hi, What can I get for you? 12 ct Grilled Nugget Meal, Large & Sweet Tea, 3 Honey Mustard"
- ho-grilled-meal-swap--headset--snr5: missed planted variant (Match) — heard "Hi, What can I get for you? 12 ct Grilled Nugget Meal, Large & Sweet Tea, 3 Honey Mustard"
- ho-grilled-meal-swap--headset--snr5: missed planted extra (Mismatch) — heard "Hi, What can I get for you? 12 ct Grilled Nugget Meal, Large & Sweet Tea, 3 Honey Mustard"
- ho-never-mind-swap--face_to_face--snr15: missed planted extra (Unconfirmed) — heard "What like that for you? | Doesn't look good | Never mind, just a medium-rat and cheese | One medium-rat and cheese"
- ho-never-mind-swap--face_to_face--snr5: missed planted count (Unconfirmed) — heard "What do I get for you? A deep cup, never long, just a mini mac and cheese | One review"
- ho-never-mind-swap--face_to_face--snr5: missed planted size (Match) — heard "What do I get for you? A deep cup, never long, just a mini mac and cheese | One review"
- ho-never-mind-swap--face_to_face--snr5: missed planted extra (Unconfirmed) — heard "What do I get for you? A deep cup, never long, just a mini mac and cheese | One review"
- ho-never-mind-swap--headset--snr5: extraction differs from truth (Rang 1 Medium Mac & Cheese, guest didn't ask) — heard "What like that for you? Why leave you mad for thieves?"
- ho-never-mind-swap--headset--snr5: missed planted count (Unconfirmed) — heard "What like that for you? Why leave you mad for thieves?"
- ho-never-mind-swap--headset--snr5: missed planted size (Unconfirmed) — heard "What like that for you? Why leave you mad for thieves?"
- ho-never-mind-swap--headset--snr5: missed planted extra (Unconfirmed) — heard "What like that for you? Why leave you mad for thieves?"
- ho-shake-mods--face_to_face--clean: extraction differs from truth (Said 2 Chocolate Milkshake, rang 1; Rang 1 Chocolate Milkshake, guest didn't ask) — heard "What can I get started for you? | 2 Chocolate Milkshakes, 1 with No Whip cream | 2 Chocolate Shakes"
- ho-shake-mods--face_to_face--clean: false alarm on correct ticket (Said 2 Chocolate Milkshake, rang 1; Rang 1 Chocolate Milkshake, guest didn't ask) — heard "What can I get started for you? | 2 Chocolate Milkshakes, 1 with No Whip cream | 2 Chocolate Shakes"
- ho-shake-mods--face_to_face--clean: missed planted missing (Mismatch) — heard "What can I get started for you? | 2 Chocolate Milkshakes, 1 with No Whip cream | 2 Chocolate Shakes"
- ho-shake-mods--face_to_face--clean: missed planted extra (Mismatch) — heard "What can I get started for you? | 2 Chocolate Milkshakes, 1 with No Whip cream | 2 Chocolate Shakes"
- ho-shake-mods--face_to_face--snr15: extraction differs from truth (Said 2 Chocolate Milkshake, rang 1; Rang 1 Chocolate Milkshake, guest didn't ask) — heard "What time I get started for you? | 2 Chocolate Milkshakes, 1 with No Whip Cream, | 2 Chocolate Cakes,"
- ho-shake-mods--face_to_face--snr15: false alarm on correct ticket (Said 2 Chocolate Milkshake, rang 1; Rang 1 Chocolate Milkshake, guest didn't ask) — heard "What time I get started for you? | 2 Chocolate Milkshakes, 1 with No Whip Cream, | 2 Chocolate Cakes,"
- ho-shake-mods--face_to_face--snr15: missed planted missing (Mismatch) — heard "What time I get started for you? | 2 Chocolate Milkshakes, 1 with No Whip Cream, | 2 Chocolate Cakes,"
- ho-shake-mods--face_to_face--snr15: missed planted extra (Mismatch) — heard "What time I get started for you? | 2 Chocolate Milkshakes, 1 with No Whip Cream, | 2 Chocolate Cakes,"
- ho-shake-mods--face_to_face--snr5: extraction differs from truth (Said 2 Chocolate Milkshake, rang 1; Rang 1 Chocolate Milkshake, guest didn't ask) — heard "What's in my get-go-to-do list? 2 Chocolate Milkshakes, 1 with no whipped cream, 2 Chocolate Cakes,"
- ho-shake-mods--face_to_face--snr5: false alarm on correct ticket (Said 2 Chocolate Milkshake, rang 1) — heard "What's in my get-go-to-do list? 2 Chocolate Milkshakes, 1 with no whipped cream, 2 Chocolate Cakes,"
- ho-shake-mods--face_to_face--snr5: missed planted missing (Mismatch) — heard "What's in my get-go-to-do list? 2 Chocolate Milkshakes, 1 with no whipped cream, 2 Chocolate Cakes,"
- ho-shake-mods--face_to_face--snr5: missed planted extra (Mismatch) — heard "What's in my get-go-to-do list? 2 Chocolate Milkshakes, 1 with no whipped cream, 2 Chocolate Cakes,"
- ho-shake-mods--headset--clean: extraction differs from truth (Said 2 Chocolate Milkshake, rang 1; Rang 1 Chocolate Milkshake, guest didn't ask) — heard "What can I get started for you? | 2 Chocolate Shakes"
- ho-shake-mods--headset--clean: false alarm on correct ticket (Said 2 Chocolate Milkshake, rang 1; Rang 1 Chocolate Milkshake, guest didn't ask) — heard "What can I get started for you? | 2 Chocolate Shakes"
- ho-shake-mods--headset--clean: missed planted count (Mismatch) — heard "What can I get started for you? | 2 Chocolate Shakes"
- ho-shake-mods--headset--clean: missed planted modifier (Match) — heard "What can I get started for you? | 2 Chocolate Shakes"
- ho-shake-mods--headset--clean: missed planted missing (Mismatch) — heard "What can I get started for you? | 2 Chocolate Shakes"
- ho-shake-mods--headset--clean: missed planted extra (Mismatch) — heard "What can I get started for you? | 2 Chocolate Shakes"
- ho-shake-mods--headset--snr15: extraction differs from truth (Rang 1 Chocolate Milkshake, guest didn't ask; Rang 1 Chocolate Milkshake, guest didn't ask) — heard "What can I get started for you? | 2 Chocuchit"
- ho-shake-mods--headset--snr15: missed planted variant (Unconfirmed) — heard "What can I get started for you? | 2 Chocuchit"
- ho-shake-mods--headset--snr15: missed planted count (Unconfirmed) — heard "What can I get started for you? | 2 Chocuchit"
- ho-shake-mods--headset--snr15: missed planted modifier (Unconfirmed) — heard "What can I get started for you? | 2 Chocuchit"
- ho-shake-mods--headset--snr15: missed planted missing (Unconfirmed) — heard "What can I get started for you? | 2 Chocuchit"
- ho-shake-mods--headset--snr15: missed planted extra (Unconfirmed) — heard "What can I get started for you? | 2 Chocuchit"
- ho-shake-mods--headset--snr5: extraction differs from truth (Rang 1 Chocolate Milkshake, guest didn't ask; Rang 1 Chocolate Milkshake, guest didn't ask) — heard "What's the amount that started for me? Who talked with me?"
- ho-shake-mods--headset--snr5: missed planted variant (Unconfirmed) — heard "What's the amount that started for me? Who talked with me?"
- ho-shake-mods--headset--snr5: missed planted count (Unconfirmed) — heard "What's the amount that started for me? Who talked with me?"
- ho-shake-mods--headset--snr5: missed planted modifier (Unconfirmed) — heard "What's the amount that started for me? Who talked with me?"
- ho-shake-mods--headset--snr5: missed planted missing (Unconfirmed) — heard "What's the amount that started for me? Who talked with me?"
- ho-shake-mods--headset--snr5: missed planted extra (Unconfirmed) — heard "What's the amount that started for me? Who talked with me?"
- ho-strips-family--face_to_face--snr15: extraction differs from truth (Rang 1 4 ct Strips Meal, guest didn't ask; Rang 2 Barbeque, guest didn't ask) — heard "Well, Chick-fil-A | High, We get a 4-count Strix meal with a medium lemonade, and an 8-count Nugget meal with a medium sweet tea, | 4-count Strix meal with lemonade, 8-count Nugget meal with sweet tea, both medium, and 2-par pickle sauces,"
- ho-strips-family--face_to_face--snr15: false alarm on correct ticket (Rang 1 4 ct Strips Meal, guest didn't ask; Rang 2 Barbeque, guest didn't ask) — heard "Well, Chick-fil-A | High, We get a 4-count Strix meal with a medium lemonade, and an 8-count Nugget meal with a medium sweet tea, | 4-count Strix meal with lemonade, 8-count Nugget meal with sweet tea, both medium, and 2-par pickle sauces,"
- ho-strips-family--face_to_face--snr15: missed planted count (Mismatch) — heard "Well, Chick-fil-A | High, We get a 4-count Strix meal with a medium lemonade, and an 8-count Nugget meal with a medium sweet tea, | 4-count Strix meal with lemonade, 8-count Nugget meal with sweet tea, both medium, and 2-par pickle sauces,"
- ho-strips-family--face_to_face--snr5: extraction differs from truth (Rang 1 4 ct Strips Meal, guest didn't ask; Rang 1 Medium Lemonade, guest didn't ask; Rang 1 8 ct Nuggets Meal, guest didn't ask; Rang 1 Medium Sweet Tea, guest didn't ask; Rang 2 Barbeque, guest didn't ask) — heard "No Pizza, No Sausage, No Pizza, No"
- ho-strips-family--face_to_face--snr5: missed planted variant (Unconfirmed) — heard "No Pizza, No Sausage, No Pizza, No"
- ho-strips-family--face_to_face--snr5: missed planted count (Unconfirmed) — heard "No Pizza, No Sausage, No Pizza, No"
- ho-strips-family--face_to_face--snr5: missed planted size (Unconfirmed) — heard "No Pizza, No Sausage, No Pizza, No"
- ho-strips-family--face_to_face--snr5: missed planted missing (Unconfirmed) — heard "No Pizza, No Sausage, No Pizza, No"
- ho-strips-family--face_to_face--snr5: missed planted extra (Unconfirmed) — heard "No Pizza, No Sausage, No Pizza, No"
- ho-strips-family--headset--clean: extraction differs from truth (Rang 2 Barbeque, guest didn't ask) — heard "Welcome to Chick-fil-A | 4 ct Strips, Meal with Lemonade, 8 ct Nugget, Meal with Leet Tea, Both Medium"
- ho-strips-family--headset--clean: false alarm on correct ticket (Rang 2 Barbeque, guest didn't ask) — heard "Welcome to Chick-fil-A | 4 ct Strips, Meal with Lemonade, 8 ct Nugget, Meal with Leet Tea, Both Medium"
- ho-strips-family--headset--clean: missed planted count (Mismatch) — heard "Welcome to Chick-fil-A | 4 ct Strips, Meal with Lemonade, 8 ct Nugget, Meal with Leet Tea, Both Medium"
- ho-strips-family--headset--snr15: extraction differs from truth (Said 4 CT STRIPS, rang 4 CT STRIPS MEAL; Said 8 CT NUGGETS, rang 8 CT NUGGETS MEAL; Rang 2 Barbeque, guest didn't ask) — heard "Welcome to Chick-fil-A | 4 ct Strips, Me with Lemonade, 8 ct Nugget, Me with Sleep Tea, Both Medium,"
- ho-strips-family--headset--snr15: false alarm on correct ticket (Said 4 CT STRIPS, rang 4 CT STRIPS MEAL; Said 8 CT NUGGETS, rang 8 CT NUGGETS MEAL; Rang 2 Barbeque, guest didn't ask) — heard "Welcome to Chick-fil-A | 4 ct Strips, Me with Lemonade, 8 ct Nugget, Me with Sleep Tea, Both Medium,"
- ho-strips-family--headset--snr15: missed planted count (Mismatch) — heard "Welcome to Chick-fil-A | 4 ct Strips, Me with Lemonade, 8 ct Nugget, Me with Sleep Tea, Both Medium,"
- ho-strips-family--headset--snr5: extraction differs from truth (Said NUGGETS, rang 8 CT NUGGETS MEAL; Rang 1 4 ct Strips Meal, guest didn't ask; Rang 2 Barbeque, guest didn't ask) — heard "No Food To Pick The Way, 4 Coms, Spirit Seal, Lemonade, 8 Coms, Nugget, and Lizzie Tea, Cold Leaping"
- ho-strips-family--headset--snr5: false alarm on correct ticket (Said NUGGETS, rang 8 CT NUGGETS MEAL; Rang 1 4 ct Strips Meal, guest didn't ask; Rang 2 Barbeque, guest didn't ask) — heard "No Food To Pick The Way, 4 Coms, Spirit Seal, Lemonade, 8 Coms, Nugget, and Lizzie Tea, Cold Leaping"
- ho-strips-family--headset--snr5: missed planted count (Mismatch) — heard "No Food To Pick The Way, 4 Coms, Spirit Seal, Lemonade, 8 Coms, Nugget, and Lizzie Tea, Cold Leaping"
- ho-strips-family--headset--snr5: missed planted size (Mismatch) — heard "No Food To Pick The Way, 4 Coms, Spirit Seal, Lemonade, 8 Coms, Nugget, and Lizzie Tea, Cold Leaping"
- ho-two-sizes--face_to_face--clean: extraction differs from truth (Rang 1 Large Lemonade, guest didn't ask) — heard "Welcome, What Can I Get For You? | I Need Delirich Lemonade & A Small Lemonade | One Large & One Small Lemonade"
- ho-two-sizes--face_to_face--clean: false alarm on correct ticket (Rang 1 Large Lemonade, guest didn't ask) — heard "Welcome, What Can I Get For You? | I Need Delirich Lemonade & A Small Lemonade | One Large & One Small Lemonade"
- ho-two-sizes--face_to_face--clean: missed planted variant (Mismatch) — heard "Welcome, What Can I Get For You? | I Need Delirich Lemonade & A Small Lemonade | One Large & One Small Lemonade"
- ho-two-sizes--face_to_face--clean: missed planted size (Mismatch) — heard "Welcome, What Can I Get For You? | I Need Delirich Lemonade & A Small Lemonade | One Large & One Small Lemonade"
- ho-two-sizes--face_to_face--clean: missed planted missing (Match) — heard "Welcome, What Can I Get For You? | I Need Delirich Lemonade & A Small Lemonade | One Large & One Small Lemonade"
- ho-two-sizes--face_to_face--clean: missed planted extra (Unconfirmed) — heard "Welcome, What Can I Get For You? | I Need Delirich Lemonade & A Small Lemonade | One Large & One Small Lemonade"
- ho-two-sizes--face_to_face--snr5: missed planted modifier (Unconfirmed) — heard "Woken, What's the most good for you? I need a large lemonade and a small lemonade. One large and one small lemonade."
- ho-two-sizes--face_to_face--snr5: missed planted extra (Unconfirmed) — heard "Woken, What's the most good for you? I need a large lemonade and a small lemonade. One large and one small lemonade."
- ho-two-sizes--headset--clean: extraction differs from truth (Rang 1 Large Lemonade, guest didn't ask) — heard "Welcome, What Can I Get For You? | One Large and One Small Lemonade"
- ho-two-sizes--headset--clean: false alarm on correct ticket (Rang 1 Large Lemonade, guest didn't ask) — heard "Welcome, What Can I Get For You? | One Large and One Small Lemonade"
- ho-two-sizes--headset--clean: missed planted variant (Mismatch) — heard "Welcome, What Can I Get For You? | One Large and One Small Lemonade"
- ho-two-sizes--headset--clean: missed planted size (Mismatch) — heard "Welcome, What Can I Get For You? | One Large and One Small Lemonade"
- ho-two-sizes--headset--clean: missed planted missing (Match) — heard "Welcome, What Can I Get For You? | One Large and One Small Lemonade"
- ho-two-sizes--headset--clean: missed planted extra (Unconfirmed) — heard "Welcome, What Can I Get For You? | One Large and One Small Lemonade"
- ho-two-sizes--headset--snr15: extraction differs from truth (Rang 1 Large Lemonade, guest didn't ask) — heard "Wilkum, What Can I Get For You? | One large and one small lemonade."
- ho-two-sizes--headset--snr15: false alarm on correct ticket (Rang 1 Large Lemonade, guest didn't ask) — heard "Wilkum, What Can I Get For You? | One large and one small lemonade."
- ho-two-sizes--headset--snr15: missed planted variant (Mismatch) — heard "Wilkum, What Can I Get For You? | One large and one small lemonade."
- ho-two-sizes--headset--snr15: missed planted size (Mismatch) — heard "Wilkum, What Can I Get For You? | One large and one small lemonade."
- ho-two-sizes--headset--snr15: missed planted missing (Match) — heard "Wilkum, What Can I Get For You? | One large and one small lemonade."
- ho-two-sizes--headset--snr15: missed planted extra (Unconfirmed) — heard "Wilkum, What Can I Get For You? | One large and one small lemonade."
- ho-two-sizes--headset--snr5: extraction differs from truth (Rang 1 Large Lemonade, guest didn't ask) — heard "Witten, What Can I Get For You? One Large and One Small Lemonade"
- ho-two-sizes--headset--snr5: missed planted variant (Unconfirmed) — heard "Witten, What Can I Get For You? One Large and One Small Lemonade"
- ho-two-sizes--headset--snr5: missed planted size (Mismatch) — heard "Witten, What Can I Get For You? One Large and One Small Lemonade"
- ho-two-sizes--headset--snr5: missed planted modifier (Unconfirmed) — heard "Witten, What Can I Get For You? One Large and One Small Lemonade"
- ho-two-sizes--headset--snr5: missed planted missing (Match) — heard "Witten, What Can I Get For You? One Large and One Small Lemonade"
- ho-two-sizes--headset--snr5: missed planted extra (Unconfirmed) — heard "Witten, What Can I Get For You? One Large and One Small Lemonade"
- ho-wrap-and-salad--face_to_face--snr15: extraction differs from truth (Rang Light Ice on Diet Cola, guest didn't ask) — heard "High Welcome | I'd like a Cool Wrap, a Side Salad, and a Large Diet Coke with Light Eyes | My pleasure"
- ho-wrap-and-salad--face_to_face--snr15: false alarm on correct ticket (Rang Light Ice on Diet Cola, guest didn't ask) — heard "High Welcome | I'd like a Cool Wrap, a Side Salad, and a Large Diet Coke with Light Eyes | My pleasure"
- ho-wrap-and-salad--face_to_face--snr15: missed planted modifier (Match) — heard "High Welcome | I'd like a Cool Wrap, a Side Salad, and a Large Diet Coke with Light Eyes | My pleasure"
