import json
import csv
import os

def load_freq_map(csv_path):
    freq_map = {}
    try:
        with open(csv_path, 'r', encoding='utf-8') as f:
            # Skip potential BOM or weird encoding issues by using utf-8-sig if needed, 
            # but let's try standard reader first.
            reader = csv.DictReader(f)
            # The CSV has some spaces in headers sometimes, let's be careful
            # Headers: serial number,character,token,ferquency(per million),total coverage rate(%)
            for row in reader:
                char = row.get('character')
                rank = row.get('\xa0serial number') or row.get('serial number')
                if char and rank:
                    freq_map[char] = int(rank)
    except Exception as e:
        print(f"Error loading CSV: {e}")
    return freq_map

def reorder_file(file_path, freq_map):
    print(f"Reordering {file_path}...")
    try:
        with open(file_path, 'r', encoding='utf-8') as f:
            data = json.load(f)
        
        if not isinstance(data, dict):
            return

        for py in data:
            entries = data[py]
            if isinstance(entries, list):
                # Sort entries based on frequency rank of 'char'
                # If char not in map, give it a very high rank (99999)
                entries.sort(key=lambda x: freq_map.get(x.get('char'), 99999))
        
        with open(file_path, 'w', encoding='utf-8') as f:
            json.dump(data, f, ensure_ascii=False, indent=2)
            
    except Exception as e:
        print(f"Error processing {file_path}: {e}")

def main():
    csv_path = 'dicts/chinese/character/Chinese character list from 2.5 billion words corpus ordered by frequency.csv'
    freq_map = load_freq_map(csv_path)
    
    if not freq_map:
        # Fallback for the weird header in the snippet: \xa0serial number
        print("Retrying with manual header parsing...")
        try:
            with open(csv_path, 'r', encoding='utf-8') as f:
                lines = f.readlines()
                if lines:
                    header = lines[0].strip().split(',')
                    char_idx = -1
                    rank_idx = -1
                    for i, h in enumerate(header):
                        if 'character' in h: char_idx = i
                        if 'serial number' in h: rank_idx = i
                    
                    if char_idx != -1 and rank_idx != -1:
                        for line in lines[1:]:
                            parts = line.strip().split(',')
                            if len(parts) > max(char_idx, rank_idx):
                                char = parts[char_idx]
                                rank = parts[rank_idx]
                                try:
                                    freq_map[char] = int(rank)
                                except:
                                    continue
        except Exception as e:
            print(f"Manual parsing failed: {e}")

    if not freq_map:
        print("Failed to load frequency data.")
        return

    print(f"Loaded {len(freq_map)} characters from frequency list.")

    target_files = [
        'dicts/chinese/character/level-1_char_en.json',
        'dicts/chinese/character/level-2_char_en.json',
        'dicts/chinese/character/level-3_char_en.json'
    ]

    for f_path in target_files:
        if os.path.exists(f_path):
            reorder_file(f_path, freq_map)
        else:
            print(f"File not found: {f_path}")

if __name__ == "__main__":
    main()
