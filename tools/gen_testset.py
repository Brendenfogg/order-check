#!/usr/bin/env python3
"""Builds the synthetic evaluation set from tools/conversations.py.

For each scripted conversation it renders every line with a macOS voice,
joins them with natural pauses, and writes clean and noisy versions:

    eval/cases/<conversation>--<situation>--<noise>/audio.wav
    eval/cases/<conversation>--<situation>--<noise>/case.json

Situations: face_to_face (both voices) and headset (employee only, for
conversations with a read-back). Noise: clean, snr15, snr5 (pink noise plus
quiet background chatter). Each case.json holds the correct ticket and one
ticket per kind of planted mistake that applies.

Usage: python3 tools/gen_testset.py --out eval/cases --seed 7
Requires macOS `say` and ffmpeg. Python stdlib only.
"""

import argparse
import array
import copy
import json
import math
import random
import re
import shutil
import subprocess
import sys
import tempfile
import tomllib
import wave
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from conversations import CONVERSATIONS  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
SR = 16000
CUSTOMER_VOICES = ["Samantha", "Daniel", "Karen", "Moira", "Fred", "Kathy", "Rishi",
                   "Rocko (English (US))", "Sandy (English (US))", "Grandpa (English (US))"]
EMPLOYEE_VOICES = ["Tessa", "Eddy (English (US))", "Flo (English (US))", "Reed (English (US))",
                   "Shelley (English (US))", "Ralph"]
NOISES = {"clean": None, "snr15": 15.0, "snr5": 5.0}
BABBLE = [
    "Did you see the game last night? It went to overtime.",
    "I think we're out of napkins at table six.",
    "Order up! Two number ones for the window.",
    "Can someone grab more ice from the back please.",
    "She said she'd be here around five thirty.",
    "That car has been sitting there for a while.",
]


