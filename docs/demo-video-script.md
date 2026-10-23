# Order Check: demo video script

**Goal:** a 60–90 second screen recording for the case study page and LinkedIn. A recruiter should understand the problem, see a mistake get caught live, and see the honest result, all with the sound off (use captions or on-screen text) and with it on.

**Length:** about 75 seconds. **Format:** 1080p screen recording with your voice, exported as MP4 (H.264).

---

## Before you record

1. **Quiet room, mic close.** The live demo uses the Mac's microphone, so sit close and speak clearly. Do one dry run first.
2. **Fresh terminal:**
   ```bash
   cd ~/Projects/order-accuracy-ai
   export PATH="$(brew --prefix rustup)/bin:$PATH"
   rm -f data/demo-video.db
   ./target/release/order-check serve --demo-transcript --seed-demo-data \
       --db data/demo-video.db --ready-after 6
   ```
   `--demo-transcript` shows what the clip hears. `--ready-after 6` makes the "food ready" buzz come quickly so the video stays short.
3. **Browser:** open http://localhost:7080/pos.html in one tab and http://localhost:7080/dashboard.html in another. Zoom the browser so the POS fills the screen (about 90%). Hide bookmarks and close other tabs.
4. **Have the case study's results table open** in a third tab (or your website page) for the closing shot.
5. **Record with QuickTime:** File → New Screen Recording → Options → Microphone: MacBook Pro Microphone → record the browser window only.

---

## Shot list

| Time | On screen | What you do | What you say (voiceover) |
|---|---|---|---|
| 0:00–0:08 | POS, no order yet | Nothing; let it sit | "When I worked at Chick-fil-A, most order mistakes were tiny: spicy instead of regular, one sauce instead of three. I built a tool that catches them before the kitchen does." |
| 0:08–0:14 | POS | Tap **Drive-thru car** → **Navy** → **SUV** → **Start order**. The clip turns blue. | "Listening starts when the employee enters the car, and only then." |
| 0:14–0:24 | POS, transcript panel | **Say the guest's order out loud:** "Can I get a spicy chicken sandwich meal with a medium sweet tea, and three Polynesian sauces?" Wait for the text to appear in *What the clip hears*. | (Your guest line is the voiceover.) |
| 0:24–0:36 | POS | Ring it in **wrong**, the way a rushed employee would: **MEAL ON** → **Chicken Sandwich** (regular) → **Sweet Tea** → **Condiments** → **Polynesian** (once). Tap **Finalize**. The clip turns red and the alert appears. | "Now I'll ring it in the way it actually happens at rush: regular instead of spicy, one Polynesian instead of three. And before it goes to the kitchen…" (pause on the alert) "…it catches both." |
| 0:36–0:48 | POS | Tap **Fix it**. Select the sandwich line → **Cancel Item**. **MEAL ON** → **Spicy Chicken Sandwich** → **Sweet Tea**. Select the Polynesian line → **▲ ▲**. Tap **Finalize**. It sends; a few seconds later the clip buzzes green. | "The employee fixes it, the order goes through, and the clip buzzes again when the food is ready. The AI never changes the ticket. The person stays in charge." |
| 0:48–0:58 | Dashboard tab | Switch tabs, click **Last 7 days**, hover one bar. | "For the restaurant, everything rolls up into a dashboard: what kinds of mistakes, when, and which items get mixed up. It stores no conversations and no employee names." |
| 0:58–1:12 | Results table (case study page) | Scroll to the results table and hold. | "I tested it on over 1,500 spoken-order checks. On orders I tuned it on, it caught 96 percent of mistakes. On new orders it had never seen, false alarms jumped to a third. That told me exactly what to build next: real recorded orders and a learned model instead of hand-written rules." |
| 1:12–1:15 | Case study page header | Hold on the title | "Order Check. Code and full write-up at the link." |

**If the live check misses** (Whisper mishears you): cancel the order, re-record that segment, and speak a little slower. Don't fake the alert. The point of the video is that it's real.

---

## On-screen text (for sound-off viewers on LinkedIn)

Add these as captions or title overlays (iMovie: Titles → Lower Third):

1. 0:00 — "Small order mistakes cost restaurants remakes, waste, and time"
2. 0:14 — "The guest's order (spoken)"
3. 0:28 — "Rung in wrong: regular instead of spicy, 1 sauce instead of 3"
4. 0:33 — "Caught before the kitchen"
5. 0:50 — "Restaurant dashboard: no conversations or names stored"
6. 1:00 — "96% caught on tuned orders · 35% false alarms on new ones → next: real data + a learned model"

---

## Export and add to the website

1. QuickTime → File → Export As → 1080p. Then shrink it for the web:
   ```bash
   ffmpeg -i order-check-demo.mov -vcodec libx264 -crf 26 -preset slow \
          -vf "scale=1920:-2" -movflags +faststart -an order-check-demo-silent.mp4   # silent version for autoplay previews
   ffmpeg -i order-check-demo.mov -vcodec libx264 -crf 26 -preset slow \
          -vf "scale=1920:-2" -movflags +faststart -acodec aac -b:a 128k order-check-demo.mp4
   ```
   Aim for under about 25 MB.
2. Copy `order-check-demo.mp4` to `~/Projects/personal-website/_media/`.
3. In `~/Projects/personal-website/projects/order-check/index.html`, uncomment the `<video>` block in the Demo section.
4. **LinkedIn:** upload the MP4 directly to the post (LinkedIn plays native video better than links). Use the same on-screen captions.

## LinkedIn post (draft)

> When I worked at Chick-fil-A, most order mistakes weren't big. Spicy instead of regular. One Polynesian instead of three. They all get caught too late.
>
> So I built Order Check: an on-device AI "second set of ears" that compares what a guest says with what gets rung in, and flags the difference before the kitchen sees it. It doesn't replace the person taking the order; it backs them up.
>
> The most useful thing I learned came from measuring it honestly. On the orders I tuned it on, it caught 96% of mistakes. On new orders it had never seen, false alarms jumped to about a third. That gap told me exactly what to build next.
>
> Built with Rust and Whisper running locally. No audio saved, no employee data stored.
> Case study and code: [link]
