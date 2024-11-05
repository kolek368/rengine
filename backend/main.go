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

func handleHello(_ *websocket.Conn, msg *pong.PongData) {
  log.Println("Hello msg:", msg.GetHello().Msg)
}

func handleIdReq(conn *websocket.Conn, _ *pong.PongData) {
  log.Println("Get ID message received")
  set_id_msg := pong.PongData {
    Type: pong.DataType_SetId,
    Data: &pong.PongData_IdRsp{
      IdRsp : &pong.CmdIdSet{
        Id: 0x1234FCB0,
      },
    },
  }
  log.Println("Sending ID response: {}", &set_id_msg)
  out, err := proto.Marshal(&set_id_msg)
  if err != nil {
    log.Println("Failed to serialize hello message ", err)
  }
  println("Sending: {:?}", out)
  err = conn.WriteMessage(1, out)
  if err != nil {
    log.Println("WriteMessage err: {}", err)
  }
  log.Println("ID response sent")
}

func reader(conn *websocket.Conn) {
  for {
    // read in a message
    _, p, err := conn.ReadMessage()
    if err != nil {
      log.Println(err)
      return
    }
    // print out that message for clarity
    pong_msg := pong.PongData{}
    if err:= proto.Unmarshal(p, &pong_msg); err != nil {
      log.Println("Failed to parse pong_msg")
      return
    }
    log.Println("Pong_msg type:", pong_msg.Type)
    switch pong_msg.Type {
    case pong.DataType_Hello:
      handleHello(conn, &pong_msg)
      break
    case pong.DataType_GetId:
      handleIdReq(conn, &pong_msg)
      break
    default:
      log.Println("Unsupported message received")
    }

    //if err := conn.WriteMessage(messageType, p); err != nil {
    //  log.Println(err)
    //  return
    //}
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
