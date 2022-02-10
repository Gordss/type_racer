use ggez::audio::SoundSource;
use ggez::conf::{ Conf, WindowMode };
use ggez::{ event, timer, filesystem };
use ggez::graphics;
use ggez::{ Context, ContextBuilder, GameResult };
use ggez::input::keyboard::is_key_pressed;
use ggez::mint::Point2;
use rand::{ Rng, seq };
use rand::rngs::ThreadRng;

use type_racer::assets::{ Assets, TextSprite };
use type_racer::entities::Word;
use type_racer::debug;

use std::str;
use std::env;
use std::path;
use std::io::Read;

fn main() {
    let conf = Conf::new()
    .window_mode(WindowMode {
        width: 1200.0,
        height: 1000.0,
        ..Default::default()
    });

    let (mut ctx, event_loop) = ContextBuilder::new("type_racer", "George Shavov")
        .default_conf(conf.clone())
        .build()
        .unwrap();

    graphics::set_window_title(&ctx, "Type Racer");

    if let Ok(manifest_dir) = env::var("CARGO_MANIFEST_DIR") {
        let mut path = path::PathBuf::from(manifest_dir);
        path.push("resources");
        filesystem::mount(&mut ctx, &path, true);
    }

    let state = MainState::new(&mut ctx, &conf).unwrap();

    event::run(ctx, event_loop, state);
}

struct MainState {
    rng: ThreadRng,
    assets: Assets,
    sound_volume: f32,
    show_info: bool,
    game_over: bool,
    current_input: String,
    cash: u32,
    typed_words: u32,
    remaining_lifes: u32,
    words: Vec<Word>,
    time_until_next_word: f32,
    game_speed_up: f32,
    screen_width: f32,
    screen_height: f32,
    words_pool: Vec<String>
}

impl MainState {
    const BUY_LIFE_TAX: u32 = 300;
    const REMOVE_WORDS_TAX: u32 = 350;
    const SLOW_WORD_SPAWN_TAX: u32 = 1000;
    const REMOVE_WORDS_COUNT: usize = 2;
    const INITAL_SOUND_VOLUME: f32 = 0.05;
    const SOUND_VOLUME_STEP: f32 = 0.005;

    fn new(ctx: &mut Context, conf: &Conf) -> GameResult<MainState> {
        let mut assets = Assets::new(ctx)?;
        assets.background_music.set_volume(MainState::INITAL_SOUND_VOLUME);
        let _ = assets.background_music.play(ctx);
        let file = filesystem::open(ctx, "/words.dict");
        
        if file.is_err() {
            panic!("Missing words dictionary!");
        }

        let mut buffer = Vec::new();
        let read_size = file?.read_to_end(&mut buffer);

        if read_size.is_err() || read_size? == 0 {
            panic!("Empty file with words dictionary!");
        }

        let words = str::from_utf8(&buffer).unwrap().split('\n').collect::<Vec<&str>>();
        let words = words.iter().map(|x| x.to_string());

        let start_state = MainState {
            rng: rand::thread_rng(),
            assets: assets,
            sound_volume: MainState::INITAL_SOUND_VOLUME,
            show_info: false,
            game_over: false,
            current_input: String::new(),
            cash: 0,
            typed_words: 0,
            remaining_lifes: 5,
            words: Vec::new(),
            time_until_next_word: 3.0,
            game_speed_up: 0.0,
            screen_width: conf.window_mode.width,
            screen_height: conf.window_mode.height,
            words_pool: words.collect()
        };

        Ok(start_state)
    }
}

