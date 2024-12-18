use std::fmt::Debug;
use std::{collections::BTreeMap, ffi::CString};
use std::path::Path;

use protobuf::Message;
use raylib::{ffi::{GetRandomValue, LoadSound, PlaySound, Sound}, prelude::*};
use websocket::ws::dataframe::DataFrame;
use websocket::OwnedMessage;
use websocket::native_tls::TlsConnector;
use websocket::sync::Client;
use websocket::stream::sync::NetworkStream;

use std::thread::{self, sleep, JoinHandle};
use std::sync::mpsc::{channel, Sender, Receiver};


mod protos;
use protos::pong::PongData;

use crate::pong::protos::pong::DataType;

use self::protos::pong::{CmdCtxGet, CmdCtxSet, CmdHello, CmdIdGet, CmdReady};

const RES_WIDTH: i32 = 1280;
const RES_HEIGHT: i32 = 720;

const PADDLE_WIDTH: i32 = 40;
const PADDLE_HEIGHT: i32 = 200;
const PADDLE_SPEED: i32 = 8;
const PADDLE_OFFSET: i32 = 2;

const BALL_WIDTH: i32 = 40;
const BALL_HEIGHT: i32 = 40;
const BALL_SPEED: i32 = 10;

#[derive(Debug, Clone, Copy, PartialEq)]
enum GameState {
    Menu, // Main menu
    Connect, // Connect to game server
    Waiting, // Wait for other player
    Init, // Initialize state
    Loop, // Game main loop
    Scored, // Player scored
    Finished, // Game finished
    Quit, // Quit game
}

#[derive(Debug, Default, Eq, PartialOrd, Ord, PartialEq)]
enum MenuState {
    #[default]
    NewGame,
    Multiplayer,
    Options,
    Quit,
}

impl MenuState {
    fn next(&self) -> MenuState {
        match self {
            MenuState::NewGame => MenuState::Multiplayer,
            MenuState::Multiplayer => MenuState::Options,
            MenuState::Options => MenuState::Quit,
            MenuState::Quit => MenuState::NewGame,
        }
    }

    fn prev(&self) -> MenuState {
        match self {
            MenuState::NewGame => MenuState::Quit,
            MenuState::Multiplayer => MenuState::NewGame,
            MenuState::Options => MenuState::Multiplayer,
            MenuState::Quit => MenuState::Options,
        }
    }
}

#[derive(Debug, PartialEq)]
enum ScreenSide {
    Left,
    Right,
}

struct GameContext {
    score_left: i32,
    score_right: i32,
    state: GameState,
    state_menu: StateMenuContext,
    multiplayer: MultiplayerContext,
    assets: GameAssets,
}

struct GameAssets {
    menu_next: Sound,
    ball_bounce: Sound,
    player_scored: Sound,
}

#[derive(Default)]
struct StateMenuContext {
    current: MenuState,
}

#[derive(Default)]
struct MultiplayerContext {
    thread: Option<JoinHandle<()>>,
    id: u32,
    session: u32,
    side: Option<ScreenSide>,
    ctx: Option<CmdCtxSet>,
    game_rx: Option<Receiver<PongData>>,
    game_tx: Option<Sender<PongData>>,
}

#[derive(Debug)]
struct Paddle {
    pos_x: i32,
    pos_y: i32,
    width: i32,
    height: i32,
    color: Color,
    side: ScreenSide,
    key_up: KeyboardKey,
    key_down: KeyboardKey
}

impl Paddle {
    fn draw(&self, d: &mut RaylibDrawHandle) {
            if self.side == ScreenSide::Left {
                d.draw_rectangle(self.pos_x + PADDLE_OFFSET, self.pos_y - self.height/2, self.width, self.height, self.color);
            } else {
                d.draw_rectangle(self.pos_x - PADDLE_OFFSET - self.width, self.pos_y - self.height/2, self.width, self.height, self.color);
            }
    }

    fn rect(&self) -> Rectangle {
        if self.side == ScreenSide::Left {
            return Rectangle { x: (self.pos_x + PADDLE_OFFSET) as f32, y: (self.pos_y - self.height/2) as f32, width: self.width as f32, height: self.height as f32 };
        } else {
            return Rectangle { x: (self.pos_x - PADDLE_OFFSET - self.width) as f32, y: (self.pos_y - self.height/2) as f32, width: self.width as f32, height: self.height as f32 };
        }
    }

