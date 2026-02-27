from playwright.sync_api import sync_playwright

def verify_activity_list_icon():
    with sync_playwright() as p:
        browser = p.chromium.launch(headless=True)
        page = browser.new_page()

        # 1. Navigate to the dashboard (demo mode bypasses auth)
        print("Navigating to dashboard...")
        page.goto("http://localhost:5173/dashboard")

        # 2. Open a preserve to see activities
        print("Waiting for dashboard preserves...")
        try:
            # Click on "Rancho San Antonio" to expand it
            preserve_btn = page.get_by_role("button", name="Rancho San Antonio")
            preserve_btn.wait_for()
            preserve_btn.click()
            print("Clicked Rancho San Antonio.")

            # Wait for activity list to appear
            page.wait_for_selector(".activity-list", timeout=10000)
            print("Activity list found.")
        except Exception as e:
            print(f"Error finding/clicking preserve: {e}")
            page.screenshot(path="error_state_2.png")
            browser.close()
            return


        # 3. Locate the first activity link
        activity_items = page.locator(".activity")
        count = activity_items.count()
        print(f"Found {count} activities.")

        if count > 0:
            activity_link = activity_items.first

            # 4. Focus on the link to verify focus styles
            activity_link.focus()

            # 5. Take a screenshot to verify the icon and focus state
            activity_link.screenshot(path="activity_link_icon.png")
            print("Screenshot saved to activity_link_icon.png")

            # 6. Verify the SVG icon exists
            svg_icon = activity_link.locator("svg")
            if svg_icon.count() > 0:
                print("SUCCESS: SVG icon found in activity link.")
            else:
                print("FAILURE: SVG icon NOT found.")
        else:
             print("No activities found in the list.")

        # 7. Take a full page screenshot for context
        page.screenshot(path="dashboard_full.png")
        print("Full page screenshot saved to dashboard_full.png")

        browser.close()

if __name__ == "__main__":
    verify_activity_list_icon()
