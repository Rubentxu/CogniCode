# Smelly: SSL verification disabled
import requests
def fetch_data():
    return requests.get("https://example.com", verify=False)