    fn is_local_player(&self, game: &mut GameContext) -> bool {
        if game.multiplayer.thread.is_none() {
            // for offline game both players are local
            return true;
        }
        
        if game.multiplayer.side.is_some() && *game.multiplayer.side.as_mut().unwrap() == self.side {
            return true;
        }
        false
    }

    fn update(&mut self, ctx: &RaylibHandle, _player: &Paddle, _ball: &Ball, game: &mut GameContext) -> GameState {
        if self.is_local_player(game) {  
            if ctx.is_key_down(self.key_down) && ctx.is_key_down(self.key_up) {
                return game.state;
            }

            if self.pos_y > self.height/2 && ctx.is_key_down(self.key_up) {
                self.pos_y = self.pos_y - PADDLE_SPEED;
            } else if self.pos_y < (RES_HEIGHT - self.height/2) && ctx.is_key_down(self.key_down) {
                self.pos_y = self.pos_y + PADDLE_SPEED;
            }
        } else {
            if game.multiplayer.ctx.is_none() {
                return game.state;
            }
            if self.side == ScreenSide::Left {
                let pos = game.multiplayer.ctx.as_mut().unwrap().left_pos;
                if pos > 0 {
                    self.pos_y = pos;
                }
            } else {
                let pos = game.multiplayer.ctx.as_mut().unwrap().right_pos;
                if pos > 0 {
                    self.pos_y = pos;
                }
            }
        }
        game.state
    }
}

struct Ball {
    pos_x: i32,
    pos_y: i32,
    width: i32,
    height: i32,
    color: Color,
    velocity_x: i32,
    velocity_y: i32,
}

impl Ball {
    fn draw(&self, d: &mut RaylibDrawHandle) {
        d.draw_rectangle(self.pos_x - self.width/2, self.pos_y - self.height/2, self.width, self.height, self.color);
    }

    fn rect(&self) -> Rectangle {
        Rectangle { x: (self.pos_x - self.width/2) as f32, y: (self.pos_y - self.height/2) as f32, width: self.width as f32, height: self.height as f32 }
    }

    fn update(&mut self, _ctx: &RaylibHandle, player_left: &Paddle, player_right: &Paddle, game: &mut GameContext) ->  GameState {
        let self_rect = self.rect();
        let velocity_y_sign = if self.velocity_y < 0 { -1 } else { 1 };
        self.pos_x = self.pos_x + self.velocity_x;
        self.pos_y = self.pos_y + self.velocity_y;

        if self_rect.check_collision_recs(&player_left.rect()) {
            self.pos_x = self.pos_x + player_left.width;
            self.pos_y = self.pos_y + self.velocity_y;
            unsafe {
                PlaySound(game.assets.ball_bounce);
                self.velocity_x = -(self.velocity_x + GetRandomValue(0, 5));
                self.velocity_y = velocity_y_sign * (BALL_SPEED + GetRandomValue(0, 3));
            }
            return game.state;
        } else if self_rect.check_collision_recs(&player_right.rect()) {
            self.pos_x = self.pos_x - self.width;
            self.pos_y = self.pos_y + self.velocity_y;
            unsafe {
                PlaySound(game.assets.ball_bounce);
                self.velocity_x = -(self.velocity_x + GetRandomValue(0, 5));
                self.velocity_y = velocity_y_sign * (BALL_SPEED + GetRandomValue(0, 3));
            }
            return game.state;
        }

        if self.pos_y <= self.height/2 || self.pos_y >= (RES_HEIGHT-self.height/2) {
            unsafe {
                PlaySound(game.assets.ball_bounce);
            }
            self.velocity_y = -self.velocity_y;
            self.pos_y = if self.pos_y < self.height/2 { self.height/2 + 1 } else { self.pos_y };
            self.pos_y = if self.pos_y > RES_HEIGHT - self.height/2 { RES_HEIGHT - self.height/2 - 1 } else { self.pos_y };
        }

        if self.pos_x < self.width/2 {
            game.score_right = game.score_right + 1;
            self.velocity_x = -self.velocity_x;
            //game.state = GameState::Scored;
        } else if self.pos_x > (RES_WIDTH-self.width/2) {
            game.score_left = game.score_left + 1;
            self.velocity_x = -self.velocity_x;
            //game.state = GameState::Scored;
        }

        if game.state == GameState::Scored {
            unsafe {
                PlaySound(game.assets.player_scored);
            }
        }

        game.state
    }
}

