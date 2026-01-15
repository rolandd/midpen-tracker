#!/usr/bin/env python3

import requests
import json
import os

def main():
    print("1. Searching ArcGIS Catalog for Midpen's 'Preserve Boundary'...")

    # 1. Search
    search_url = "https://www.arcgis.com/sharing/rest/search"
    params = {
        "q": 'title:"Preserve Boundary" AND type:"Feature Service"',
        "f": "json",
        "num": 20
    }
    headers = {"User-Agent": "Mozilla/5.0"}

    try:
        resp = requests.get(search_url, params=params, headers=headers)
        resp.raise_for_status()
        results = resp.json().get("results", [])

        target_service = None
        for item in results:
            txt = (item.get("owner", "") + item.get("snippet", "") + str(item.get("tags", ""))).lower()
            if "midpen" in txt or "mrosd" in txt:
                target_service = item
                break

        if not target_service:
            print("❌ Could not find a Midpen-owned service.")
            return

        print(f"✅ Found Service: {target_service['title']}")
        base_url = target_service["url"]
        layer_url = f"{base_url}/0" 

        # 2. Inspect Metadata (Future-Proofing)
        print(f"2. Inspecting layer metadata at: {layer_url}")
        meta_resp = requests.get(f"{layer_url}?f=json", headers=headers)
        meta_data = meta_resp.json()

        all_fields = [f["name"] for f in meta_data.get("fields", [])]
        print(f"   ℹ️  Available Fields: {', '.join(all_fields)}")

        # Dynamic Field Detection
        name_field = find_field(all_fields, ["preserve", "rname", "name", "unit_name", "label"], "OBJECTID")
        url_field  = find_field(all_fields, ["webpageurl", "web_url", "url", "link"], "")

        print(f"   -> Selected Name Field: '{name_field}'")
        print(f"   -> Selected URL Field:  '{url_field if url_field else '(None Found)'}'")

        # 3. Download
        # We construct the outFields dynamically based on what we found
        fields_to_fetch = [name_field, "OBJECTID"]
        if url_field:
            fields_to_fetch.append(url_field)

        query_url = f"{layer_url}/query"
        query_params = {
            "where": "1=1",
            "outFields": ",".join(fields_to_fetch),
            "f": "json",
            "returnGeometry": "true",
            "outSR": "4326"
        }

        print(f"3. Downloading data...")
        data_resp = requests.get(query_url, params=query_params, headers=headers)
        data_resp.raise_for_status()
        esri_data = data_resp.json()

        if "error" in esri_data:
            print(f"❌ API Error: {esri_data['error']}")
            return

        # 4. Convert & Save
        count = len(esri_data.get('features', []))
        print(f"4. Downloaded {count} preserves. Converting to GeoJSON...")

        geojson = convert_esri_to_geojson(esri_data, name_field, url_field)

        if not os.path.exists("data"):
            os.makedirs("data")

        output_path = "data/midpen_boundaries.geojson"
        with open(output_path, "w") as f:
            json.dump(geojson, f, indent=2)

        print(f"✅ Success! Saved to {output_path}")

    except Exception as e:
        print(f"❌ Failed: {e}")

def find_field(available, candidates, fallback):
    """
    Helper to find the first matching field from a candidate list (case-insensitive).
    """
    for cand in candidates:
        match = next((f for f in available if f.lower() == cand), None)
        if match:
            return match
    return fallback

def convert_esri_to_geojson(esri_data, name_key, url_key):
    features = []
    for feature in esri_data.get("features", []):
        attr = feature.get("attributes", {})
        geometry = feature.get("geometry", {})

        # Robust property extraction
        props = {
            "name": attr.get(name_key, "Unknown"),
            "id": attr.get("OBJECTID", 0)
        }

        # Only add URL if we found a valid field for it
        if url_key:
            props["url"] = attr.get(url_key, "")

        if "rings" in geometry:
            geo_feature = {
                "type": "Feature",
                "properties": props,
                "geometry": {
                    "type": "Polygon",
                    "coordinates": geometry["rings"]
                }
            }
            features.append(geo_feature)

    return {"type": "FeatureCollection", "features": features}

if __name__ == "__main__":
    main()
