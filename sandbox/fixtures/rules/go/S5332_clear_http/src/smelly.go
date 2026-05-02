// Smelly: Using clear HTTP URL
package main

import "net/http"

func fetchData() {
	resp, _ := http.Get("http://api.example.com/data")
	_ = resp
}
