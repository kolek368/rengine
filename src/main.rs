use std::{collections::BTreeMap, thread::sleep, time::Duration};

use raylib::prelude::*;

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
    _Options, // Option menu
    _NewGame, // Start new game
    Loop, // Game main loop
    Scored, // Player scored
    _Finished, // Game finished
    Quit, // Quit game
}

#[derive(Debug, Default, PartialEq)]
enum MenuState {
    #[default]
    NewGame,
    Options,
    Quit,
}

impl MenuState {
    fn next(&self) -> MenuState {
        match self {
            MenuState::NewGame => MenuState::Options,
            MenuState::Options => MenuState::Quit,
            MenuState::Quit => MenuState::NewGame,
        }
    }

    fn prev(&self) -> MenuState {
        match self {
            MenuState::NewGame => MenuState::Quit,
            MenuState::Options => MenuState::NewGame,
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
        if self_rect.check_collision_recs(&player_left.rect()) {
            self.velocity_x = -self.velocity_x;
            self.pos_x = self.pos_x + player_left.width;
            return game.state;
        } else if self_rect.check_collision_recs(&player_right.rect()) {
            self.velocity_x = -self.velocity_x;
            self.pos_x = self.pos_x - player_left.width;
            return game.state;
        }

        self.pos_x = self.pos_x + self.velocity_x;
        self.pos_y = self.pos_y + self.velocity_y;

        if self.pos_y < self.height/2 || self.pos_y > (RES_HEIGHT-self.height/2) {
            self.velocity_y = -self.velocity_y;
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
        game.state
    }
}

fn main() {
    let (mut rl, thread) = raylib::init()
        .size(RES_WIDTH, RES_HEIGHT)
        .title("Safe Pong in RUST")
        .build();

    rl.set_target_fps(60);

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

    let mut game: GameContext = GameContext {
        score_left: 0,
        score_right: 0,
        state: GameState::Menu,
        state_menu: Default::default()
    };

    while !rl.window_should_close() && game.state != GameState::Quit {
        match game.state {
            GameState::Loop => loop_state(&mut player_left, &mut player_right, &mut ball, &mut game, &mut rl, &thread),
            GameState::Scored => scored_state(&mut player_left, &mut player_right, &mut ball, &mut game, &mut rl, &thread),
            GameState::Menu => menu_state(&mut player_left, &mut player_right, &mut ball, &mut game, &mut rl, &thread),
            _ => sleep(Duration::from_secs(1)),
        }
    }
}

fn menu_state(_player_one: &mut Paddle, _player_two: &mut Paddle, _ball: &mut Ball, game: &mut GameContext, rl: &mut RaylibHandle, thread: &RaylibThread) {
    let menu_messages: BTreeMap<&str, MenuState> = BTreeMap::from([
        ("New Game", MenuState::NewGame), 
        ("Options", MenuState::Options), 
        ("Quit", MenuState::Quit),
    ]);

    if rl.is_key_pressed(KeyboardKey::KEY_DOWN) {
        game.state_menu.current = game.state_menu.current.next();
    } else if rl.is_key_pressed(KeyboardKey::KEY_UP) {
        game.state_menu.current = game.state_menu.current.prev();
    } else if rl.is_key_pressed(KeyboardKey::KEY_ENTER) {
        if game.state_menu.current == MenuState::Quit {
            game.state = GameState::Quit;
            return;
        } else if game.state_menu.current == MenuState::NewGame {
            game.state = GameState::Loop;
            return;
        }
    }

    let mut d = rl.begin_drawing(&thread);
    let mut y_offset = 80;
    d.clear_background(Color::BLACK);
    for menu_message in menu_messages {
        let menu_message_width = d.measure_text(menu_message.0, 40);
        if menu_message.1 == game.state_menu.current {
            d.draw_text(menu_message.0, RES_WIDTH/2 - menu_message_width/2, y_offset, 40, Color::RED);
        } else {
            d.draw_text(menu_message.0, RES_WIDTH/2 - menu_message_width/2, y_offset, 40, Color::WHITE);
        }
        y_offset = y_offset + 80;
    }
}

fn scored_state(player_one: &mut Paddle, player_two: &mut Paddle, ball: &mut Ball, game: &mut GameContext, rl: &mut RaylibHandle, thread: &RaylibThread) {
    ball.pos_x = RES_WIDTH/2;
    ball.pos_y = RES_HEIGHT/2;
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
