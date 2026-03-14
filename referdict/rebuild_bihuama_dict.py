import re
import json
from pathlib import Path

STROKE_PAIR_MAP = {
    "11": "g", "12": "f", "13": "d", "14": "s", "15": "a",
    "21": "h", "22": "j", "23": "k", "24": "l", "25": "m",
    "31": "t", "32": "r", "33": "e", "34": "w", "35": "q",
    "41": "y", "42": "u", "43": "i", "44": "o", "45": "p",
    "51": "n", "52": "b", "53": "v", "54": "c", "55": "x",
}

def parse_data_v2(js_path: Path) -> dict[str, str]:
    content = js_path.read_text(encoding="utf-8")
    pairs = re.findall(r'\[\s*"(.{1})"\s*,\s*"([1-5]+)"\s*\]', content)
    return {char: strokes for char, strokes in pairs}


def load_existing_pinyin(chars_txt: Path, rime_chars_json: Path) -> dict[str, str]:
    mapping: dict[str, str] = {}

    if rime_chars_json.exists():
        data = json.loads(rime_chars_json.read_text(encoding="utf-8"))
        for pinyin, entries in data.items():
            py = pinyin.strip().lower()
            if not py:
                continue
            for entry in entries:
                char = entry.get("char", "")
                if isinstance(char, str) and len(char) == 1 and char not in mapping:
                    mapping[char] = py

    if not chars_txt.exists():
        return mapping

    for line in chars_txt.read_text(encoding="utf-8").splitlines():
        parts = line.split("\t")
        if len(parts) < 2:
            continue
        pinyin, char = parts[0].strip().lower(), parts[1].strip()
        if pinyin and char and char not in mapping:
            mapping[char] = pinyin
    return mapping


def get_pinyin_initial(char: str, pinyin_map: dict[str, str]) -> str:
    py = pinyin_map.get(char, "")
    if py:
        c = py[0].lower()
        if "a" <= c <= "z":
            return c
    return "x"


def encode_pair(strokes: str) -> str:
    if len(strokes) >= 2:
        return STROKE_PAIR_MAP.get(strokes[:2], "x")
    return "x"


def encode_single(stroke: str) -> str:
    return {
        "1": "g",
        "2": "h",
        "3": "t",
        "4": "y",
        "5": "n",
    }.get(stroke, "x")


def build_code(strokes: str, py_initial: str) -> str:
    n = len(strokes)
    if n <= 0:
        stroke_code = "x"
    elif n == 1:
        # 1 笔：直接单字母（如 一->g, 乙->n）
        stroke_code = encode_single(strokes[0])
    elif n == 2:
        # 2 笔：直接两笔合一码
        stroke_code = encode_pair(strokes[:2])
    elif n == 3:
        # 3 笔：前两笔一码 + 最后一笔一码
        stroke_code = encode_pair(strokes[:2]) + encode_single(strokes[2])
    elif n == 4:
        # 4 笔：前两笔一码 + 后两笔一码
        stroke_code = encode_pair(strokes[:2]) + encode_pair(strokes[2:4])
    else:
        # 5 笔及以上：前 4 笔两码 + 最后两笔一码
        stroke_code = (
            encode_pair(strokes[:2])
            + encode_pair(strokes[2:4])
            + encode_pair(strokes[-2:])
        )

    # 末位始终拼音首字母
    return f"{stroke_code}{py_initial}"


def main() -> None:
    root = Path(__file__).resolve().parent
    data_v2 = root / "data_v2.js"
    bihua_txt = root / "bihua.txt"
    chars_txt = root / "chars.txt"
    rime_chars_json = root.parent / "dicts" / "chinese" / "chars" / "rime_mint_chars.json"

    stroke_map = parse_data_v2(data_v2)
    pinyin_map = load_existing_pinyin(chars_txt, rime_chars_json)

    bihua_lines = []
    dict_lines = []

    fallback_count = 0
    for char, strokes in stroke_map.items():
        bihua_lines.append(f"{char}\t{strokes}")
        py_initial = get_pinyin_initial(char, pinyin_map)
        if py_initial == "x" and char not in pinyin_map:
            fallback_count += 1
        code = build_code(strokes, py_initial)
        pinyin = pinyin_map.get(char, "")
        # 保持 4 列格式，第三列写入笔画序列
        dict_lines.append(f"{pinyin}\t{char}\t{strokes}\t{code}")

    bihua_txt.write_text("\n".join(bihua_lines) + "\n", encoding="utf-8")
    chars_txt.write_text("\n".join(dict_lines) + "\n", encoding="utf-8")

    print(f"done: {len(stroke_map)} chars")
    print(f"fallback pinyin initial (x): {fallback_count}")


if __name__ == "__main__":
    main()