fn get_winning_score() -> i32 {
    10
}

fn get_winner(game: &GameContext) -> &str {
    if game.score_left == get_winning_score() {
        return "One";
    } else {
        return "Two";
    }
}

fn init_state(player_one: &mut Paddle, player_two: &mut Paddle, ball: &mut Ball, game: &mut GameContext, _rl: &mut RaylibHandle, _thread: &RaylibThread) {
    ball.pos_x = RES_WIDTH/2;
    ball.pos_y = RES_HEIGHT/2;
    player_one.pos_y = RES_HEIGHT/2;
    player_two.pos_y = RES_HEIGHT/2;

    game.score_left = 0;
    game.score_right = 0;
    game.state = GameState::Loop;
}

fn finished_state(_player_one: &mut Paddle, _player_two: &mut Paddle, _ball: &mut Ball, game: &mut GameContext, rl: &mut RaylibHandle, thread: &RaylibThread) {
    if !(game.score_left >= get_winning_score() || game.score_right >= get_winning_score()) {
        game.state = GameState::Loop;
    }

    if rl.is_key_pressed(KeyboardKey::KEY_N) {
        game.state = GameState::Quit;
        return;
    }

    if rl.is_key_pressed(KeyboardKey::KEY_Y) {
        game.state = GameState::Init;
        return;
    }

    let mut d = rl.begin_drawing(&thread);
    let y_offset = 80;
    let finished_message = format!("Game finished, Player {} won.", get_winner(game));
    let continue_message = "Do you want to play again?";
    let yes_no_message = "Y / N";
    d.clear_background(Color::BLACK);
    d.draw_text(&finished_message, RES_WIDTH/2 - d.measure_text(&finished_message, 40)/2, y_offset, 40, Color::RED);
    d.draw_text(&continue_message, RES_WIDTH/2 - d.measure_text(&continue_message, 40)/2, y_offset + 80, 40, Color::RED);
    d.draw_text(&yes_no_message, RES_WIDTH/2 - d.measure_text(&yes_no_message, 60)/2, y_offset + 160, 60, Color::RED);
}

fn menu_state(_player_one: &mut Paddle, _player_two: &mut Paddle, _ball: &mut Ball, game: &mut GameContext, rl: &mut RaylibHandle, thread: &RaylibThread) {
    let menu_messages: BTreeMap<MenuState, &str> = BTreeMap::from([
        (MenuState::NewGame, "New Game"), 
        (MenuState::Multiplayer, "Multiplayer"), 
        (MenuState::Options, "Options"), 
        (MenuState::Quit, "Quit"),
    ]);

    if rl.is_key_pressed(KeyboardKey::KEY_DOWN) {
        game.state_menu.current = game.state_menu.current.next();
        unsafe {
            PlaySound(game.assets.menu_next);
        }
    } else if rl.is_key_pressed(KeyboardKey::KEY_UP) {
        game.state_menu.current = game.state_menu.current.prev();
        unsafe {
            PlaySound(game.assets.menu_next);
        }
    } else if rl.is_key_pressed(KeyboardKey::KEY_ENTER) {
        if game.state_menu.current == MenuState::Quit {
            game.state = GameState::Quit;
            return;
        } else if game.state_menu.current == MenuState::NewGame {
            game.state = GameState::Loop;
            return;
        } else if game.state_menu.current == MenuState::Multiplayer {
            game.state = GameState::Connect;
            return;
        }
    }

    let mut d = rl.begin_drawing(&thread);
    let mut y_offset = 80;
    d.clear_background(Color::BLACK);
    for menu_message in menu_messages {
        let menu_message_width = d.measure_text(menu_message.1, 40);
        if menu_message.0 == game.state_menu.current {
            d.draw_text(menu_message.1, RES_WIDTH/2 - menu_message_width/2, y_offset, 40, Color::RED);
        } else {
            d.draw_text(menu_message.1, RES_WIDTH/2 - menu_message_width/2, y_offset, 40, Color::WHITE);
        }
        y_offset = y_offset + 80;
    }
}