impl event::EventHandler for MainState {
    fn update(&mut self, ctx: &mut Context) -> GameResult<()> {
        if self.game_over {
            return Ok(())
        }

        const FPS_CAP: u32 = 60;

        while timer::check_update_time(ctx, FPS_CAP)
        {
            let seconds = 1.0 / (FPS_CAP as f32);

            // Spawn  words
            self.time_until_next_word -= seconds;
            if self.time_until_next_word <= 0.0 {
                let random_point = Point2 {
                    x: 0.0,
                    //TODO: check if 100.0 is okey for word size
                    y: self.rng.gen_range(40.0 .. self.screen_height - 100.0)
                };
            
                let random_word = self.words_pool[self.rng.gen_range(0 .. self.words_pool.len())].clone();
                let random_speed = self.rng.gen_range(50.0 .. 200.0);
    
                let is_color_changing = self.rng.gen_range(0 ..= 100) < 30;
                let word_sprite = Box::new(TextSprite::new(&random_word, ctx)?);
                let word = Word::new(&random_word, random_point, random_speed, word_sprite, is_color_changing)?;
    
                self.words.push(word);
                let min_word_gen_time = 3.0 - self.game_speed_up;
                let max_word_gen_time = 3.5 - self.game_speed_up;
                self.time_until_next_word = self.rng.gen_range(min_word_gen_time .. max_word_gen_time);
                self.game_speed_up += 0.01;
            }

            for word in self.words.iter_mut() {
                word.update(seconds);
    
                if word.label() == self.current_input {
                    word.is_typed = true;
                    self.typed_words += 1;
                    self.assets.word_typed_sound.set_volume(self.sound_volume);
                    let _ = self.assets.word_typed_sound.play(ctx);

                    // color chaning words give more points
                    if word.is_color_changing {
                        self.cash += 20;
                    }
                    else {
                        self.cash += 10;
                    }
                    // clear the input field after successfully typing word
                    self.current_input = String::new();
                }

                if word.pos.x >= self.screen_width {
                    word.is_typed = true;

                    if !debug::is_active() {
                        // don't end the game is debug is active
                        self.remaining_lifes -= 1;

                        if self.remaining_lifes == 0 {
                            self.game_over = true;
                        }
                    }
                }
            }

            self.words.retain(|word| !word.is_typed);
        }

        Ok(())
    }

