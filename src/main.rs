#![feature(allocator_api)]
#![feature(trait_upcasting)]

use acollections::AVec;
use libshim::NewValue;
use libshim::ShimError;
use libshim::ShimValue;
use libshim::Userdata;
use std::any::Any;
use std::ops::Deref;

use libshim::ShimInto;

use macroquad::color::Color;
use macroquad::prelude::*;

struct ColorHandle {
    color: Color,
}

impl Userdata for ColorHandle {}

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
    let allocator = std::alloc::Global;
    let mut interpreter = libshim::Interpreter::new(allocator);

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
            let key_is_down = macroquad::input::is_key_down(
                macroquad::input::KeyCode::from(key),
            );
            interpreter.new_value(key_is_down)
        }
    );

    shim_fn!(
        interpreter,
        fn draw_text(text: &str, x: f32, y: f32, size: f32, color_handle: &ColorHandle) {
            draw_text(
                text,
                x,
                y,
                size,
                color_handle.color
            );

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
            b"game_print",
            libshim::ShimValue::NativeFn(Box::new(move |args, interpreter| {
                for arg in args.iter() {
                    match &*arg.borrow() {
                        libshim::ShimValue::I128(i) => println!("i128: {}", i),
                        libshim::ShimValue::F64(f) => println!("f64: {}", f),
                        libshim::ShimValue::Bool(b) => println!("bool: {}", b),
                        libshim::ShimValue::Unit => println!("()"),
                        libshim::ShimValue::Userdata(u) => {
                            let data = u.deref() as &dyn Any;

                            if let Some(_handle) = data.downcast_ref::<ColorHandle>() {
                                println!("got color handle!");
                            } else {
                                println!("not a color handle");
                            }
                        }
                        _ => println!("other"),
                    }
                }
                Ok(interpreter.g.the_unit.clone())
            })),
        )
        .unwrap();

    let script = load_file("game.shm").await.unwrap();
    let loop_fn = interpreter.interpret(&script).unwrap();

    loop {
        clear_background(BLACK);

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

        next_frame().await
    }
}