fn can_game_continue(_player_one: &mut Paddle, _player_two: &mut Paddle, _ball: &mut Ball, game: &mut GameContext, rl: &mut RaylibHandle, _thread: &RaylibThread) -> bool {
    if game.multiplayer.thread.is_none() {
        return rl.is_key_down(KeyboardKey::KEY_SPACE);
    }
    false
}


fn scored_state(player_one: &mut Paddle, player_two: &mut Paddle, ball: &mut Ball, game: &mut GameContext, rl: &mut RaylibHandle, thread: &RaylibThread) {
    if game.score_right >= get_winning_score() || game.score_left >= get_winning_score() {
        game.state = GameState::Finished;
        return;
    }

    ball.pos_x = RES_WIDTH/2;
    ball.pos_y = RES_HEIGHT/2;
    if ball.velocity_x > 0 {
        ball.velocity_x = BALL_SPEED;
    } else {
        ball.velocity_x = -BALL_SPEED;
    }
    unsafe {
        ball.velocity_y = GetRandomValue(-3 - BALL_SPEED, BALL_SPEED + 3);
    }
    player_one.pos_y = RES_HEIGHT/2;
    player_two.pos_y = RES_HEIGHT/2;
    let can_continue: bool = can_game_continue(player_one, player_two, ball, game, rl, thread);
    let continue_message = "Press SPACE to continue.";
    let mut d = rl.begin_drawing(&thread);
    d.clear_background(Color::BLACK);
    let message_width = d.measure_text(continue_message, 40);
    d.draw_text(continue_message, RES_WIDTH/2 - message_width/2, RES_HEIGHT/2 - 20, 40, Color::WHITE);
    d.draw_fps(RES_WIDTH-25, 0);
    if can_continue {
        game.state = GameState::Loop;
    }
}

fn srv_multiplayer_update_out(player_one: &mut Paddle, player_two: &mut Paddle, ball: &Ball, game: &mut GameContext) {
    if game.multiplayer.thread.is_none() {
        return;
    }

    if game.multiplayer.side.is_none() {
        return;
    }
    let mut player_left_pos: i32 = -1;
    let mut player_right_pos: i32 = -1;
    if *game.multiplayer.side.as_mut().unwrap() == ScreenSide::Left {
        player_left_pos = player_one.pos_y;
    } else {
        player_right_pos = player_two.pos_y;
    }
    let mut cmd_set_ctx: CmdCtxSet = CmdCtxSet::default();
    cmd_set_ctx.session = game.multiplayer.session;
    cmd_set_ctx.left_pos = player_left_pos;
    cmd_set_ctx.right_pos = player_right_pos;
    let ctx = game.multiplayer.ctx.as_mut().unwrap();
    if game.multiplayer.id == ctx.ball_master {
        println!("Current master. Updating Ball: vx:{} vy:{} px:{} py:{}", ball.velocity_x, ball.velocity_y, ball.pos_x, ball.pos_y);
        cmd_set_ctx.ball_vx = ball.velocity_x;
        cmd_set_ctx.ball_vy = ball.velocity_y;
        cmd_set_ctx.ball_posx = ball.pos_x;
        cmd_set_ctx.ball_posy = ball.pos_y;
    } else {
        cmd_set_ctx.ball_vx = std::i32::MAX;
        cmd_set_ctx.ball_vy = std::i32::MAX;
    }
    let pong_msg = proto_ctx_resp_msg(cmd_set_ctx);
    game.multiplayer.game_tx.as_mut().unwrap().send(pong_msg).unwrap();
}

fn srv_multiplayer_update_ready(game: &mut GameContext) {
    if game.multiplayer.thread.is_none() {
        return;
    }

    println!("Sending Ready signal to SRV_THREAD");
    let mut cmd_ready: CmdReady = CmdReady::default();
    cmd_ready.session = game.multiplayer.session;
    cmd_ready.player = game.multiplayer.id;
    let pong_msg = proto_ready_msg(cmd_ready);
    game.multiplayer.game_tx.as_mut().unwrap().send(pong_msg).unwrap();
}

