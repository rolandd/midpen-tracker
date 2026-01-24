#!/usr/bin/env python3

# SPDX-License-Identifier: MIT
# Copyright 2026 Roland Dreier <roland@kernel.org>

# /// script
# requires-python = ">=3.13"
# dependencies = [
#     "contextily>=1.7.0",
#     "geopandas>=1.1.2",
#     "matplotlib>=3.10.8",
#     "osmnx>=2.0.7",
#     "requests>=2.32.5",
#     "shapely>=2.1.2",
# ]
# ///

import requests
import json
import os
import geopandas as gpd
import osmnx as ox
import matplotlib.pyplot as plt
import contextily as ctx
from shapely.geometry import shape, mapping
from shapely.ops import unary_union
from shapely.validation import make_valid

# Road buffer distance in meters
ROAD_BUFFER_METERS = 50


def main():
    print("1. Searching ArcGIS Catalog for Midpen's 'Preserve Boundary'...")

    # 1. Search
    search_url = "https://www.arcgis.com/sharing/rest/search"
    params = {
        "q": 'title:"Preserve Boundary" AND type:"Feature Service"',
        "f": "json",
        "num": 20,
    }
    headers = {"User-Agent": "Mozilla/5.0"}

    try:
        resp = requests.get(search_url, params=params, headers=headers)
        resp.raise_for_status()
        results = resp.json().get("results", [])

        target_service = None
        for item in results:
            txt = (
                item.get("owner", "")
                + item.get("snippet", "")
                + str(item.get("tags", ""))
            ).lower()
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
        name_field = find_field(
            all_fields, ["preserve", "rname", "name", "unit_name", "label"], "OBJECTID"
        )
        url_field = find_field(all_fields, ["webpageurl", "web_url", "url", "link"], "")

        print(f"   -> Selected Name Field: '{name_field}'")
        print(
            f"   -> Selected URL Field:  '{url_field if url_field else '(None Found)'}'"
        )

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
            "outSR": "4326",
        }

        print("3. Downloading data...")
        data_resp = requests.get(query_url, params=query_params, headers=headers)
        data_resp.raise_for_status()
        esri_data = data_resp.json()

        if "error" in esri_data:
            print(f"❌ API Error: {esri_data['error']}")
            return

        # 4. Convert & Save Raw
        count = len(esri_data.get("features", []))
        print(f"4. Downloaded {count} preserves. Converting to GeoJSON...")

        geojson = convert_esri_to_geojson(esri_data, name_field, url_field)

        if not os.path.exists("data"):
            os.makedirs("data")

        raw_path = "data/midpen_boundaries_raw.geojson"
        with open(raw_path, "w") as f:
            json.dump(geojson, f, indent=2)
        print(f"✅ Saved raw boundaries to {raw_path}")

        # 5. Process boundaries - subtract road buffers
        print("5. Processing boundaries - subtracting road buffers...")
        processed_geojson = subtract_roads_from_preserves(geojson)

        output_path = "data/midpen_boundaries.geojson"
        with open(output_path, "w") as f:
            json.dump(processed_geojson, f, indent=2)
        print(f"✅ Saved processed boundaries to {output_path}")

        # 6. Generate comparison images
        print("6. Generating before/after comparison images...")
        generate_comparison_images(geojson, processed_geojson)
        print("✅ Comparison images saved to data/preserve_comparisons/")

    except Exception as e:
        import traceback

        print(f"❌ Failed: {e}")
        traceback.print_exc()


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
        props = {"name": attr.get(name_key, "Unknown"), "id": attr.get("OBJECTID", 0)}

        # Only add URL if we found a valid field for it
        if url_key:
            props["url"] = attr.get(url_key, "")

        if "rings" in geometry:
            geo_feature = {
                "type": "Feature",
                "properties": props,
                "geometry": {"type": "Polygon", "coordinates": geometry["rings"]},
            }
            features.append(geo_feature)

    return {"type": "FeatureCollection", "features": features}


def subtract_roads_from_preserves(geojson):
    """
    For each preserve, download driveable roads from OSM and subtract a buffer around them.
    """
    processed_features = []

    for i, feature in enumerate(geojson["features"]):
        name = feature["properties"].get("name", "Unknown")
        print(f"   Processing {i + 1}/{len(geojson['features'])}: {name}")

        try:
            # Convert to shapely geometry
            geom = shape(feature["geometry"])

            # Get bounding box (minx, miny, maxx, maxy)
            bounds = geom.bounds

            # Download driveable roads from OSM within the bounding box
            # Using a slightly expanded bbox to catch roads at edges
            buffer_deg = 0.001  # ~100m buffer for road query
            # bbox format for osmnx 2.x: (west, south, east, north) = (minx, miny, maxx, maxy)
            bbox = (
                bounds[0] - buffer_deg,
                bounds[1] - buffer_deg,
                bounds[2] + buffer_deg,
                bounds[3] + buffer_deg,
            )

            road_geom = get_driveable_roads(bbox, name)

            if road_geom is not None and not road_geom.is_empty:
                # Create a GeoDataFrame to do the projection
                gdf = gpd.GeoDataFrame(geometry=[geom], crs="EPSG:4326")
                roads_gdf = gpd.GeoDataFrame(geometry=[road_geom], crs="EPSG:4326")

                # Project to a local UTM zone for accurate buffering
                utm_crs = gdf.estimate_utm_crs()
                gdf_utm = gdf.to_crs(utm_crs)
                roads_utm = roads_gdf.to_crs(utm_crs)

                # Make geometries valid before operations
                preserve_geom = gdf_utm.geometry.iloc[0]
                if not preserve_geom.is_valid:
                    preserve_geom = make_valid(preserve_geom)

                road_unified = roads_utm.geometry.iloc[0]
                if not road_unified.is_valid:
                    road_unified = make_valid(road_unified)

                # Buffer the roads by 50 meters
                road_buffer = road_unified.buffer(ROAD_BUFFER_METERS)

                # Subtract the road buffer from the preserve
                new_geom = preserve_geom.difference(road_buffer)

                # Validate result
                if not new_geom.is_valid:
                    new_geom = make_valid(new_geom)

                # Convert back to WGS84
                result_gdf = gpd.GeoDataFrame(geometry=[new_geom], crs=utm_crs)
                result_gdf = result_gdf.to_crs("EPSG:4326")
                final_geom = result_gdf.geometry.iloc[0]

                # Create the processed feature
                processed_feature = {
                    "type": "Feature",
                    "properties": feature["properties"].copy(),
                    "geometry": mapping(final_geom),
                }
            else:
                # No roads found, keep original geometry
                processed_feature = feature.copy()

        except Exception as e:
            print(f"      ⚠️  Warning: Could not process {name}: {e}")
            processed_feature = feature.copy()

        processed_features.append(processed_feature)

    return {"type": "FeatureCollection", "features": processed_features}


