#!/usr/bin/env python3

import requests
import json
import os
import sys
import argparse

def main():
    # 1. Get Token from Environment
    access_token = os.getenv("STRAVA_ACCESS_TOKEN")
    if not access_token:
        print("❌ Error: STRAVA_ACCESS_TOKEN environment variable is not set.")
        print("   Run: export STRAVA_ACCESS_TOKEN='your_token_here'")
        sys.exit(1)

    # 2. Parse Command Line Arguments
    parser = argparse.ArgumentParser(description="Download detailed Strava activity JSON.")
    parser.add_argument("activity_id", help="The ID of the Strava activity to fetch")
    args = parser.parse_args()

    activity_id = args.activity_id

    # 3. Setup Request
    url = f"https://www.strava.com/api/v3/activities/{activity_id}?include_all_efforts=false"
    headers = {"Authorization": f"Bearer {access_token}"}

    print(f"Fetching Activity {activity_id}...")

    try:
        response = requests.get(url, headers=headers)

        if response.status_code != 200:
            print(f"\n❌ API Request Failed: {response.status_code}")
            print(f"   Your Token Scopes: {response.headers.get('X-OAuth-Scopes', 'Unknown')}")

            # 2. Try to print the detailed JSON error from Strava
            try:
                error_body = response.json()
                print(f"   Error Details: {json.dumps(error_body, indent=2)}")
            except:
                print(f"   Raw Response: {response.text}")

            sys.exit(1)

        # Check for common Strava errors
        if response.status_code == 401:
            print("❌ Error: 401 Unauthorized. Your access token is likely expired or invalid.")
            sys.exit(1)
        elif response.status_code == 404:
            print(f"❌ Error: Activity {activity_id} not found.")
            sys.exit(1)

        response.raise_for_status()
        data = response.json()

        # 4. Save to File
        filename = f"activity_{activity_id}.json"
        with open(filename, "w") as f:
            json.dump(data, f, indent=2)

        # Check if we got the high-res polyline
        polyline_status = "✅ Found high-res polyline" if data.get("map", {}).get("polyline") else "⚠️  Warning: No detailed polyline found"

        print(f"✅ Saved to {filename}")
        print(f"   {polyline_status}")

    except requests.exceptions.RequestException as e:
        print(f"❌ Network Error: {e}")
        sys.exit(1)

if __name__ == "__main__":
    main()
