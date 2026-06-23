#!/usr/bin/env python3
import json
import sys

BREAKS = {
    "hyphenation": [2, 6, 7],
    "dictionary": [3, 7],
    "extensive": [2, 5],
    "diagnostics": [2, 4, 7],
    "about": [],
}

for line in sys.stdin:
    if not line.strip():
        continue
    request = json.loads(line)
    word = request["word"]
    response = {
        "id": request.get("id"),
        "method": "external-jsonl-fixture",
        "breaks": BREAKS.get(word, []),
    }
    print(json.dumps(response), flush=True)
