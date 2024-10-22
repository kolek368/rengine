package main

import (
  "log"
  "net/http"
)

func main() {
  log.Println("rengine backend started")
  http.HandleFunc("/", func(w http.ResponseWriter, r *http.Request) {
    w.Write([]byte("Hello, World!\n"))
  })

  err := http.ListenAndServe(":8080", nil)
  if err != nil {
    log.Fatalln("Failed to start server")
  }
}
