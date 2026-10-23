"""Scripted test orders with known correct tickets.

Each conversation is a list of (speaker, text) lines: "c" is the guest, "t" is
the team member. `truth` is the ticket a careful employee would ring in.
Lines built with said=... record what the guest actually specified, so the
generator only plants mistakes the check is supposed to catch (a size the
guest never said can't be "wrong").
"""


def it(item, qty=1, size=None, mods=(), said=("item", "qty", "size")):
    return {"item": item, "qty": qty, "size": size, "modifiers": list(mods), "meal": None, "_said": list(said)}


def meal(item, drink=None, drink_size="md", qty=1, mods=(), drink_mods=(), side="waffle_fries", side_size="md",
         said=("item", "qty"), drink_said=("item", "size")):
    m = {"item": item, "qty": qty, "size": None, "modifiers": list(mods), "_said": list(said),
         "meal": {"side": {"item": side, "size": side_size, "modifiers": []}, "drink": None}}
    if drink:
        m["meal"]["drink"] = {"item": drink, "size": drink_size, "modifiers": list(drink_mods), "_said": list(drink_said)}
    return m


C, T = "c", "t"

CONVERSATIONS = [
    {"id": "spicy-meal-poly", "has_readback": True, "lines": [
        (T, "Welcome to Chick-fil-A! What can I get started for you?"),
        (C, "Can I get a spicy chicken sandwich meal with a medium sweet tea?"),
        (T, "Absolutely. Would you like to try our seasonal peach milkshake today?"),
        (C, "No thanks. Oh, and can I get three Polynesian sauces?"),
        (T, "So that's a spicy chicken sandwich meal with a medium sweet tea and three Polynesian sauces?"),
        (C, "Yes, that's it."),
        (T, "My pleasure! Please pull forward."),
    ], "truth": [meal("spicy_sandwich", "sweet_tea", "md"), it("polynesian", 3)]},

    {"id": "nugget12-lemonade-cfa", "has_readback": True, "lines": [
        (T, "Hi there, what can I get for you today?"),
        (C, "Hi, can I get a twelve count nugget meal with a large lemonade?"),
        (T, "Sure. Any sauces with that?"),
        (C, "Two Chick-fil-A sauces please."),
        (T, "Okay, a twelve count nugget meal, large lemonade, and two Chick-fil-A sauces."),
        (C, "Perfect."),
    ], "truth": [meal("nuggets_12", "lemonade", "lg"), it("cfa_sauce", 2)]},

    {"id": "deluxe-no-tomato", "has_readback": True, "lines": [
        (T, "What can I get started for you?"),
        (C, "I'll do a deluxe chicken sandwich with no tomato, and a small unsweet tea."),
        (T, "A deluxe sandwich, no tomato, and a small unsweet tea. Anything else?"),
        (C, "That's all."),
        (T, "My pleasure."),
    ], "truth": [it("deluxe_sandwich", mods=["no_tomato"]), it("unsweet_tea", size="sm")]},

    {"id": "two-cookies-shake", "has_readback": True, "lines": [
        (T, "Welcome! What can I get for you?"),
        (C, "Can I get two cookies and a chocolate milkshake?"),
        (T, "Two cookies and a chocolate milkshake. Would you like whipped cream on the shake?"),
        (C, "No whipped cream, thanks."),
        (T, "Got it, two cookies and a chocolate shake, no whipped cream."),
    ], "truth": [it("cookie", 2), it("chocolate_shake", mods=["no_whipped_cream"])]},

    {"id": "grilled-nuggets-8-ranch", "has_readback": True, "lines": [
        (T, "Hi, what can I get for you?"),
        (C, "An eight count grilled nuggets with a garden herb ranch."),
        (T, "Eight count grilled nuggets and one garden herb ranch. Anything to drink?"),
        (C, "Just a medium diet lemonade."),
        (T, "Medium diet lemonade, you got it."),
    ], "truth": [it("grilled_nuggets_8"), it("garden_herb_ranch"), it("diet_lemonade", size="md")]},

    {"id": "correction-size", "has_readback": True, "lines": [
        (T, "What can I get started for you?"),
        (C, "Can I get a medium Sunjoy? Actually, make that a large."),
        (T, "A large Sunjoy. Anything else?"),
        (C, "And a waffle fries, medium."),
        (T, "Large Sunjoy and medium waffle fries."),
    ], "truth": [it("sunjoy", size="lg"), it("waffle_fries", size="md")]},

    {"id": "correction-variant", "has_readback": True, "lines": [
        (T, "Hi, what can I get for you today?"),
        (C, "Let me get a spicy chicken sandwich. No wait, regular, not spicy."),
        (T, "Okay, a regular chicken sandwich."),
        (C, "And four Polynesian sauces."),
        (T, "A chicken sandwich and four Polynesian. My pleasure."),
    ], "truth": [it("chicken_sandwich"), it("polynesian", 4)]},

    {"id": "make-it-a-meal", "has_readback": True, "lines": [
        (T, "Welcome, what can I get for you?"),
        (C, "Can I get an eight count nugget?"),
        (T, "Sure, would you like to make it a meal?"),
        (C, "Yeah, actually make it a meal, with a medium Dr Pepper."),
        (T, "Eight count nugget meal with a medium Dr Pepper."),
    ], "truth": [meal("nuggets_8", "dr_pepper", "md")]},

    {"id": "removal", "has_readback": True, "lines": [
        (T, "What can I get started for you?"),
        (C, "A cookie, a fruit cup, and a medium lemonade."),
        (C, "Actually, never mind the cookie."),
        (T, "Okay, a fruit cup and a medium lemonade."),
    ], "truth": [it("fruit_cup", size="md", said=("item", "qty")), it("lemonade", size="md")]},

    {"id": "lto-accepted", "has_readback": True, "lines": [
        (T, "Hi! Would you like to try our seasonal peach milkshake today?"),
        (C, "Sure, I'll try it."),
        (T, "Great. Anything else?"),
        (C, "And a three count strips meal with a small sweet tea."),
        (T, "A peach milkshake and a three count strips meal with a small sweet tea. My pleasure!"),
    ], "truth": [it("peach_shake"), meal("strips_3", "sweet_tea", "sm")]},

    {"id": "family-order", "has_readback": True, "lines": [
        (T, "Welcome to Chick-fil-A, what can I get for you?"),
        (C, "Okay, we need two spicy deluxe meals, both with large lemonades."),
        (C, "Then a twelve count nuggets, a large mac and cheese, and three honey mustard."),
        (T, "Two spicy deluxe meals with large lemonades, twelve count nuggets, large mac and cheese, three honey mustard."),
        (C, "That's right."),
    ], "truth": [meal("spicy_deluxe", "lemonade", "lg", qty=2), it("nuggets_12"), it("mac_and_cheese", size="lg"), it("honey_mustard", 3)]},

    {"id": "cool-wrap-water", "has_readback": False, "lines": [
        (T, "What can I get started for you?"),
        (C, "Just a cool wrap and a cup of water, no ice please."),
        (T, "My pleasure."),
    ], "truth": [it("cool_wrap"), it("water", mods=["no_ice"])]},

    {"id": "sandwich-add-cheese", "has_readback": True, "lines": [
        (T, "Hi, what can I get for you?"),
        (C, "A chicken sandwich meal with pepper jack, and a large Coke."),
        (T, "Chicken sandwich meal, add pepper jack, large Coke."),
    ], "truth": [meal("chicken_sandwich", "cola", "lg", mods=["add_pepper_jack"])]},

    {"id": "sauces-mix", "has_readback": True, "lines": [
        (T, "Anything else for you today?"),
        (C, "Can I get two barbeque sauces and one zesty buffalo?"),
        (T, "Two barbeque and one zesty buffalo."),
        (C, "And a medium waffle fries."),
    ], "truth": [it("barbeque", 2), it("zesty_buffalo"), it("waffle_fries", size="md")]},

    {"id": "frosted-lemonade", "has_readback": True, "lines": [
        (T, "What can I get started for you?"),
        (C, "Two frosted lemonades and a vanilla milkshake with no cherry."),
        (T, "Two frosted lemonades and a vanilla shake, no cherry."),
    ], "truth": [it("frosted_lemonade", 2), it("vanilla_shake", mods=["no_cherry"])]},

    {"id": "grilled-sandwich-meal", "has_readback": True, "lines": [
        (T, "Welcome! What can I get for you?"),
        (C, "A grilled chicken sandwich meal, but sub mac and cheese for the fries, and a medium unsweet tea with lemon."),
        (T, "Grilled chicken sandwich meal with mac and cheese and a medium unsweet tea with lemon."),
    ], "truth": [meal("grilled_sandwich", "unsweet_tea", "md", side="mac_and_cheese", drink_mods=["add_lemon"])]},

    {"id": "strips-4-sriracha", "has_readback": True, "lines": [
        (T, "Hi there!"),
        (C, "Hi, a four count chick n strips and two sweet and spicy sriracha sauces."),
        (T, "Four count strips and two sriracha."),
        (C, "And a small fruit cup."),
    ], "truth": [it("strips_4"), it("sriracha", 2), it("fruit_cup", size="sm")]},

    {"id": "unspecified-nuggets", "has_readback": False, "lines": [
        (T, "What can I get for you?"),
        (C, "Some nuggets and a lemonade please."),
        (T, "My pleasure."),
    ], "truth": [it("nuggets_8", said=()), it("lemonade", size="md", said=("item", "qty"))]},

    {"id": "spicy-deluxe-extra-pickles", "has_readback": True, "lines": [
        (T, "What can I get for you today?"),
        (C, "Can I get a spicy deluxe with extra pickles and no lettuce?"),
        (T, "Spicy deluxe, extra pickles, no lettuce. Anything else?"),
        (C, "A large sweet tea with light ice."),
        (T, "Large sweet tea, light ice."),
    ], "truth": [it("spicy_deluxe", mods=["extra_pickles", "no_lettuce"]), it("sweet_tea", size="lg", mods=["light_ice"])]},

    {"id": "kids-shakes", "has_readback": True, "lines": [
        (T, "Welcome!"),
        (C, "Hi, can we get a strawberry milkshake and a cookies and cream milkshake?"),
        (T, "A strawberry shake and a cookies and cream shake."),
        (C, "And one more cookies and cream."),
        (T, "So one strawberry and two cookies and cream."),
    ], "truth": [it("strawberry_shake"), it("cookies_cream_shake", 2)]},

    {"id": "honey-roasted", "has_readback": True, "lines": [
        (T, "What can I get started for you?"),
        (C, "An eight count nugget meal with a medium lemonade and two honey roasted barbecue sauces."),
        (T, "Eight count nugget meal, medium lemonade, two honey roasted barbecue."),
    ], "truth": [meal("nuggets_8", "lemonade", "md"), it("honey_roasted_bbq", 2)]},

    {"id": "side-salad", "has_readback": False, "lines": [
        (T, "Hi, what can I get for you?"),
        (C, "A side salad and a small Sprite."),
        (T, "My pleasure!"),
    ], "truth": [it("side_salad"), it("lemon_lime", size="sm")]},

    {"id": "twelve-grilled", "has_readback": True, "lines": [
        (T, "Welcome to Chick-fil-A."),
        (C, "Can I have a twelve count grilled nuggets and a large diet Coke?"),
        (T, "Twelve count grilled nuggets and a large Diet Coke. Would you like to try the peach milkshake?"),
        (C, "Not today."),
    ], "truth": [it("grilled_nuggets_12"), it("diet_cola", size="lg")]},

    {"id": "two-meals-different", "has_readback": True, "lines": [
        (T, "What can I get for you?"),
        (C, "One chicken sandwich meal with a medium sweet tea, and one spicy chicken sandwich meal with a medium lemonade."),
        (T, "A chicken sandwich meal with sweet tea and a spicy chicken sandwich meal with lemonade, both medium."),
    ], "truth": [meal("chicken_sandwich", "sweet_tea", "md"), meal("spicy_sandwich", "lemonade", "md")]},

    {"id": "big-sauce-count", "has_readback": True, "lines": [
        (T, "Anything else?"),
        (C, "Yeah, can I get four Chick-fil-A sauces and two Polynesian?"),
        (T, "Four Chick-fil-A sauce, two Polynesian."),
    ], "truth": [it("cfa_sauce", 4), it("polynesian", 2)]},

    {"id": "deluxe-meal-large", "has_readback": True, "lines": [
        (T, "Hi! What can I get for you?"),
        (C, "A large deluxe chicken sandwich meal with a Dr Pepper, no tomato."),
        (T, "Large deluxe meal with a Dr Pepper, no tomato."),
    ], "truth": [meal("deluxe_sandwich", "dr_pepper", "lg", side_size="lg", mods=["no_tomato"])]},

    {"id": "water-and-cookie", "has_readback": False, "lines": [
        (T, "What can I get started for you?"),
        (C, "Just a chocolate chunk cookie and a cup of water."),
        (T, "My pleasure."),
    ], "truth": [it("cookie"), it("water")]},

    {"id": "spicy-nuggets-mix", "has_readback": True, "lines": [
        (T, "Welcome!"),
        (C, "I'll get a spicy chicken sandwich and an eight count nuggets, and a large waffle fries."),
        (T, "Spicy chicken sandwich, eight count nuggets, large fries."),
        (C, "And three ranch."),
        (T, "Three garden herb ranch."),
    ], "truth": [it("spicy_sandwich"), it("nuggets_8"), it("waffle_fries", size="lg"), it("garden_herb_ranch", 3)]},

    {"id": "lemonade-variant", "has_readback": True, "lines": [
        (T, "Hi, what can I get for you?"),
        (C, "A medium diet lemonade and a medium regular lemonade."),
        (T, "One diet lemonade and one regular lemonade, both medium."),
    ], "truth": [it("diet_lemonade", size="md"), it("lemonade", size="md")]},

    {"id": "strips-meal-ice", "has_readback": True, "lines": [
        (T, "What can I get you?"),
        (C, "A four count strips meal with a large unsweet tea, no ice."),
        (T, "Four count strips meal, large unsweet tea, no ice."),
        (C, "And two buffalo sauces."),
    ], "truth": [meal("strips_4", "unsweet_tea", "lg", drink_mods=["no_ice"]), it("zesty_buffalo", 2)]},

    {"id": "quick-shake", "has_readback": False, "lines": [
        (C, "Can I just get a vanilla milkshake?"),
        (T, "Absolutely, my pleasure."),
    ], "truth": [it("vanilla_shake")]},

    {"id": "mac-and-sandwich", "has_readback": True, "lines": [
        (T, "Hi there, what can I get for you?"),
        (C, "A chicken sandwich with cheese, no pickles, and a medium mac and cheese."),
        (T, "Chicken sandwich, add cheese, no pickles, and a medium mac and cheese."),
    ], "truth": [it("chicken_sandwich", mods=["add_cheese", "no_pickles"]), it("mac_and_cheese", size="md")]},
]
