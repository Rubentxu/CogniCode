# Clean: Using secure HTTPS URL
import urllib.request

def fetch_data():
    url = "https://api.example.com/data"
    response = urllib.request.urlopen(url)
    return response.read()
