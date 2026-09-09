#!/usr/bin/env python3
"""Pull weight from Garmin Connect into fitness tracker."""

import json
import os
import sys
from datetime import date, timedelta
from pathlib import Path

try:
    import garminconnect
except ImportError:
    print("garminconnect not installed. Run: pip install garminconnect")
    sys.exit(1)

DATA_FILE = Path.home() / ".local/share/fitness/weights.json"
TOKEN_FILE = Path.home() / ".local/share/fitness/garmin_tokens.json"
KG_TO_LBS = 2.20462


def load():
    if not DATA_FILE.exists():
        return []
    return json.loads(DATA_FILE.read_text())


def save(entries):
    entries.sort(key=lambda e: e["date"])
    DATA_FILE.write_text(json.dumps(entries, indent=2))


def to_lbs(value):
    if value is None:
        return None
    # Garmin returns weight in grams
    return round((value / 1000) * KG_TO_LBS, 1)


def main():
    days = int(sys.argv[1]) if len(sys.argv) > 1 else 90

    email = os.environ.get("GARMIN_EMAIL") or input("Garmin email: ")
    password = os.environ.get("GARMIN_PASSWORD") or input("Garmin password: ")

    print("Connecting to Garmin Connect...")
    try:
        client = garminconnect.Garmin(
            email, password,
            prompt_mfa=lambda: input("Garmin MFA code: "),
        )
        client.login(tokenstore_path=str(TOKEN_FILE))
    except garminconnect.GarminConnectAuthenticationError as e:
        print(f"Authentication failed: {e}")
        sys.exit(1)
    except Exception as e:
        print(f"Login error: {e}")
        sys.exit(1)

    end = date.today()
    start = end - timedelta(days=days)
    print(f"Fetching weight data {start} → {end}...")

    try:
        data = client.get_weigh_ins(start.isoformat(), end.isoformat())
    except Exception as e:
        print(f"Failed to fetch weigh-ins: {e}")
        sys.exit(1)

    summaries = data.get("dailyWeightSummaries", [])
    entries = load()
    existing = {e["date"] for e in entries}

    added = 0
    for s in summaries:
        date_str = s.get("summaryDate")
        # prefer lowInGrams (best of day), fall back to allDayAvg
        raw = s.get("lowInGrams") or s.get("allDayAvg")
        weight = to_lbs(raw)
        if not date_str or not weight:
            continue
        if date_str in existing:
            continue
        entries.append({"date": date_str, "weight": weight})
        added += 1

    save(entries)
    print(f"Done — {added} new entries added.")


if __name__ == "__main__":
    main()
