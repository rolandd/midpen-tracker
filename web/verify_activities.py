import sys
import os
import json
from playwright.sync_api import sync_playwright

def run_cuj(page):
    page.goto("http://localhost:5173/dashboard")
    page.wait_for_timeout(2000) # wait for svelte/app mount

    # Ensure page context has auth cookie to bypass login
    context = page.context
    context.add_cookies([{'name': 'jwt', 'value': 'dummy', 'domain': 'localhost', 'path': '/'}])
    page.reload()
    page.wait_for_timeout(2000)

    print("Clicking preserve header to expand activities...")
    # Find and click a preserve header to expand activities
    # The first preserve might be 0, but we want one with activities
    # Use playwright's locator to find a button with preserve-header-btn class
    preserve_btn = page.locator(".preserve-header-btn").first
    preserve_btn.click()
    page.wait_for_timeout(1000)

    print("Tab-navigating to focus the activity...")
    # Simulate keyboard navigation to focus the activity link
    # This will demonstrate the :focus-visible style
    page.keyboard.press("Tab")
    page.wait_for_timeout(500)
    page.keyboard.press("Tab")
    page.wait_for_timeout(500)
    page.keyboard.press("Tab")
    page.wait_for_timeout(500)

    # Alternatively directly focus to ensure we hit it:
    first_activity = page.locator(".activity").first
    if first_activity.count() > 0:
        first_activity.focus()
        print("Focused activity successfully.")

    page.wait_for_timeout(1000)

    print("Taking screenshot...")
    page.screenshot(path="/home/jules/verification/screenshots/verification.png")
    page.wait_for_timeout(1000)

if __name__ == "__main__":
    os.makedirs("/home/jules/verification/videos", exist_ok=True)
    os.makedirs("/home/jules/verification/screenshots", exist_ok=True)

    with sync_playwright() as p:
        browser = p.chromium.launch(headless=True)
        context = browser.new_context(
            record_video_dir="/home/jules/verification/videos"
        )
        page = context.new_page()
        try:
            run_cuj(page)
        except Exception as e:
            print(f"Error: {e}")
        finally:
            context.close()
            browser.close()