    fn key_down_event(&mut self, ctx: &mut Context, keycode: event::KeyCode, _keymods: event::KeyMods, _repeat: bool) {
        
        match keycode {
            event::KeyCode::Escape => event::quit(ctx),
            event::KeyCode::Numpad1 => {
                if self.cash >= MainState::BUY_LIFE_TAX {
                    self.cash -= MainState::BUY_LIFE_TAX;
                    self.remaining_lifes += 1;
                }
            },
            event::KeyCode::Key1 => {
                if self.cash >= MainState::BUY_LIFE_TAX {
                    self.cash -= MainState::BUY_LIFE_TAX;
                    self.remaining_lifes += 1;
                }
            }
            event::KeyCode::Numpad2 => {
                if self.cash >= MainState::REMOVE_WORDS_TAX && self.words.len() > 0 {
                    self.cash -= MainState::REMOVE_WORDS_TAX;

                    if self.words.len() <= MainState::REMOVE_WORDS_COUNT {
                        self.words.iter_mut().for_each(|word| word.is_typed = true);
                    }
                    else {
                        let sample_indexes = seq::index::sample(&mut self.rng, self.words.len(), MainState::REMOVE_WORDS_COUNT);

                        for index in sample_indexes.iter() {
                            self.words[index].is_typed = true;
                        }
                    }
                }
            },
            event::KeyCode::Key2 => {
                if self.cash >= MainState::REMOVE_WORDS_TAX && self.words.len() > 0 {
                    self.cash -= MainState::REMOVE_WORDS_TAX;

                    if self.words.len() <= MainState::REMOVE_WORDS_COUNT {
                        self.words.iter_mut().for_each(|word| word.is_typed = true);
                    }
                    else {
                        let sample_indexes = seq::index::sample(&mut self.rng, self.words.len(), MainState::REMOVE_WORDS_COUNT);

                        for index in sample_indexes.iter() {
                            self.words[index].is_typed = true;
                        }
                    }
                }
            },
            event::KeyCode::Key3 => {
                if self.cash >= MainState::SLOW_WORD_SPAWN_TAX {
                    self.cash -= MainState::SLOW_WORD_SPAWN_TAX;
                    self.game_speed_up /= 2.0;
                }
            },
            event::KeyCode::Numpad3 => {
                if self.cash >= MainState::SLOW_WORD_SPAWN_TAX {
                    self.cash -= MainState::SLOW_WORD_SPAWN_TAX;
                    self.game_speed_up /= 2.0;
                }
            },
            event::KeyCode::Plus => {
                if self.sound_volume + MainState::SOUND_VOLUME_STEP <= 100.0 {
                    self.sound_volume += MainState::SOUND_VOLUME_STEP;
                    self.assets.background_music.set_volume(self.sound_volume);
                }
            },
            event::KeyCode::NumpadAdd => {
                if self.sound_volume + MainState::SOUND_VOLUME_STEP <= 100.0 {
                    self.sound_volume += MainState::SOUND_VOLUME_STEP;
                    self.assets.background_music.set_volume(self.sound_volume);
                }
            },
            event::KeyCode::NumpadSubtract => {
                if self.sound_volume - MainState::SOUND_VOLUME_STEP >= 0.0 {
                    self.sound_volume -= MainState::SOUND_VOLUME_STEP;
                    self.assets.background_music.set_volume(self.sound_volume);
                }
            },
            event::KeyCode::Grave => {
                self.show_info ^= true;
            }
            event::KeyCode::Minus => {
                self.current_input += "-";
            },
            event::KeyCode::A => {
                self.current_input = check_shift_pressed(self.current_input.clone(), ctx, "a", "A")
            },
            event::KeyCode::B => {
                self.current_input = check_shift_pressed(self.current_input.clone(), ctx, "b", "B")
            },
            event::KeyCode::C => {
                self.current_input = check_shift_pressed(self.current_input.clone(), ctx, "c", "C")
            },
            event::KeyCode::D => {
                self.current_input = check_shift_pressed(self.current_input.clone(), ctx, "d", "D")
            },
            event::KeyCode::E => {
                self.current_input = check_shift_pressed(self.current_input.clone(), ctx, "e", "E")
            },
            event::KeyCode::F => {
                self.current_input = check_shift_pressed(self.current_input.clone(), ctx, "f", "F")
            },
            event::KeyCode::G => {
                self.current_input = check_shift_pressed(self.current_input.clone(), ctx, "g", "G")
            },
            event::KeyCode::H => {
                self.current_input = check_shift_pressed(self.current_input.clone(), ctx, "h", "H")
            },
            event::KeyCode::I => {
                self.current_input = check_shift_pressed(self.current_input.clone(), ctx, "i", "I")
            },
            event::KeyCode::J => {
                self.current_input = check_shift_pressed(self.current_input.clone(), ctx, "j", "J")
            },
            event::KeyCode::K => {
                self.current_input = check_shift_pressed(self.current_input.clone(), ctx, "k", "K")
            },
            event::KeyCode::L => {
                self.current_input = check_shift_pressed(self.current_input.clone(), ctx, "l", "L")
            },
            event::KeyCode::M => {
                self.current_input = check_shift_pressed(self.current_input.clone(), ctx, "m", "M")
            },
            event::KeyCode::N => {
                self.current_input = check_shift_pressed(self.current_input.clone(), ctx, "n", "N")
            },
            event::KeyCode::O => {
                self.current_input = check_shift_pressed(self.current_input.clone(), ctx, "o", "O")
            },
            event::KeyCode::P => {
                self.current_input = check_shift_pressed(self.current_input.clone(), ctx, "p", "P")
            },
            event::KeyCode::Q => {
                self.current_input = check_shift_pressed(self.current_input.clone(), ctx, "q", "Q")
            },
            event::KeyCode::R => {
                self.current_input = check_shift_pressed(self.current_input.clone(), ctx, "r", "R")
            },
            event::KeyCode::S => {
                self.current_input = check_shift_pressed(self.current_input.clone(), ctx, "s", "S")
            },
            event::KeyCode::T => {
                self.current_input = check_shift_pressed(self.current_input.clone(), ctx, "t", "T")
            },
            event::KeyCode::U => {
                self.current_input = check_shift_pressed(self.current_input.clone(), ctx, "u", "U")
            },
            event::KeyCode::V => {
                self.current_input = check_shift_pressed(self.current_input.clone(), ctx, "v", "V")
            },
            event::KeyCode::W => {
                self.current_input = check_shift_pressed(self.current_input.clone(), ctx, "w", "W")
            },
            event::KeyCode::X => {
                self.current_input = check_shift_pressed(self.current_input.clone(), ctx, "x", "X")
            },
            event::KeyCode::Y => {
                self.current_input = check_shift_pressed(self.current_input.clone(), ctx, "y", "Y")
            },
            event::KeyCode::Z => {
                self.current_input = check_shift_pressed(self.current_input.clone(), ctx, "z", "Z")
            },
            event::KeyCode::Back => {
                self.current_input.pop();
            },
            _ => ()
        }
    }