fn loop_state(player_one: &mut Paddle, player_two: &mut Paddle, ball: &mut Ball, game: &mut GameContext, rl: &mut RaylibHandle, thread: &RaylibThread) {
    player_one.update(&rl, &player_two, &ball, game);
    player_two.update(&rl, &player_one, &ball, game);
    ball.update(&rl, &player_one, &player_two, game);
    multiplayer_update(player_one, player_two, ball, game);
    let score_left = format!("{}", game.score_left);
    let score_right = format!("{}", game.score_right);
    let score_right_len = rl.measure_text(&score_right, 40);

    let mut d = rl.begin_drawing(&thread);

    d.clear_background(Color::BLACK);
    d.draw_text(&score_left, PADDLE_WIDTH + 10, 10, 40, Color::WHITE);
    d.draw_text(&score_right, RES_WIDTH - 10 - PADDLE_WIDTH - score_right_len, 10, 40, Color::WHITE);
    d.draw_fps(RES_WIDTH-25, 0);

    player_one.draw(&mut d);
    player_two.draw(&mut d);
    ball.draw(&mut d);
}

#[allow(dead_code)]
fn proto_hello_msg(msg: &str) -> OwnedMessage {
    // hello protobuf message
    let mut msg_hello: PongData = PongData::new();
    let cmd_hello: CmdHello = CmdHello{
        msg: msg.to_string(),
        special_fields: ::protobuf::SpecialFields::default(),
    };
    msg_hello.set_hello(cmd_hello);
    let ret_msg = OwnedMessage::Binary(msg_hello.write_to_bytes().unwrap());
    ret_msg
}

fn proto_id_req_msg() -> OwnedMessage {
    let mut msg_get_id: PongData = PongData::new();
    let cmd_get_id: CmdIdGet = CmdIdGet::default();
    msg_get_id.type_ = DataType::GetId.into();
    msg_get_id.set_id_req(cmd_get_id);
    let msg = OwnedMessage::Binary(msg_get_id.write_to_bytes().unwrap());
    msg
}

fn proto_ctx_req_msg(cmd: CmdCtxGet) -> OwnedMessage {
    let mut msg_get_ctx: PongData = PongData::new();
    msg_get_ctx.type_ = DataType::GetCtx.into();
    msg_get_ctx.set_ctx_req(cmd);
    let msg = OwnedMessage::Binary(msg_get_ctx.write_to_bytes().unwrap());
    msg
}

fn proto_ctx_resp_msg(ctx: CmdCtxSet) -> PongData {
    let mut msg_set_ctx: PongData = PongData::new();
    msg_set_ctx.type_ = DataType::SetCtx.into();
    msg_set_ctx.set_ctx_rsp(ctx);
    msg_set_ctx
}

fn proto_ready_msg(ctx: CmdReady) -> PongData {
    let mut msg_ready: PongData = PongData::new();
    msg_ready.type_ = DataType::Ready.into();
    msg_ready.set_ready(ctx);
    msg_ready
}

fn srv_connect() -> websocket::sync::Client<Box<dyn NetworkStream + std::marker::Send>> {
    println!("Creating new socket");
    let tls_connector = TlsConnector::builder().danger_accept_invalid_certs(true).build().unwrap();
    let mut ws = websocket::ClientBuilder::new("wss://127.0.0.1:8443/ws").unwrap().connect(Some(tls_connector)).unwrap();
    let read_ret = ws.recv_dataframe();
    println!("Dataframe received");
    if read_ret.is_ok() {
        let srv_resp = PongData::parse_from_bytes(&read_ret.unwrap().take_payload()).unwrap();
        println!("Srv_resp: {}", srv_resp.to_string());
        if DataType::Hello == srv_resp.type_.unwrap() {
            println!("Server hello msg: {:?} - {}", DataType::Hello, srv_resp.hello().msg);
            } else {
                println!("Did not receive hello! {:?}", srv_resp.type_.unwrap());
            }
    }
    ws
}

