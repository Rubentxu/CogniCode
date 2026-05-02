# Clean: SSL verification enabled
import requests
def fetch_data():
    return requests.get("https://example.com", verify=True)
