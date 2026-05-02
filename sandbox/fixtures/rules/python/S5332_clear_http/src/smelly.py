# Smelly: Using clear HTTP URL
import urllib.request

def fetch_data():
    url = "http://api.example.com/data"
    response = urllib.request.urlopen(url)
    return response.read()