def get_driveable_roads(bbox, preserve_name=""):
    """
    Download driveable roads from OSM within the given bounding box.
    bbox: (west, south, east, north) = (minx, miny, maxx, maxy)
    Returns a unified geometry of all road LineStrings, or None if no roads found.
    """
    try:
        # Define road types that are driveable by cars
        tags = {
            "highway": [
                "motorway",
                "trunk",
                "primary",
                "secondary",
                "tertiary",
                "motorway_link",
                "trunk_link",
                "primary_link",
                "secondary_link",
                "tertiary_link",
                "residential",
                "unclassified",
            ]
        }

        # Download road features from OSM using the bounding box
        gdf = ox.features_from_bbox(bbox=bbox, tags=tags)

        if gdf.empty:
            print("      📍 No driveable roads found in area")
            return None

        # Filter to only LineString geometries (roads are lines, not polygons)
        gdf = gdf[gdf.geometry.type == "LineString"]

        if gdf.empty:
            print(
                "      📍 No driveable roads found in area (after filtering to LineStrings)"
            )
            return None

        # Debug output: show road names and stats
        print(f"      📍 Found {len(gdf)} road LineStrings")

        # Show highway type breakdown
        if "highway" in gdf.columns:
            highway_counts = gdf["highway"].value_counts()
            types_str = ", ".join(
                [f"{k}={v}" for k, v in highway_counts.head(5).items()]
            )
            print(f"         Types: {types_str}")

        # Show named roads (most relevant for our use case)
        if "name" in gdf.columns:
            named_roads = gdf["name"].dropna().unique()
            if len(named_roads) > 0:
                # Show first few road names
                sample_names = list(named_roads[:6])
                if len(named_roads) > 6:
                    print(
                        f"         Roads: {', '.join(sample_names)}, +{len(named_roads) - 6} more"
                    )
                else:
                    print(f"         Roads: {', '.join(sample_names)}")

        # Combine all road geometries into one
        all_roads = unary_union(gdf.geometry)
        return all_roads

    except Exception as e:
        print(f"      ⚠️  OSM query error: {e}")
        return None


def generate_comparison_images(raw_geojson, processed_geojson):
    """
    Generate before/after comparison images for each preserve.
    """
    output_dir = "data/preserve_comparisons"
    if not os.path.exists(output_dir):
        os.makedirs(output_dir)

    for i, (raw_feature, proc_feature) in enumerate(
        zip(raw_geojson["features"], processed_geojson["features"])
    ):
        name = raw_feature["properties"].get("name", f"preserve_{i}")
        safe_name = "".join(
            c if c.isalnum() or c in (" ", "-", "_") else "_" for c in name
        )

        try:
            # Create GeoDataFrames
            raw_geom = shape(raw_feature["geometry"])
            proc_geom = shape(proc_feature["geometry"])

            raw_gdf = gpd.GeoDataFrame(geometry=[raw_geom], crs="EPSG:4326")
            proc_gdf = gpd.GeoDataFrame(geometry=[proc_geom], crs="EPSG:4326")

            # Convert to Web Mercator for contextily
            raw_gdf = raw_gdf.to_crs(epsg=3857)
            proc_gdf = proc_gdf.to_crs(epsg=3857)

            # Create figure with two subplots
            fig, (ax1, ax2) = plt.subplots(1, 2, figsize=(16, 8))

            # Before image
            raw_gdf.plot(ax=ax1, facecolor="none", edgecolor="red", linewidth=2)
            ctx.add_basemap(ax1, source=ctx.providers.CartoDB.Positron, crs=raw_gdf.crs)
            ax1.set_title(f"{name} - Before (Raw)", fontsize=12)
            ax1.set_axis_off()

            # After image
            proc_gdf.plot(ax=ax2, facecolor="none", edgecolor="green", linewidth=2)
            ctx.add_basemap(
                ax2, source=ctx.providers.CartoDB.Positron, crs=proc_gdf.crs
            )
            ax2.set_title(f"{name} - After (Roads Subtracted)", fontsize=12)
            ax2.set_axis_off()

            plt.tight_layout()
            plt.savefig(f"{output_dir}/{safe_name}.png", dpi=150, bbox_inches="tight")
            plt.close(fig)

        except Exception as e:
            print(f"      ⚠️  Could not generate image for {name}: {e}")


if __name__ == "__main__":
    main()
