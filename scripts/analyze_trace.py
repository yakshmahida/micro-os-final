#!/usr/bin/env python3
"""
Serial execution trace log analyzer for Titanium Micro-OS.
Parses [SCHED] serial logs, extracts task residency, burst ratio Θ,
and computes workload metrics.
"""

import sys
import re

SCHED_PATTERN = re.compile(
    r"^\[SCHED\]\s+tick=(?P<tick>\d+)\s+task=(?P<task>\d+)\s+\((?P<name>\w+)\)\s+Q=(?P<queue>\d+)"
    r".*?state=(?P<state>\w+).*?exec=(?P<exec>\d+)ms.*?wait=(?P<wait>\d+)ms.*?yield=(?P<yield>\d+)ms"
    r".*?[Θtheta]=(?P<theta>[\d.]+)"
)

def analyze_trace(log_path):
    try:
        with open(log_path, "r", encoding="utf-8", errors="replace") as f:
            lines = f.readlines()
    except Exception as e:
        print(f"Error opening log file: {e}", file=sys.stderr)
        return 1

    records = []
    for line in lines:
        line = line.strip()
        m = SCHED_PATTERN.match(line)
        if m:
            records.append({
                "tick": int(m.group("tick")),
                "task": int(m.group("task")),
                "name": m.group("name"),
                "queue": int(m.group("queue")),
                "state": m.group("state"),
                "exec": int(m.group("exec")),
                "wait": int(m.group("wait")),
                "yield": int(m.group("yield")),
                "theta": float(m.group("theta")),
            })

    if not records:
        print("No valid [SCHED] trace records found.")
        return 0

    print("============================================================")
    print(" Titanium Micro-OS Serial Trace Analysis Report")
    print(f" Parsed {len(records)} scheduler execution records.")
    print("============================================================")

    # Task metrics
    tasks = {}
    for r in records:
        tid = r["task"]
        if tid not in tasks:
            tasks[tid] = {"name": r["name"], "ticks": 0, "thetas": []}
        tasks[tid]["ticks"] += 1
        tasks[tid]["thetas"].append(r["theta"])

    print(f"{'PID':<5} {'NAME':<12} {'TICKS':<8} {'AVG_THETA':<12} {'CLASS':<12}")
    print("-" * 55)
    for tid, data in sorted(tasks.items()):
        avg_theta = sum(data["thetas"]) / len(data["thetas"])
        cls = "Interactive" if avg_theta < 0.20 else "Compute"
        print(f"{tid:<5} {data['name']:<12} {data['ticks']:<8} {avg_theta:<12.4f} {cls:<12}")

    return 0

if __name__ == "__main__":
    path = sys.argv[1] if len(sys.argv) > 1 else "traces/qemu-boot.log"
    sys.exit(analyze_trace(path))
