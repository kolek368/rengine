use raylib::prelude::*;

const RES_WIDTH: i32 = 1280;
const RES_HEIGHT: i32 = 720;

const PADDLE_WIDTH: i32 = 40;
const PADDLE_HEIGHT: i32 = 200;
const PADDLE_SPEED: i32 = 5;

const BALL_WIDTH: i32 = 40;
const BALL_HEIGHT: i32 = 40;
const BALL_SPEED: i32 = 10;

#[derive(Debug, PartialEq)]
enum ScreenSide {
    Left,
    Right,
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
                d.draw_rectangle(self.pos_x + 20, self.pos_y - self.height/2, self.width, self.height, self.color);
            } else {
                d.draw_rectangle(self.pos_x - 20 - self.width, self.pos_y - self.height/2, self.width, self.height, self.color);
            }
    }

    fn update(&mut self, ctx: &RaylibHandle) {
        if ctx.is_key_down(self.key_down) && ctx.is_key_down(self.key_up) {
            return;
        }

        if self.pos_y > self.height/2 && ctx.is_key_down(self.key_up) {
            self.pos_y = self.pos_y - PADDLE_SPEED;
        } else if self.pos_y < (RES_HEIGHT - self.height/2) && ctx.is_key_down(self.key_down) {
            self.pos_y = self.pos_y + PADDLE_SPEED;
        }
    }

    fn rect(&self) -> Rectangle {
        Rectangle { x: self.pos_x as f32, y: self.pos_y as f32, width: self.width as f32, height: self.height as f32 }
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

    fn update(&mut self, _ctx: &RaylibHandle) {
        self.pos_x = self.pos_x + self.velocity_x;
        self.pos_y = self.pos_y + self.velocity_y;

        if self.pos_y < self.height/2 || self.pos_y > (RES_HEIGHT-self.height/2) {
            self.velocity_y = -self.velocity_y;
        }

        if self.pos_x < self.width/2 || self.pos_x > (RES_WIDTH-self.width/2) {
            self.velocity_x = -self.velocity_x;
        }
    }
}

fn main() {
    let (mut rl, thread) = raylib::init()
        .size(RES_WIDTH, RES_HEIGHT)
        .title("Safe Pong in RUST")
        .build();

    rl.set_target_fps(60);

    let mut player_1: Paddle = Paddle {
        pos_x: 0,
        pos_y: RES_HEIGHT/2,
        width: PADDLE_WIDTH,
        height: PADDLE_HEIGHT,
        color: Color::WHITE,
        side: ScreenSide::Left,
        key_up: KeyboardKey::KEY_Q,
        key_down: KeyboardKey::KEY_A,
    };
    let mut player_2: Paddle = Paddle {
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
    while !rl.window_should_close() {
        player_1.update(&rl);
        player_2.update(&rl);
        ball.update(&rl);
        let mut d = rl.begin_drawing(&thread);

        d.clear_background(Color::BLACK);
        d.draw_fps(RES_WIDTH-25, 0);

        player_1.draw(&mut d);
        player_2.draw(&mut d);
        ball.draw(&mut d);
    }
}
