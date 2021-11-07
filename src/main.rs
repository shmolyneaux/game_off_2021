#![allow(incomplete_features)]
#![feature(allocator_api)]
#![feature(trait_upcasting)]

use acollections::AVec;
use libshim::NewValue;
use libshim::ShimError;
use libshim::ShimValue;
use libshim::Userdata;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use libshim::ShimInto;

use macroquad::audio::Sound;
use macroquad::color::Color;
use macroquad::texture::{
    Texture2D,
    DrawTextureParams,
};
use macroquad::prelude::*;

struct ColorHandle {
    color: Color,
}

impl Userdata for ColorHandle {}

#[derive(Clone)]
struct DrawParamHandle {
    params: DrawTextureParams,
}

impl Userdata for DrawParamHandle {}

#[derive(Copy, Clone)]
enum AssetState<T> {
    Loading(usize),
    Loaded(T),
}

struct AssetHandle<T> {
    asset: Cell<AssetState<T>>,
}

impl<T: 'static> Userdata for AssetHandle<T> {}

struct TextureHandle {
    asset: Cell<AssetState<Texture2D>>,
    params: RefCell<DrawTextureParams>,
}

impl Userdata for TextureHandle {}

#[derive(Clone)]
struct AssetLoader<T> {
    // Requests to load new sounds are put here by the game script. These are
    // processed at the start of each loop.
    to_load: Rc<RefCell<Vec<(usize, String)>>>,

    // This stores the next ID for new sound load requests
    next_request_id: Rc<Cell<usize>>,

    // Sounds are put into this hashmap when they're loaded in the main loop.
    // A sound is removed from this hashmap and put into a loaded AssetHandle<T> the
    // first time it's played.
    loaded_asset: Rc<RefCell<HashMap<usize, T>>>,
}

impl<T> AssetLoader<T> {
    fn load(&self, path: &str) -> usize {
        let mut load_list = self.to_load.borrow_mut();

        let id = self.next_request_id.get();
        self.next_request_id.set(id + 1);

        load_list.push((id, path.to_string()));

        id
    }
}

impl<T> Default for AssetLoader<T> {
    fn default() -> Self {
        Self {
            to_load: Default::default(),
            next_request_id: Default::default(),
            loaded_asset: Default::default(),
        }
    }
}

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

    let sound_asset_loader_og: AssetLoader<Sound> = Default::default();
    let sound_asset_loader = sound_asset_loader_og.clone();
    shim_fn!(
        interpreter,
        fn load_sound(path: &str) {
            interpreter.new_value(ShimValue::Userdata(Box::new(AssetHandle {
                asset: Cell::new(AssetState::<Sound>::Loading(sound_asset_loader.load(path))),
            })))
        }
    );

    let sound_asset_loader = sound_asset_loader_og.clone();
    shim_fn!(
        interpreter,
        fn play_sound(sound_handle: &AssetHandle<Sound>) {
            match sound_handle.asset.get() {
                AssetState::Loaded(sound) => {
                    macroquad::audio::play_sound_once(sound);
                }
                AssetState::Loading(request_id) => {
                    if let Some(sound) = sound_asset_loader.loaded_asset
                        .borrow_mut()
                        .remove(&request_id)
                    {
                        macroquad::audio::play_sound_once(sound);
                        sound_handle.asset.set(AssetState::Loaded(sound));
                    }
                }
            }

            interpreter.new_value(())
        }
    );

    let texture_asset_loader_og: AssetLoader<Texture2D> = Default::default();
    let texture_asset_loader = texture_asset_loader_og.clone();
    shim_fn!(
        interpreter,
        fn load_texture(path: &str) {
            interpreter.new_value(ShimValue::Userdata(Box::new(TextureHandle {
                asset: Cell::new(AssetState::<Texture2D>::Loading(texture_asset_loader.load(path))),
                params: Default::default(),
            })))
        }
    );

    let texture_asset_loader = texture_asset_loader_og.clone();
    shim_fn!(
        interpreter,
        fn load_texture_with_params(path: &str, params: &DrawParamHandle) {
            interpreter.new_value(ShimValue::Userdata(Box::new(TextureHandle {
                asset: Cell::new(AssetState::<Texture2D>::Loading(texture_asset_loader.load(path))),
                params: RefCell::new((&params.params).clone()),
            })))
        }
    );

    shim_fn!(
        interpreter,
        fn update_texture_params(texture_handle: &TextureHandle, params: &DrawParamHandle) {
            texture_handle.params.replace(params.params.clone());
            interpreter.new_value(())
        }
    );

    shim_fn!(
        interpreter,
        fn draw_params(ds_x: f32, ds_y: f32, r_x: f32, r_y: f32, r_w: f32, r_h: f32, rot:f32, flip_x: bool, flip_y: bool) {
            interpreter.new_value(ShimValue::Userdata(Box::new(
                DrawParamHandle {
                    params: DrawTextureParams {
                        dest_size: Some(macroquad::math::Vec2::new(ds_x, ds_y)),
                        source: Some(macroquad::math::Rect::new(r_x, r_y, r_w, r_h)),
                        rotation: rot,
                        flip_x: flip_x,
                        flip_y: flip_y,
                        pivot: None,
                    },
                }
            )))
        }
    );

    shim_fn!(
        interpreter,
        fn default_draw_params() {
            interpreter.new_value(ShimValue::Userdata(Box::new(
                DrawParamHandle {
                    params: Default::default(),
                }
            )))
        }
    );

    let texture_asset_loader = texture_asset_loader_og.clone();
    shim_fn!(
        interpreter,
        fn draw_texture(texture_handle: &TextureHandle, x: f32, y: f32, color: &ColorHandle) {
            match (texture_handle.asset.get(), texture_handle.params.borrow()) {
                (AssetState::Loaded(texture), params) => {
                    macroquad::texture::draw_texture_ex(
                        texture, x, y, color.color, params.clone()
                    );
                }
                (AssetState::Loading(request_id), params) => {
                    if let Some(texture) = texture_asset_loader.loaded_asset
                        .borrow_mut()
                        .remove(&request_id)
                    {
                        texture.set_filter(macroquad::texture::FilterMode::Nearest);
                        macroquad::texture::draw_texture_ex(texture, x, y, color.color, params.clone());
                        texture_handle.asset.set(AssetState::Loaded(texture));
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

    shim_fn!(
        interpreter,
        fn frame_time() {
            interpreter.new_value(macroquad::telemetry::frame().full_frame_time)
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

    let should_exit: Rc<Cell<bool>> = Default::default();
    let should_exit_closed = should_exit.clone();
    shim_fn!(
        interpreter,
        fn exit() {
            should_exit_closed.set(true);
            interpreter.new_value(())
        }
    );

    let script = load_file("game.shm").await.unwrap();
    let loop_fn = interpreter.interpret(&script).unwrap();

    loop {
        while let Some((request_id, path)) = sound_asset_loader_og.to_load.borrow_mut().pop() {
            sound_asset_loader_og.loaded_asset.borrow_mut().insert(
                request_id,
                macroquad::audio::load_sound(&path).await.unwrap(),
            );
        }

        while let Some((request_id, path)) = texture_asset_loader_og.to_load.borrow_mut().pop() {
            texture_asset_loader_og.loaded_asset.borrow_mut().insert(
                request_id,
                macroquad::texture::load_texture(&path).await.unwrap(),
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

        if should_exit.get() {
            break;
        }

        next_frame().await;
    }
}