    fn draw(&mut self, ctx: &mut Context) -> GameResult<()> {
        let background_color = graphics::Color::from_rgb(0, 0, 0);
        graphics::clear(ctx, background_color);

        let font = graphics::Font::new(ctx, "/RedHatDisplay-Regular.otf")?;

        // Game over scene
        if self.game_over {

            let ending;
            if self.typed_words < 5 {
                ending = "Bummer, I know you can do better :) Try again!";
            }
            else if self.typed_words >= 5 && self.typed_words < 20 {
                ending = "Not very bad!";
            }
            else if self.typed_words >= 20 && self.typed_words < 50 {
                ending = "Amazing, but can you do better?"
            }
            else {
                ending = "You're a madman, niiice :)"
            }

            let mut game_over_text = graphics::Text::new(format!("Game over!\nWords typed: {}\n{}", self.typed_words, ending));
            game_over_text.set_font(font, graphics::PxScale::from(40.0));

            let centered = Point2 {
                x: (self.screen_width - game_over_text.width(ctx)) / 2.0,
                y: (self.screen_height - game_over_text.height(ctx)) / 2.0
            };

            graphics::draw(ctx, &game_over_text, graphics::DrawParam::default().dest(centered))?;
            graphics::present(ctx)?;
            return Ok(())
        }

        // Game info panel
        if self.show_info {
            let mut game_info = graphics::Text::new(
                format!(
"(+) to volume up
(-) to volume down

Buffs become visible when you have the required cash:
(1) for extra life  ({}$)
(2) for words removal  ({}$)
(3) for slow words spawn  ({}$)

(Esc) to quit",
               MainState::BUY_LIFE_TAX,
               MainState::REMOVE_WORDS_TAX,
               MainState::SLOW_WORD_SPAWN_TAX));
            game_info.set_font(font, graphics::PxScale::from(34.0));
            let label_color = graphics::Color::from_rgb(48, 116, 115);

            let centered = Point2 {
                x: (self.screen_width - game_info.width(ctx)) / 2.0,
                y: (self.screen_height - game_info.height(ctx)) / 2.0
            };

            let margin = 30.0;
            let left = centered.x - margin;
            let right = centered.x + game_info.width(ctx) + margin;
            let top = centered.y - margin;
            let bottom = centered.y + game_info.height(ctx) + margin;

            let background = graphics::Rect::new(left, top, right - left, bottom - top);
            let draw_mode = graphics::DrawMode::Fill(graphics::FillOptions::DEFAULT);
            let silver = graphics::Color::from_rgb(192, 192, 192);
            let background_mesh = graphics::MeshBuilder::new().
                rectangle(draw_mode, background, silver).
                unwrap().
                build(ctx).
                unwrap();

            graphics::draw(ctx, &background_mesh, graphics::DrawParam::default())?;
            graphics::draw(ctx, &game_info, graphics::DrawParam::default().dest(centered).color(label_color))?;
        }

        let label_margin = 10.0;

        // Draw current volume
        let mut right_margin = 0.0;
        let mut options_label = graphics::Text::new(format!("(`) for Info|"));
        options_label.set_font(font, graphics::PxScale::from(34.0));

        let top_left = Point2 {
            x: label_margin,
            y: 0.0
        };
        right_margin += options_label.width(ctx) + label_margin;
        graphics::draw(ctx, &options_label, graphics::DrawParam::default().dest(top_left))?;

        let mut current_volume = graphics::Text::new(format!("Volume: {:.3}", self.sound_volume));
        current_volume.set_font(font, graphics::PxScale::from(34.0));

        let top_left = Point2 {
            x: right_margin + label_margin,
            y: 0.0
        };
        graphics::draw(ctx, &current_volume, graphics::DrawParam::default().dest(top_left))?;

        // Draw current user input
        let mut current_input = graphics::Text::new(format!("Input: {}", self.current_input));
        current_input.set_font(font, graphics::PxScale::from(40.0));

        let bottom_left = Point2 {
            x: 0.0,
            y: (self.screen_height - current_input.height(ctx))
        };
        graphics::draw(ctx, &current_input, graphics::DrawParam::default().dest(bottom_left))?;

        // Draw current cash
        let mut cash_label = graphics::Text::new(format!("Cash: {}", self.cash));
        cash_label.set_font(font, graphics::PxScale::from(40.0));

        let bottom_right = Point2 {
            x: (self.screen_width - cash_label.width(ctx) - label_margin),
            y: (self.screen_height - cash_label.height(ctx))
        };
        graphics::draw(ctx, &cash_label, graphics::DrawParam::default().dest(bottom_right))?;

        // Draw remaining lifes
        let mut lifes_label = graphics::Text::new(format!("Lifes: {}", self.remaining_lifes));
        lifes_label.set_font(font, graphics::PxScale::from(40.0));

        let next_to_cash = Point2 {
            x: (self.screen_width - cash_label.width(ctx) - lifes_label.width(ctx) - label_margin * 2.0),
            y: (self.screen_height - lifes_label.height(ctx))
        };
        graphics::draw(ctx, &lifes_label, graphics::DrawParam::default().dest(next_to_cash))?;

        // Draw power ups
        let mut left_margin = 0.0;
        if self.cash >= MainState::SLOW_WORD_SPAWN_TAX {
            let mut slow_word_spawn_label = graphics::Text::new(format!("(3) Slow spawn ({}$)", MainState::SLOW_WORD_SPAWN_TAX));
            slow_word_spawn_label.set_font(font, graphics::PxScale::from(34.0));

            let top_right = Point2 {
                x: (self.screen_width - slow_word_spawn_label.width(ctx) - label_margin - left_margin),
                y: 0.0
            };
            left_margin += slow_word_spawn_label.width(ctx) + label_margin;
            graphics::draw(ctx, &slow_word_spawn_label, graphics::DrawParam::default().dest(top_right))?;
        }

        if self.cash >= MainState::REMOVE_WORDS_TAX {
            let mut remove_words_label = graphics::Text::new(format!("(2) Remove {} words ({}$)",MainState::REMOVE_WORDS_COUNT , MainState::REMOVE_WORDS_TAX));
            remove_words_label.set_font(font, graphics::PxScale::from(34.0));

            let top_right = Point2 {
                x: (self.screen_width - remove_words_label.width(ctx) - label_margin - left_margin),
                y: 0.0
            };
            left_margin += remove_words_label.width(ctx) + label_margin;
            graphics::draw(ctx, &remove_words_label, graphics::DrawParam::default().dest(top_right))?;
        }

        if self.cash >= MainState::BUY_LIFE_TAX {
            let mut buy_life_label = graphics::Text::new(format!("(1) extra life ({}$)", MainState::BUY_LIFE_TAX));
            buy_life_label.set_font(font, graphics::PxScale::from(34.0));

            let top_right = Point2 {
                x: (self.screen_width - buy_life_label.width(ctx) - label_margin - left_margin),
                y: 0.0
            };
            graphics::draw(ctx, &buy_life_label, graphics::DrawParam::default().dest(top_right))?;
        }

        for word in self.words.iter_mut() {
            word.draw(ctx)?;
        }

        if debug::is_active() {
            for word in &mut self.words {
                debug::draw_outline(word.bounding_rect(ctx), ctx).unwrap();
            }
        }

        graphics::present(ctx)?;
        Ok(())
    }
}

fn check_shift_pressed(current_input: String, ctx: &mut Context, lower_letter: &str, upper_letter: &str) -> String {
    if is_key_pressed(ctx, event::KeyCode::LShift) ||
       is_key_pressed(ctx, event::KeyCode::RShift) {
        return current_input + upper_letter;
    }

    current_input + lower_letter
}