fn srv_get_id(ws: &mut Client<Box<dyn NetworkStream + Send>>) -> Result<PongData, String> {
    let msg = proto_id_req_msg();
    let ret = ws.send_message(&msg);
    if ret.is_err() {
        println!("Return: ERR::{}", ret.err().unwrap());
    } else {
        println!("Return: OK ::{:?}", ret.unwrap());
    }
     
    let read_ret = ws.recv_dataframe();
    if read_ret.is_err() {
        println!("Error: {}", read_ret.err().unwrap());
        return Err("Did not receive server response".to_string());
    }
    let srv_resp = PongData::parse_from_bytes(&read_ret.unwrap().take_payload()).unwrap();
    if srv_resp.type_.unwrap() != DataType::SetId {
        return Err("Did not receive id response from server".to_string());
    }
    println!("Received player id: {} session id: {} from server", srv_resp.id_rsp().id, srv_resp.id_rsp().session);
    Ok(srv_resp)
}

fn srv_get_ctx(ws: &mut Client<Box<dyn NetworkStream + Send>>, session: u32) -> Result<PongData, String> {
    let mut cmd_get_ctx: CmdCtxGet = CmdCtxGet::default();
    cmd_get_ctx.session = session;
    let msg = proto_ctx_req_msg(cmd_get_ctx);
    let ret = ws.send_message(&msg);
    if ret.is_err() {
        return Err("Failed to send ctx request.".to_string())
    }

    let read_ret = ws.recv_dataframe();
    if read_ret.is_err() {
        return Err("Did not receive ctx response.".to_string())
    }

    let srv_resp = PongData::parse_from_bytes(&read_ret.unwrap().take_payload()).unwrap();
    if srv_resp.type_.unwrap() != DataType::SetCtx {
        return Err("Invalid response received".to_string());
    }
    Ok(srv_resp)
}

fn srv_send_data(ws: &mut Client<Box<dyn NetworkStream + Send>>, ctx: PongData) {
    let msg = OwnedMessage::Binary(ctx.write_to_bytes().unwrap());
    let ret = ws.send_message(&msg);
    if ret.is_err() {
        println!("Failed to send updated local context {:?}", ret.err());
    }
}

fn srv_thread(tx: Sender<PongData>, rx: Receiver<PongData>) {
    let mut session: u32 = std::u32::MAX;
    let mut ws = srv_connect();
    let get_it_resp = srv_get_id(&mut ws);
    if get_it_resp.is_err() {
        println!("Error: {}", get_it_resp.err().unwrap());
    } else {
        let multiplayer_data = get_it_resp.unwrap();
        println!("Multiplayer data: {:?}", multiplayer_data.id_rsp());
        session = multiplayer_data.id_rsp().session;
        println!("Sending data to game loop session: {}", session);
        tx.send(multiplayer_data).unwrap();
    }
    loop {
        let loop_rx = rx.try_recv();
        if loop_rx.is_ok() {
            let pong_msg = loop_rx.unwrap();
            if pong_msg.type_ == DataType::SetCtx.into() {
                println!("Srv CTX: {:?}", pong_msg);
                srv_send_data(&mut ws, pong_msg);
            } else if pong_msg.type_ == DataType::Ready.into() {
                println!("Srv READY: {:?}", pong_msg);
                srv_send_data(&mut ws, pong_msg);
            }
        } else {
            sleep(std::time::Duration::from_millis(20));
        }

        let srv_ctx = srv_get_ctx(&mut ws, session);
        if srv_ctx.is_ok() {
            tx.send(srv_ctx.unwrap()).unwrap();
        }
        // sleep(std::time::Duration::from_secs(2));
    }
}

fn srv_thread_start(game: &mut GameContext) {
    let (thread_tx, game_rx) = channel::<PongData>();
    let (game_tx, thread_rx) = channel::<PongData>();
    game.multiplayer.game_tx = Some(game_tx);
    game.multiplayer.game_rx = Some(game_rx);
    game.multiplayer.thread = Some(thread::spawn(|| srv_thread(thread_tx, thread_rx)));
}

fn multiplayer_is_connected(game: &GameContext) -> bool {
    if game.multiplayer.id != std::u32::MAX && game.multiplayer.session != std::u32::MAX {
        return true;
    }
    false
}

