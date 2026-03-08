import json

# 模拟 syllables
syllables = {"zao", "shang", "zai", "sh", "ang"} # 简化版

def matches_segments(key, segments):
    current_key = key
    for i, seg in enumerate(segments):
        if not current_key: return False
        if i == len(segments) - 1:
            return current_key.startswith(seg)
        
        found = False
        # 模拟 Processor 的音节切分逻辑
        for length in range(len(current_key), 0, -1):
            syl = current_key[:length]
            if syl in ["zao", "zai", "shang", "ang", "sh"]: # 模拟 syllables.contains
                if syl.startswith(seg):
                    current_key = current_key[length:]
                    found = True
                    break
        if not found: return False
    return True

print(f"zaoshang with [z, shang]: {matches_segments('zaoshang', ['z', 'shang'])}")
print(f"zaishang with [z, shang]: {matches_segments('zaishang', ['z', 'shang'])}")
