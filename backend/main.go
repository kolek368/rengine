package main

import (
  "log"
  "net/http"
  "github.com/gorilla/websocket"
  "rengine-backend/mighty/pong"
  "google.golang.org/protobuf/proto"
)

var upgrader = websocket.Upgrader{
  ReadBufferSize:  1024,
  WriteBufferSize: 1024,
  CheckOrigin:     func(r *http.Request) bool { return true },
}

func defaultPage(w http.ResponseWriter, r *http.Request) {
    w.Header().Set("Content-Type", "text/plain")
    w.Write([]byte("Mighty backend welcomes you, player!\n"))
}

func reader(conn *websocket.Conn) {
  for {
    // read in a message
    messageType, p, err := conn.ReadMessage()
    if err != nil {
      log.Println(err)
      return
    }
    // print out that message for clarity
    log.Println(string(p))
    if err := conn.WriteMessage(messageType, p); err != nil {
      log.Println(err)
      return
    }
  }
}

func wsPage(w http.ResponseWriter, r *http.Request) {
  // upgrade this connection to a WebSocket
  // connection
  ws, err := upgrader.Upgrade(w, r, nil)
  if err != nil {
    log.Println(err)
  }
  log.Println("Client Connected")
  hello_msg := pong.PongData {
    Type: pong.DataType_Hello,
    Data: &pong.PongData_Hello{
      Hello: &pong.CmdHello{
        Msg: "Hello mighty Client!",
      },
    },
  }
  //hello_msg := pong.PongData_CmdHello {
  //  Msg: "Hello mighty Client!",
  //}
  out, err := proto.Marshal(&hello_msg)
  if err != nil {
    log.Println("Failed to serialize hello message ", err)
  }
  //err = ws.WriteMessage(1, []byte("Hi Client!"))
  err = ws.WriteMessage(1, out)
  if err != nil {
    log.Println(err)
  }
  // listen indefinitely for new messages coming
  // through on our WebSocket connection
  reader(ws)
}

func main() {
  log.Println("rengine backend started")
  http.HandleFunc("/", defaultPage)
  http.HandleFunc("/ws", wsPage)

  err := http.ListenAndServeTLS(":8443", "certificates/server.crt", "certificates/server.key", nil)
  //err := http.ListenAndServe(":8443", nil)
  if err != nil {
    log.Fatalln("Failed to start server: ", err)
  }
}