fn multiplayer_update(player_one: &mut Paddle, player_two: &mut Paddle, ball: &mut Ball, game: &mut GameContext) {
    if game.multiplayer.thread.is_none() {
        println!("Multiplaer thread not started");
        return;
    }

    let rx_data = game.multiplayer.game_rx.as_mut().unwrap().try_recv();
    if rx_data.is_err() {
        return;
    }
    let mut rx_data = rx_data.unwrap();
    match rx_data.type_.unwrap() {
        DataType::SetId => {
            game.multiplayer.id = rx_data.id_rsp().id;
            game.multiplayer.session = rx_data.id_rsp().session;
            println!("Loop session id: {} player id: {}", game.multiplayer.session, game.multiplayer.id);
        },
        DataType::SetCtx => {
            //println!("Loop ctx: {}", rx_data.ctx_rsp());
            if rx_data.ctx_rsp().ball_master != game.multiplayer.id {
                if rx_data.ctx_rsp().ball_vy != std::i32::MAX && rx_data.ctx_rsp().ball_vx != std::i32::MAX {
                    ball.velocity_x = rx_data.ctx_rsp().ball_vx;
                    ball.velocity_y = rx_data.ctx_rsp().ball_vy;
                }
                if rx_data.ctx_rsp().ball_master != std::u32::MAX &&  rx_data.ctx_rsp().ball_posx != std::i32::MAX && rx_data.ctx_rsp().ball_posx != std::i32::MAX {
                        ball.pos_x = rx_data.ctx_rsp().ball_posx;
                        ball.pos_y = rx_data.ctx_rsp().ball_posy;
                }
            }
            game.multiplayer.ctx = Some(rx_data.take_ctx_rsp());
            //println!("Ball vx: {} vy: {}", ball.velocity_x, ball.velocity_y);
        }
        _ => println!("Received invalid data type from thread: {:?}", rx_data.type_),
    }
    srv_multiplayer_update_out(player_one, player_two, ball, game);
}

fn connect_state(player_one: &mut Paddle, player_two: &mut Paddle, ball: &mut Ball, game: &mut GameContext, rl: &mut RaylibHandle, thread: &RaylibThread) {
    multiplayer_update(player_one, player_two, ball, game);
    if game.multiplayer.thread.is_none() {
        game.multiplayer.id = std::u32::MAX;
        game.multiplayer.session = std::u32::MAX;
        println!("SRV thread starting");
        srv_thread_start(game);
        println!("SRV thread started");
    } else {
        println!("Socket already exists");
        if multiplayer_is_connected(game) {
            game.state = GameState::Waiting;
        }
    }

    let connecting_msg = "Connecting ...";
    let connecting_msg_len = rl.measure_text(&connecting_msg, 40);
    let mut d = rl.begin_drawing(&thread);

    d.clear_background(Color::BLACK);
    d.draw_text(&connecting_msg, (RES_WIDTH - connecting_msg_len)/2 , 10, 40, Color::WHITE);
}

fn waiting_state(player_one: &mut Paddle, player_two: &mut Paddle, ball: &mut Ball, game: &mut GameContext, rl: &mut RaylibHandle, thread: &RaylibThread) {
    multiplayer_update(player_one, player_two, ball, game);
    const WAITING_MESSAGES: &[&str] = &["Waiting .  ", "Waiting  . ", "Waiting   ."];
    static mut WAITING_COUNTER: usize = 0;
    let waiting_msg: &str;
    let mut send_request: bool = false;
    // Shame on me for wanting to have an animated waiting screen and using unsafe. Like an animal.
    unsafe  {
        waiting_msg = WAITING_MESSAGES[WAITING_COUNTER/60];
        WAITING_COUNTER = (WAITING_COUNTER + 1) % (WAITING_MESSAGES.len() * 60);
        if WAITING_COUNTER%60 == 0 {
            send_request = true;
        }
    }
    let waiting_msg_len = rl.measure_text(&waiting_msg, 40);
    
    if send_request {
        if game.multiplayer.ctx.is_some() {
            // If both players have IDs assigned not equal 0xFFFFFFFF it means that we can proceed to
            // the next state. Also check which side we are assigned.
            let ctx = game.multiplayer.ctx.as_ref().unwrap();
            println!("Left_id: {} Right_id: {}", ctx.left_id, ctx.right_id);
            if ctx.left_id == game.multiplayer.id {
                game.multiplayer.side = Some(ScreenSide::Left);
            } else if ctx.right_id == game.multiplayer.id {
                game.multiplayer.side = Some(ScreenSide::Right);
            } else {
                // Neither player has assigned our Id - must be communicaiton error.
                println!("Invalid id assigned, could not determine side.")
            }
            if ctx.left_id != std::u32::MAX && ctx.right_id != std::u32::MAX {
                println!("Second player connected, can start the game.");
                srv_multiplayer_update_ready(game);
                game.state = GameState::Loop;
            }
        }
    }

    let mut d = rl.begin_drawing(&thread);

    d.clear_background(Color::BLACK);
    d.draw_text(&waiting_msg, (RES_WIDTH - waiting_msg_len)/2 , 10, 40, Color::WHITE);
}

