# Order Accuracy AI (working title)

**An AI listener that hears what the customer orders, checks it against what the employee rang in, and catches mistakes before the ticket reaches the kitchen.**

---

## The problem

Order-taking mistakes are caught too late. A wrong size, a missed modifier, or a forgotten sauce usually shows up at the drive-thru window, at the table, or as a complaint. By then the food has been made wrong, and the restaurant pays for it in remakes, wasted food, slower lines, comped meals, and bad reviews.

Nothing checks the gap between what the customer **said** and what the employee **entered**. That holds at the counter, in the drive-thru, and at the table.

A second problem: restaurants set service standards for how orders are taken, and those are hard to measure. At Chick-fil-A, that means saying "My pleasure" and offering limited-time menu items. Managers can only spot-check, so nobody has a reliable picture of whether employees are hitting those targets.

## The idea

1. **Listen.** Capture the conversation between the customer and the employee while the order is taken.
2. **Understand.** Turn speech into a structured order: items, quantities, sizes, modifiers, sauces, drinks.
3. **Cross-check.** Compare the spoken order with the ticket entered on the POS (Toast or similar).
4. **Fix before the kitchen.** If they don't match, alert the employee before the ticket fires to back of house so they can correct it.
5. **Track service standards.** Detect whether target phrases and offers happened ("My pleasure," the limited-time item offer) and report them per employee and per shift.

**Goal:** order-taking mistakes drop to almost zero, and employees hit their service metrics consistently.

## Who it's for

- **Large chains** with strict service standards (e.g., Chick-fil-A)
- **Small businesses and independent restaurants**
- **Restaurants that don't use software-based ordering at all** (paper tickets, basic registers)

## The big open question: software or wearable?

### Option A: Software on the POS
- Runs on Toast or other POS devices
- Reads the ticket directly from the POS
- Uses the device's microphone or a nearby one
- Easiest to roll out wherever the POS is already digital

### Option B: Wearable pin or clip
- Worn by the employee, so it goes wherever they take orders
- Works in restaurants with no digital ordering system
- **Bonus feature:** connects to the Toast expo screen (KDS) and **vibrates to tell a server when their food is ready.** That makes the whole restaurant faster and better organized, not just order-taking.

## Where the idea comes from

I worked at Chick-fil-A. I saw the order mistakes happen, and I saw how much weight gets put on lines like "My pleasure" and on offering limited-time items.

## Why I'm building it

It's a portfolio project for LinkedIn. It should show product thinking and technical execution, and help me land an internship.

## Questions to work through

- Software, wearable, or both? Which one makes the better demo?
- Without a digital POS, where does the "ticket" side of the comparison come from?
- Recording customers and employees raises consent and privacy questions, and the laws vary by state
- Kitchen and drive-thru noise: can speech recognition hold up?
- Which parts can be built as a working demo, and which stay as the pitched vision?
