#![allow(incomplete_features)]
#![feature(allocator_api)]
#![feature(trait_upcasting)]

use acollections::AVec;
use libshim::NewValue;
use libshim::ShimError;
use libshim::ShimValue;
use libshim::Userdata;

use std::cell::Cell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Mutex;

use libshim::ShimInto;

use macroquad::audio::Sound;
use macroquad::color::Color;
use macroquad::prelude::*;

struct ColorHandle {
    color: Color,
}

impl Userdata for ColorHandle {}

#[derive(Copy, Clone)]
enum SoundState {
    Loading(usize),
    Loaded(Sound),
}

struct SoundHandle {
    sound: Cell<SoundState>,
}

impl Userdata for SoundHandle {}

macro_rules! unpack_args {
    ($args:ident, $count:expr,) => {};
    ($args:ident, $count:expr, $arg_v:ident:$arg_t:ty) => {
        let arg0 = &*$args[$count].borrow();
        let $arg_v: $arg_t = arg0.shim_into()?;
    };
    ($args:ident, $count:expr, $arg_v:ident:$arg_t:ty, $($xs_arg_v:ident:$xs_arg_t:ty),*) => {
        let arg0 = &*$args[$count].borrow();
        let $arg_v: $arg_t = arg0.shim_into()?;
        unpack_args!(
            $args,
            $count + 1,
            $($xs_arg_v:$xs_arg_t),*
        );
    }
}

macro_rules! count {
    () => {0};
    ($arg_v:ident:$arg_t:ty) => {
        1
    };
    ($arg_v:ident:$arg_t:ty, $($xs_arg_v:ident:$xs_arg_t:ty),*) => {
        1 + count!($($xs_arg_v:$xs_arg_t),*)
    }
}

macro_rules! shim_fn {
    (
        $interpreter:ident,
        fn $name:ident ($($arg_v:ident:$arg_t:ty),*) $code:tt) => {
        $interpreter
            .add_global(
                stringify!($name).as_bytes(),
                libshim::ShimValue::NativeFn(Box::new(move |args, $interpreter| {
                    if args.len() != count!($($arg_v:$arg_t),*) {
                        return Err(ShimError::Other(b"wrong arity"));
                    }

                    unpack_args!(args, 0, $($arg_v:$arg_t),*);

                    $code
                })),
            )
            .unwrap();
    };
}

#[macroquad::main("Shimlang Test")]
async fn main() {
    macroquad::file::set_pc_assets_folder("assets");

    let allocator = std::alloc::Global;
    let mut interpreter = libshim::Interpreter::new(allocator);

    // Requests to load new sounds are put here by the game script. These are
    // processed at the start of each loop.
    let sounds_to_load: Rc<Mutex<Vec<(usize, String)>>> = Rc::default();
    let sounds_to_load_copy = sounds_to_load.clone();

    // This stores the next ID for new sound load requests
    let next_sound_request_id_src: Rc<Cell<usize>> = Rc::default();
    let next_sound_request_id = next_sound_request_id_src.clone();

    // Sounds are put into this hashmap when they're loaded in the main loop.
    // A sound is removed from this hashmap and put into a loaded SoundHandle the
    // first time it's played.
    let unplayed_but_loaded_sounds: Rc<Mutex<HashMap<usize, Sound>>> = Rc::default();
    let unplayed_but_loaded_sounds_copy = unplayed_but_loaded_sounds.clone();

    shim_fn!(
        interpreter,
        fn load_sound(path: &str) {
            let mut test = sounds_to_load_copy.lock().unwrap();

            let id = next_sound_request_id.get();
            next_sound_request_id.set(id + 1);

            test.push((id, path.to_string()));

            interpreter.new_value(ShimValue::Userdata(Box::new(SoundHandle {
                sound: Cell::new(SoundState::Loading(id)),
            })))
        }
    );

    shim_fn!(
        interpreter,
        fn play_sound(sound_handle: &SoundHandle) {
            match sound_handle.sound.get() {
                SoundState::Loaded(sound) => {
                    macroquad::audio::play_sound_once(sound);
                }
                SoundState::Loading(request_id) => {
                    if let Some(sound) = unplayed_but_loaded_sounds_copy
                        .lock()
                        .unwrap()
                        .remove(&request_id)
                    {
                        macroquad::audio::play_sound_once(sound);
                        sound_handle.sound.set(SoundState::Loaded(sound));
                    }
                }
            }

            interpreter.new_value(())
        }
    );

    shim_fn!(
        interpreter,
        fn mouse_pos_x() {
            interpreter.new_value(macroquad::input::mouse_position().0 as f64)
        }
    );

    shim_fn!(
        interpreter,
        fn mouse_pos_y() {
            interpreter.new_value(macroquad::input::mouse_position().1 as f64)
        }
    );

    interpreter
        .add_global(
            b"str",
            libshim::ShimValue::NativeFn(Box::new(move |args, interpreter| {
                interpreter.new_value(ShimValue::SString(args[0].borrow().stringify(allocator)?))
            })),
        )
        .unwrap();

    shim_fn!(
        interpreter,
        fn is_key_pressed(key: u32) {
            let key_is_down = macroquad::input::is_key_down(macroquad::input::KeyCode::from(key));
            interpreter.new_value(key_is_down)
        }
    );

    shim_fn!(
        interpreter,
        fn draw_text(text: &str, x: f32, y: f32, size: f32, color_handle: &ColorHandle) {
            draw_text(text, x, y, size, color_handle.color);

            Ok(interpreter.g.the_unit.clone())
        }
    );

    shim_fn!(
        interpreter,
        fn color(r: f32, g: f32, b: f32, a: f32) {
            interpreter.new_value(ShimValue::Userdata(Box::new(ColorHandle {
                color: Color::new(r, g, b, a),
            })))
        }
    );

    interpreter
        .add_global(
            b"debug",
            libshim::ShimValue::NativeFn(Box::new(move |args, interpreter| {
                for arg in args.iter() {
                    dbg!(&*arg.borrow());
                }
                Ok(interpreter.g.the_unit.clone())
            })),
        )
        .unwrap();

    let script = load_file("game.shm").await.unwrap();
    let loop_fn = interpreter.interpret(&script).unwrap();

    loop {
        while let Some((request_id, path)) = sounds_to_load.lock().unwrap().pop() {
            unplayed_but_loaded_sounds.lock().unwrap().insert(
                request_id,
                macroquad::audio::load_sound(&path).await.unwrap(),
            );
        }

        let result = loop_fn
            .borrow()
            .call(&AVec::new(allocator), &mut interpreter);

        match result {
            Ok(_) => {}
            Err(ShimError::Other(text)) => {
                println!("ERROR: {}", std::str::from_utf8(text).unwrap());
                return;
            }
            Err(_) => {
                println!("Some other error");
                return;
            }
        }

        next_frame().await;
    }
}
