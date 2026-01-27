# SPDX-License-Identifier: MIT
# Copyright 2026 Roland Dreier <roland@rolandd.dev>

import requests
import os
import sys
import json

def main():
    token = os.getenv("STRAVA_ACCESS_TOKEN")
    if not token:
        print("❌ STRAVA_ACCESS_TOKEN is missing.")
        sys.exit(1)

    print(f"🔑 Testing Token: {token[:6]}...{token[-4:]}")
    headers = {"Authorization": f"Bearer {token}"}

    # TEST 1: Get Athlete Profile (Who is this?)
    print("\n1. Fetching Athlete Profile...")
    r = requests.get("https://www.strava.com/api/v3/athlete", headers=headers)

    print(f"   Raw Response: {r.text}")
    print(f"   Raw Headers: {r.headers}")

    if r.status_code == 200:
        athlete = r.json()
        print(f"   ✅ SUCCESS. Token belongs to:")
        print(f"      Name: {athlete.get('firstname')} {athlete.get('lastname')}")
        print(f"      ID:   {athlete.get('id')}  <-- COMPARE THIS WITH YOUR PROFILE URL")
        print(f"      Username: {athlete.get('username')}")
    else:
        print(f"   ❌ FAILED to get profile. Status: {r.status_code}")
        print(f"      Response: {r.text}")
        sys.exit(1)

    # TEST 2: Check Token Scopes (from headers)
    # Strava sends scopes in the 'X-OAuth-Scopes' header of every successful request
    scopes = r.headers.get("X-OAuth-Scopes", "None provided")
    print(f"\n2. Token Scopes: [{scopes}]")
    
    if "activity:read" not in scopes and "activity:read_all" not in scopes:
        print("   ⚠️  WARNING: You are missing 'activity:read'. This token cannot fetch activities.")

    # TEST 3: List Last 3 Activities
    # This checks if we can see *any* activities at all.
    print("\n3. Listing last 3 activities for this athlete...")
    r2 = requests.get("https://www.strava.com/api/v3/athlete/activities?per_page=3", headers=headers)
    
    if r2.status_code == 200:
        activities = r2.json()
        print(f"   ✅ Found {len(activities)} activities.")
        for act in activities:
            print(f"      - ID: {act['id']} | Name: {act['name']}")
    else:
        print(f"   ❌ FAILED to list activities. Status: {r2.status_code}")

if __name__ == "__main__":
    main()
