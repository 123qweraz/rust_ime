
def encode_pair(pair):
    STROKE_PAIR_MAP = {
        "11": "g", "12": "f", "13": "d", "14": "s", "15": "a",
        "21": "h", "22": "j", "23": "k", "24": "l", "25": "m",
        "31": "t", "32": "r", "33": "e", "34": "w", "35": "q",
        "41": "y", "42": "u", "43": "i", "44": "o", "45": "p",
        "51": "n", "52": "b", "53": "v", "54": "c", "55": "x",
    }
    return STROKE_PAIR_MAP.get(pair, "x")

def encode_single(stroke):
    return {"1": "g", "2": "h", "3": "t", "4": "y", "5": "n"}.get(stroke, "x")

strokes = "32511354"
# 的: 32(r) 51(n) 13(d) 54(c) -> rndc? No, let's see.
# Wait, strokes is 32 51 13 54
# r(32), n(51), d(13), c(54) -> rndc?

print(f"'的' strokes: {strokes}")
print(f"r: {encode_pair('32')}, n: {encode_pair('51')}, d: {encode_pair('13')}, c: {encode_pair('54')}")

# Check 54 in map: 54 is 'c'
# So '的' should be 'rndc' in full code? 
# Wait, rebuild_bihuama_dict.py has py_initial too.
# code = rndcd (d for de)

# But in StrokeScheme, it's just pure stroke letters.
