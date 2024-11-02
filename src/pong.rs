use std::fmt::Debug;
use std::{collections::BTreeMap, ffi::CString};
use std::path::Path;

use raylib::{ffi::{GetRandomValue, LoadSound, PlaySound, Sound}, prelude::*};
use websocket::OwnedMessage;
use websocket::native_tls::TlsConnector;

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

    fn update(&mut self, ctx: &RaylibHandle, _player: &Paddle, _ball: &Ball, game: &mut GameContext) -> GameState {
        if ctx.is_key_down(self.key_down) && ctx.is_key_down(self.key_up) {
            return game.state;
        }

        if self.pos_y > self.height/2 && ctx.is_key_down(self.key_up) {
            self.pos_y = self.pos_y - PADDLE_SPEED;
        } else if self.pos_y < (RES_HEIGHT - self.height/2) && ctx.is_key_down(self.key_down) {
            self.pos_y = self.pos_y + PADDLE_SPEED;
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

        self.pos_x = self.pos_x + self.velocity_x;
        self.pos_y = self.pos_y + self.velocity_y;

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
            game.state = GameState::Scored;
        } else if self.pos_x > (RES_WIDTH-self.width/2) {
            game.score_left = game.score_left + 1;
            self.velocity_x = -self.velocity_x;
            game.state = GameState::Scored;
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
    let can_continue: bool = rl.is_key_down(KeyboardKey::KEY_SPACE);
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

fn loop_state(player_one: &mut Paddle, player_two: &mut Paddle, ball: &mut Ball, game: &mut GameContext, rl: &mut RaylibHandle, thread: &RaylibThread) {
        player_one.update(&rl, &player_two, &ball, game);
        player_two.update(&rl, &player_one, &ball, game);
        ball.update(&rl, &player_one, &player_two, game);
        let score_left = format!("{}", game.score_left);
        let score_right = format!("{}", game.score_right);
        let score_right_len = rl.measure_text(&score_right, 40);

        let mut d = rl.begin_drawing(&thread);

        d.clear_background(Color::BLACK);
        d.draw_text(&score_left, 10, 10, 40, Color::WHITE);
        d.draw_text(&score_right, RES_WIDTH - 10 - score_right_len, 10, 40, Color::WHITE);
        d.draw_fps(RES_WIDTH-25, 0);

        player_one.draw(&mut d);
        player_two.draw(&mut d);
        ball.draw(&mut d);
}

fn connect_state(_player_one: &mut Paddle, _player_two: &mut Paddle, _ball: &mut Ball, _game: &mut GameContext, rl: &mut RaylibHandle, thread: &RaylibThread) {
    let tls_connector = TlsConnector::builder().danger_accept_invalid_certs(true).build().unwrap();
    let mut ws = websocket::ClientBuilder::new("wss://127.0.0.1:8443/ws").unwrap().connect(Some(tls_connector)).unwrap();
    let read_ret = ws.recv_message();
    if read_ret.is_ok() {
        println!("Server resp: {:?}", read_ret.unwrap());
    }
    let msg = OwnedMessage::Text("Hello from mighty rust client!".to_string());
    let ret = ws.send_message(&msg);
    if ret.is_err() {
        println!("Return: ERR::{}", ret.err().unwrap());
    } else {
        println!("Return: OK ::{:?}", ret.unwrap());
    }
    let _ = ws.shutdown();

    let connecting_msg = "Connecting ...";
    let connecting_msg_len = rl.measure_text(&connecting_msg, 40);
    let mut d = rl.begin_drawing(&thread);

    d.clear_background(Color::BLACK);
    d.draw_text(&connecting_msg, (RES_WIDTH - connecting_msg_len)/2 , 10, 40, Color::WHITE);
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
            GameState::Init => init_state(&mut player_left, &mut player_right, &mut ball, &mut game, &mut rl, &thread),
            GameState::Loop => loop_state(&mut player_left, &mut player_right, &mut ball, &mut game, &mut rl, &thread),
            GameState::Scored => scored_state(&mut player_left, &mut player_right, &mut ball, &mut game, &mut rl, &thread),
            GameState::Menu => menu_state(&mut player_left, &mut player_right, &mut ball, &mut game, &mut rl, &thread),
            GameState::Finished => finished_state(&mut player_left, &mut player_right, &mut ball, &mut game, &mut rl, &thread),
            _ => game.state = GameState::Quit,
        }
    }
}
