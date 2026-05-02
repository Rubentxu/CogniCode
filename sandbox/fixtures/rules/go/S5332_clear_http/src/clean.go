// Clean: Using secure HTTPS URL
package main

import "net/http"

func fetchData() {
	resp, _ := http.Get("https://api.example.com/data")
	_ = resp
}
