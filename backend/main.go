package main

import (
  "log"
  "net/http"
  "github.com/gorilla/websocket"
  "rengine-backend/mighty/pong"
  "google.golang.org/protobuf/proto"
  "math"
  "math/rand"
  "sync"
)

type GameContext struct {
  game_id uint32
  player_left uint32
  player_right uint32
}

type GameContexts struct {
  mtx sync.Mutex
  ctx []GameContext
}

type ConnectionContext struct {
  player_id uint32 
  session_id uint32
}

var upgrader = websocket.Upgrader{
  ReadBufferSize:  1024,
  WriteBufferSize: 1024,
  CheckOrigin:     func(r *http.Request) bool { return true },
}

var game_contexts = GameContexts {
  ctx: []GameContext {
    {
      game_id: math.MaxUint32,
      player_left: math.MaxUint32,
      player_right: math.MaxUint32,
    },
  },
}

var game_sessions = make(map[uint32]*GameContext)
var connected_players = make(map[*websocket.Conn]ConnectionContext)

func getGameCtx() *GameContext {
  for i:=0; i < len(game_contexts.ctx); i++ {
    ctx := &game_contexts.ctx[i]
    if ctx.game_id == math.MaxUint32 {
      return ctx
    } else if ctx.player_right == math.MaxUint32 || ctx.player_left == math.MaxUint32 {
      return ctx
    }
  }
  return nil
}

func getGameCtxConcurent() *GameContext {
  game_contexts.mtx.Lock()
  retval := getGameCtx()
  game_contexts.mtx.Unlock()
  return retval
}

func getSessionIdAndPlayerId() (uint32, uint32) {
  var sessionId uint32 = math.MaxUint32
  var playerId uint32 = math.MaxUint32
  game_contexts.mtx.Lock()
  var gameCtx = getGameCtx()
  if gameCtx != nil {
    log.Println("Found context")
    if gameCtx.game_id == math.MaxUint32 {
      // New session, must creat session id and player id
      log.Println("Context is empty, generating new session id")
      var gameId = rand.Uint32()
      _, ok := game_sessions[gameId]
      for ok {
        gameId = rand.Uint32()
        _, ok := game_sessions[gameId]
        if !ok {
          break
        }
      }
      gameCtx.game_id = gameId
      sessionId = gameId
      game_sessions[sessionId] = gameCtx
    }

    if gameCtx.player_left == math.MaxUint32 {
      sessionId = gameCtx.game_id
      playerId = (gameCtx.game_id << 2) | 0x1
      gameCtx.player_left = playerId
    } else {
      log.Println("Context is initialized, generating next player id")
      sessionId = gameCtx.game_id
      playerId = (gameCtx.game_id << 2) | 0x2
      gameCtx.player_right = playerId
    }
  } else {
    log.Println("Could not find an empty session!")
  }
  game_contexts.mtx.Unlock()
  return sessionId, playerId
}

func removePlayerFromSession(player uint32, session uint32) {
  game_contexts.mtx.Lock()
  ctx, ok := game_sessions[session]
  if ok {
    if ctx.player_left == player {
      ctx.player_left = math.MaxUint32
    } else if ctx.player_right == player {
      ctx.player_right = math.MaxUint32
    } else {
      log.Println("Invalid player id")
    }
  }
  game_contexts.mtx.Unlock()
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
  var sessionId, playerId = getSessionIdAndPlayerId()
  set_id_msg := pong.PongData {
    Type: pong.DataType_SetId,
    Data: &pong.PongData_IdRsp{
      IdRsp : &pong.CmdIdSet{
        Id: uint32(playerId),
        Session: uint32(sessionId),
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
    log.Println("WriteMessage err:", err)
  }
  log.Println("ID response sent")
  connected_players[conn] = ConnectionContext{
    player_id: playerId,
    session_id: sessionId,
  }
}

func handleCtxReq(conn *websocket.Conn, msg *pong.PongData) {
  var sessionId = msg.GetCtxReq().GetSession()
  log.Println("Get CTX message received for session:", sessionId)
  ctx, ok := game_sessions[sessionId]
  if !ok {
    log.Println("Invalid session id requested")
    return
  }
  
  log.Println("Left_id: ", ctx.player_left, " Right_id: ", ctx.player_right)
  set_ctx_msg := pong.PongData {
    Type: pong.DataType_SetCtx,
    Data: &pong.PongData_CtxRsp{
      CtxRsp: &pong.CmdCtxSet {
        LeftId: ctx.player_left,
        RightId: ctx.player_right,
      },
    },
  }
  out, err := proto.Marshal(&set_ctx_msg)
  if err != nil {
    log.Println("Failed to serialize set_ctx message", err)
  }
  err = conn.WriteMessage(1, out)
  if err != nil {
    log.Println("WritMessage err:", err)
  }
  log.Println("CTX response sent")
}

func reader(conn *websocket.Conn) {
  for {
    // read in a message
    _, p, err := conn.ReadMessage()
    if err != nil {
      player_ctx := connected_players[conn]
      log.Println("ReadMessage error: ", err, " Session: ", player_ctx.session_id, " Player: ", player_ctx.player_id)
      removePlayerFromSession(player_ctx.player_id, player_ctx.session_id)
      delete(connected_players, conn)
      return
    }

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
    case pong.DataType_GetCtx:
      handleCtxReq(conn, &pong_msg)
      break
    default:
      log.Println("Unsupported message received")
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
  out, err := proto.Marshal(&hello_msg)
  if err != nil {
    log.Println("Failed to serialize hello message ", err)
  }
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
