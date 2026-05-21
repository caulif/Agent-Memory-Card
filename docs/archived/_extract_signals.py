"""
从 claude code 历史 jsonl 抽出 user message。
目标：找到真正反复出现、跨多会话的"长期偏好/约束"原话。
"""

import json
import re
from pathlib import Path
from collections import defaultdict, Counter

DIR = Path(r"C:\Users\15893\.claude\projects\C--Users-15893-Documents-New-project")

# 信号词（任意命中之一即纳入候选）
SIGNAL = re.compile(
    r"以后|每次|必须|统一|默认|一律|不要再|别再|禁止|"
    r"原则|规范|约定|惯例|标准|偏好|要求你|希望你|"
    r"以后都|从现在起|一定要|永远|不应该|应当"
)

# 排除：明显的本任务指令开头
TASK_OPEN = re.compile(
    r"^(帮我|请帮我|帮忙|修一下|改一下|加一下|实现一下|"
    r"看一下|分析一下|总结一下|你能|怎么|为什么|如何|"
    r"请把|请你|继续|接着|然后|现在|先|再)"
)

# 排除：粘贴的系统/错误输出
PASTE = re.compile(
    r"^(<system|Traceback|error:|Error:|warning:|"
    r"cargo|npm |bun |error\[|warning\[|panic|"
    r"\$|>>> |--- |\+\+\+ |@@ )"
)

def extract_text(content):
    if isinstance(content, str):
        return content
    if isinstance(content, list):
        parts = []
        for item in content:
            if isinstance(item, dict) and item.get("type") == "text":
                parts.append(item.get("text", ""))
        return "\n".join(parts)
    return ""

def collect():
    seen_text = defaultdict(list)
    for path in sorted(DIR.glob("*.jsonl")):
        sid = path.stem[:8]
        with path.open("r", encoding="utf-8") as f:
            for line in f:
                line = line.strip()
                if not line:
                    continue
                try:
                    rec = json.loads(line)
                except Exception:
                    continue
                if rec.get("type") != "user":
                    continue
                msg = rec.get("message") or {}
                if msg.get("role") != "user":
                    continue
                text = extract_text(msg.get("content")).strip()
                if not text or len(text) < 8 or len(text) > 800:
                    continue
                # 命中信号词
                if not SIGNAL.search(text):
                    continue
                # 拍平
                snippet = re.sub(r"\s+", " ", text).strip()
                if len(snippet) > 250:
                    snippet = snippet[:250] + "..."
                seen_text[snippet].append(sid)
    return seen_text

def categorize(text):
    """粗分类，便于查看"""
    if PASTE.match(text):
        return "PASTE"
    if TASK_OPEN.match(text):
        return "TASK"
    if SIGNAL.search(text):
        return "RULE"
    return "OTHER"

def main():
    cands = collect()
    items = sorted(cands.items(), key=lambda kv: (-len(kv[1]), kv[0]))
    bucket = defaultdict(list)
    for text, sessions in items:
        bucket[categorize(text)].append((text, sessions))

    print(f"# 总候选: {sum(len(v) for v in bucket.values())}")
    for cat in ["RULE", "TASK", "PASTE", "OTHER"]:
        rows = bucket.get(cat, [])
        print(f"\n# === {cat} (n={len(rows)}) ===\n")
        for text, sessions in rows[:80]:
            print(f"[{len(sessions)}x] {text}")

if __name__ == "__main__":
    main()