pub fn pong() {
    let (mut rl, thread) = raylib::init()
        .size(RES_WIDTH, RES_HEIGHT)
        .title("Safe Pong in RUST")
        .build();

    let rl_audio = raylib::audio::RaylibAudio::init_audio_device();
    if rl_audio.is_err() {
        println!("Failed to initialize audio device!");
    }

    let mut player_left: Paddle = Paddle {
        pos_x: 0,
        pos_y: RES_HEIGHT/2,
        width: PADDLE_WIDTH,
        height: PADDLE_HEIGHT,
        color: Color::WHITE,
        side: ScreenSide::Left,
        key_up: KeyboardKey::KEY_Q,
        key_down: KeyboardKey::KEY_A,
    };
    let mut player_right: Paddle = Paddle {
        pos_x: RES_WIDTH,
        pos_y: RES_HEIGHT/2,
        width: PADDLE_WIDTH,
        height: PADDLE_HEIGHT,
        color: Color::WHITE,
        side: ScreenSide::Right,
        key_up: KeyboardKey::KEY_P,
        key_down: KeyboardKey::KEY_L,
    };
    let mut ball: Ball = Ball {
        pos_x: RES_WIDTH/2,
        pos_y: RES_HEIGHT/2,
        width: BALL_WIDTH,
        height: BALL_HEIGHT,
        color: Color::WHITE,
        velocity_x: BALL_SPEED,
        velocity_y: BALL_SPEED,
    };

    println!("Assets path exists: {}", Path::new("assets/ball_bounce.wav").exists());
    let mut game: GameContext;
    unsafe {
        let menu_next_path = CString::new("assets/menu_next.wav").unwrap();
        let ball_bounce_path = CString::new("assets/ball_bounce.wav").unwrap();
        let player_scored_path = CString::new("assets/player_scored.wav").unwrap();
        game = GameContext {
            score_left: 0,
            score_right: 0,
            state: GameState::Menu,
            state_menu: Default::default(),
            multiplayer: Default::default(),
            assets: GameAssets {
                menu_next: LoadSound(menu_next_path.as_ptr()),
                ball_bounce: LoadSound(ball_bounce_path.as_ptr()),
                player_scored: LoadSound(player_scored_path.as_ptr()),
            }
        };
    }

    rl.set_target_fps(60);

    while !rl.window_should_close() && game.state != GameState::Quit {
        match game.state {
            GameState::Connect => connect_state(&mut player_left, &mut player_right, &mut ball, &mut game, &mut rl, &thread),
            GameState::Waiting => waiting_state(&mut player_left, &mut player_right, &mut ball, &mut game, &mut rl, &thread),
            GameState::Init => init_state(&mut player_left, &mut player_right, &mut ball, &mut game, &mut rl, &thread),
            GameState::Loop => loop_state(&mut player_left, &mut player_right, &mut ball, &mut game, &mut rl, &thread),
            GameState::Scored => scored_state(&mut player_left, &mut player_right, &mut ball, &mut game, &mut rl, &thread),
            GameState::Menu => menu_state(&mut player_left, &mut player_right, &mut ball, &mut game, &mut rl, &thread),
            GameState::Finished => finished_state(&mut player_left, &mut player_right, &mut ball, &mut game, &mut rl, &thread),
            _ => game.state = GameState::Quit,
        }
    }
}