def run(cmd):
    subprocess.run(cmd, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


def installed_voices():
    out = subprocess.run(["say", "-v", "?"], capture_output=True, text=True, check=True).stdout
    return {re.split(r"\s{2,}", line.strip())[0] for line in out.splitlines() if line.strip()}


def read_wav(path):
    with wave.open(str(path)) as w:
        assert w.getnchannels() == 1 and w.getsampwidth() == 2 and w.getframerate() == SR, path
        return array.array("h", w.readframes(w.getnframes()))


def write_wav(path, samples):
    with wave.open(str(path), "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(SR)
        w.writeframes(samples.tobytes())


def to_wav(src, dst):
    run(["ffmpeg", "-y", "-loglevel", "error", "-i", str(src), "-ar", str(SR), "-ac", "1", "-sample_fmt", "s16", str(dst)])


def speak(text, voice, tmp):
    aiff = tmp / "line.aiff"
    wav = tmp / "line.wav"
    run(["say", "-v", voice, "-o", str(aiff), text])
    to_wav(aiff, wav)
    return read_wav(wav)


def silence(ms):
    return array.array("h", bytes(int(SR * ms / 1000) * 2))


def rms(samples, active_only=False):
    vals = [s for s in samples if abs(s) > 300] if active_only else samples
    if not vals:
        return 1.0
    return math.sqrt(sum(s * s for s in vals) / len(vals))


def build_noise(tmp, voices, rng):
    pink = tmp / "pink.wav"
    run(["ffmpeg", "-y", "-loglevel", "error", "-f", "lavfi", "-i", f"anoisesrc=color=pink:sample_rate={SR}:duration=90",
         "-ac", "1", "-sample_fmt", "s16", str(pink)])
    pink = read_wav(pink)
    babble = array.array("h")
    for i, sentence in enumerate(BABBLE * 3):
        babble.extend(speak(sentence, voices[i % len(voices)], tmp))
        babble.extend(silence(rng.randint(100, 400)))
    p, b = rms(pink), rms(babble, active_only=True)
    n = len(pink)
    return [0.7 * pink[i] / p + 0.5 * babble[(i * 7) % len(babble)] / b for i in range(n)]


def mix(speech, noise, snr_db, rng):
    if snr_db is None:
        return speech
    target = rms(speech, active_only=True) / (10 ** (snr_db / 20))
    noise_rms = math.sqrt(sum(x * x for x in noise[:SR * 5]) / (SR * 5))
    gain = target / noise_rms
    start = rng.randrange(len(noise))
    out = array.array("h")
    for i, s in enumerate(speech):
        v = s + gain * noise[(start + i) % len(noise)]
        out.append(max(-32768, min(32767, int(v))))
    return out


# ---------------------------------------------------------------- tickets

def public(ticket_lines):
    """The ticket as the POS would send it (no generator bookkeeping)."""
    def clean(x):
        if isinstance(x, dict):
            return {k: clean(v) for k, v in x.items() if not k.startswith("_")}
        if isinstance(x, list):
            return [clean(v) for v in x]
        return x
    return {"lines": clean(ticket_lines)}


def family_members(menu, item_id):
    item = menu["items"][item_id]
    return [i for i, it in menu["items"].items() if it["family"] == item["family"] and i != item_id
            and it.get("meal", False) == item.get("meal", False)]


def mutations(truth, menu, rng):
    """One ticket per mistake kind that applies to this order."""
    out = []

    def variant(kind, fn):
        t = copy.deepcopy(truth)
        if fn(t):
            out.append((kind, t))

    def swap_variant(t):
        options = []
        for i, l in enumerate(t):
            if "item" in l["_said"] and family_members(menu, l["item"]):
                options.append((l, "item"))
            d = (l.get("meal") or {}).get("drink")
            if d and "item" in d.get("_said", []) and family_members(menu, d["item"]):
                options.append((d, "item"))
        if not options:
            return False
        target, key = rng.choice(options)
        target[key] = rng.choice(family_members(menu, target[key]))
        return True

    def change_count(t):
        options = [l for l in t if "qty" in l["_said"]]
        if not options:
            return False
        sauces = [l for l in options if menu["items"][l["item"]]["category"] == "sauce"]
        l = rng.choice(sauces or options)
        l["qty"] = l["qty"] + 1 if l["qty"] == 1 else l["qty"] - 1
        return True

    def change_size(t):
        options = [l for l in t if "size" in l["_said"] and menu["items"][l["item"]].get("sized")]
        for l in t:
            d = (l.get("meal") or {}).get("drink")
            if d and "size" in d.get("_said", []):
                options.append(d)
        if not options:
            return False
        l = rng.choice(options)
        l["size"] = rng.choice([s for s in ("sm", "md", "lg") if s != l["size"]])
        return True

    def change_modifier(t):
        with_mods = [x for l in t for x in (l, (l.get("meal") or {}).get("drink")) if x and x["modifiers"]]
        if with_mods:
            x = rng.choice(with_mods)
            x["modifiers"].remove(rng.choice(x["modifiers"]))
            return True
        candidates = [l for l in t if menu["items"][l["item"]].get("modifiers")]
        if not candidates:
            return False
        l = rng.choice(candidates)
        l["modifiers"].append(rng.choice(menu["items"][l["item"]]["modifiers"]))
        return True

    def drop_line(t):
        if len(t) < 2:
            return False
        t.pop(rng.randrange(len(t)))
        return True

    def add_extra(t):
        present = {l["item"] for l in t}
        for item in ("cookie", "honey_mustard", "fruit_cup"):
            if item not in present:
                size = "md" if menu["items"][item].get("sized") else None
                t.append({"item": item, "qty": 1, "size": size, "modifiers": [], "meal": None, "_said": []})
                return True
        return False

    variant("variant", swap_variant)
    variant("count", change_count)
    variant("size", change_size)
    variant("modifier", change_modifier)
    variant("missing", drop_line)
    variant("extra", add_extra)
    return out


def load_menu():
    with open(ROOT / "menus" / "chick-fil-a.toml", "rb") as f:
        raw = tomllib.load(f)
    return {"items": {i["id"]: i for i in raw["items"]}}


# ---------------------------------------------------------------- main

def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--out", default=str(ROOT / "eval" / "cases"))
    ap.add_argument("--seed", type=int, default=7)
    ap.add_argument("--only", help="generate a single conversation id")
    args = ap.parse_args()

    rng = random.Random(args.seed)
    menu = load_menu()
    have = installed_voices()
    customers = [v for v in CUSTOMER_VOICES if v in have]
    employees = [v for v in EMPLOYEE_VOICES if v in have]
    if not customers or not employees:
        sys.exit("no usable English voices found (see `say -v ?`)")

    out = Path(args.out)
    if out.exists() and not args.only:
        shutil.rmtree(out)
    out.mkdir(parents=True, exist_ok=True)

    convs = [c for c in CONVERSATIONS if not args.only or c["id"] == args.only]
    with tempfile.TemporaryDirectory() as tmpdir:
        tmp = Path(tmpdir)
        noise = build_noise(tmp, customers + employees, rng)
        written = 0
        for n, conv in enumerate(convs):
            truth = conv["truth"]
            muts = mutations(truth, menu, rng)
            variants = [{"name": "correct", "ticket": public(truth), "planted": []}]
            variants += [{"name": kind, "ticket": public(t), "planted": [kind]} for kind, t in muts]
            situations = ["face_to_face"] + (["headset"] if conv["has_readback"] else [])
            for situation in situations:
                guest, staff = customers[n % len(customers)], employees[n % len(employees)]
                speech = silence(300)
                for who, text in conv["lines"]:
                    if situation == "headset" and who == "c":
                        continue
                    speech.extend(speak(text, guest if who == "c" else staff, tmp))
                    speech.extend(silence(rng.randint(400, 900)))
                for noise_name, snr in NOISES.items():
                    case_id = f"{conv['id']}--{situation}--{noise_name}"
                    d = out / case_id
                    d.mkdir(parents=True, exist_ok=True)
                    write_wav(d / "audio.wav", mix(speech, noise, snr, rng))
                    case = {"id": case_id, "situation": situation, "noise": noise_name,
                            "truth": public(truth), "variants": variants}
                    (d / "case.json").write_text(json.dumps(case, indent=1))
                    written += 1
            print(f"{conv['id']}: {len(situations)} situation(s), {len(variants)} tickets", flush=True)
    print(f"wrote {written} cases to {out}")


if __name__ == "__main__":
    main()
