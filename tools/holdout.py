"""Held-out test orders: written after tuning and never used to adjust the logic.

Scores on this set are the honest out-of-sample numbers. Don't tune against
it; if you change the logic because of a failure here, write a new held-out
set. Same format as conversations.py.
"""

from conversations import C, T, it, meal

CONVERSATIONS = [
    {"id": "ho-declined-meal", "has_readback": True, "lines": [
        (T, "Hi, welcome in. What can I get for you?"),
        (C, "Just a spicy deluxe sandwich, please."),
        (T, "Would you like to make that a meal today?"),
        (C, "No thanks, just the sandwich."),
        (T, "One spicy deluxe sandwich. My pleasure."),
    ], "truth": [it("spicy_deluxe")]},

    {"id": "ho-declined-size", "has_readback": True, "lines": [
        (T, "What can I get started for you?"),
        (C, "Can I have a medium sweet tea and a three count strips?"),
        (T, "Do you want to make that tea a large for just a little more?"),
        (C, "Nah, medium's fine."),
        (T, "Medium sweet tea and a three count strips."),
    ], "truth": [it("sweet_tea", size="md"), it("strips_3")]},

    {"id": "ho-actually-also", "has_readback": True, "lines": [
        (T, "Hi there, what can I get for you?"),
        (C, "A chicken sandwich meal with a large Dr Pepper."),
        (C, "Oh actually, can I also get a cookie?"),
        (T, "Chicken sandwich meal with a large Dr Pepper and a cookie."),
    ], "truth": [meal("chicken_sandwich", "dr_pepper", "lg"), it("cookie")]},

    {"id": "ho-free-reward", "has_readback": True, "lines": [
        (T, "What can I get for you today?"),
        (C, "I've got a reward for a free cookie, and I'll also take a small lemonade."),
        (T, "A cookie on the reward and a small lemonade."),
    ], "truth": [it("cookie"), it("lemonade", size="sm")]},

    {"id": "ho-two-sizes", "has_readback": True, "lines": [
        (T, "Welcome! What can I get for you?"),
        (C, "I need a large lemonade and a small lemonade."),
        (T, "One large and one small lemonade."),
    ], "truth": [it("lemonade", size="lg"), it("lemonade", size="sm")]},

    {"id": "ho-never-mind-swap", "has_readback": True, "lines": [
        (T, "What can I get for you?"),
        (C, "A fruit cup. Never mind, just a medium mac and cheese."),
        (T, "One medium mac and cheese."),
    ], "truth": [it("mac_and_cheese", size="md")]},

    {"id": "ho-grilled-meal-swap", "has_readback": True, "lines": [
        (T, "Hi, what can I get for you?"),
        (C, "Could I get a twelve count grilled nugget meal with a large unsweet tea?"),
        (C, "And three honey mustard."),
        (T, "Twelve count grilled nugget meal, large unsweet tea, three honey mustard."),
    ], "truth": [meal("grilled_nuggets_12", "unsweet_tea", "lg"), it("honey_mustard", 3)]},

    {"id": "ho-shake-mods", "has_readback": True, "lines": [
        (T, "What can I get started for you?"),
        (C, "Two chocolate milkshakes, one with no whipped cream."),
        (T, "Two chocolate shakes."),
    ], "truth": [it("chocolate_shake"), it("chocolate_shake", mods=["no_whipped_cream"])]},

    {"id": "ho-wrap-and-salad", "has_readback": False, "lines": [
        (T, "Hi, welcome!"),
        (C, "I'd like a cool wrap, a side salad, and a large Diet Coke with light ice."),
        (T, "My pleasure."),
    ], "truth": [it("cool_wrap"), it("side_salad"), it("diet_cola", size="lg", mods=["light_ice"])]},

    {"id": "ho-correction-count", "has_readback": True, "lines": [
        (T, "Any sauces for you?"),
        (C, "Two Polynesian. Actually make that three."),
        (T, "Three Polynesian sauces."),
    ], "truth": [it("polynesian", 3)]},

    {"id": "ho-deluxe-meal-sub", "has_readback": True, "lines": [
        (T, "What can I get for you?"),
        (C, "A deluxe chicken sandwich meal with no pickles, sub a fruit cup for the fries, and a medium Sunjoy."),
        (T, "Deluxe meal, no pickles, fruit cup, medium Sunjoy."),
    ], "truth": [meal("deluxe_sandwich", "sunjoy", "md", side="fruit_cup", mods=["no_pickles"])]},

    {"id": "ho-strips-family", "has_readback": True, "lines": [
        (T, "Welcome to Chick-fil-A."),
        (C, "Hi, can we get a four count strips meal with a medium lemonade, and an eight count nugget meal with a medium sweet tea?"),
        (T, "Four count strips meal with lemonade, eight count nugget meal with sweet tea, both medium."),
        (C, "And two barbeque sauces."),
    ], "truth": [meal("strips_4", "lemonade", "md"), meal("nuggets_8", "sweet_tea", "md"), it("barbeque", 2)]},
]
