#![feature(allocator_api)]
#![feature(trait_upcasting)]

use acollections::AVec;
use libshim::NewValue;
use libshim::ShimError;
use libshim::ShimValue;
use libshim::Userdata;
use std::any::Any;
use std::ops::Deref;

use macroquad::prelude::*;
use macroquad::color::Color;

struct ColorHandle {
    color: Color,
}

impl Userdata for ColorHandle {}

#[macroquad::main("Shimlang Test")]
async fn main() {
    let allocator = std::alloc::Global;
    let mut interpreter = libshim::Interpreter::new(allocator);

    interpreter
        .add_global(
            b"mouse_pos_x",
            libshim::ShimValue::NativeFn(Box::new(move |args, interpreter| {
                if args.len() != 0 {
                    return Err(ShimError::Other(b"wrong arity"));
                }
                interpreter.new_value(ShimValue::F64(macroquad::input::mouse_position().0 as f64))
            })),
        )
        .unwrap();

    interpreter
        .add_global(
            b"mouse_pos_y",
            libshim::ShimValue::NativeFn(Box::new(move |args, interpreter| {
                if args.len() != 0 {
                    return Err(ShimError::Other(b"wrong arity"));
                }
                interpreter.new_value(ShimValue::F64(macroquad::input::mouse_position().1 as f64))
            })),
        )
        .unwrap();

    interpreter
        .add_global(
            b"str",
            libshim::ShimValue::NativeFn(Box::new(move |args, interpreter| {
                interpreter.new_value(ShimValue::SString(args[0].borrow().stringify(allocator)?))
            })),
        )
        .unwrap();

    interpreter
        .add_global(
            b"is_key_pressed",
            libshim::ShimValue::NativeFn(Box::new(move |args, interpreter| {
                if let ShimValue::I128(num) = &*args[0].borrow() {
                    interpreter.new_value(ShimValue::Bool(macroquad::input::is_key_down(
                        macroquad::input::KeyCode::from(*num as u32),
                    )))
                } else {
                    Ok(interpreter.g.the_unit.clone())
                }
            })),
        )
        .unwrap();

    interpreter
        .add_global(
            b"draw_text",
            libshim::ShimValue::NativeFn(Box::new(move |args, interpreter| {
                if args.len() != 5 {
                    return Err(ShimError::Other(b"wrong arity"));
                }

                let arg0 = &*args[0].borrow();
                let text = if let ShimValue::SString(text) = arg0 {
                    text
                } else {
                    return Err(ShimError::Other(b"arg 0 should be SString"));
                };
                let arg1 = &*args[1].borrow();
                let x = if let ShimValue::F64(x) = arg1 {
                    *x as f32
                } else {
                    return Err(ShimError::Other(b"arg 1 should be F64"));
                };
                let arg2 = &*args[2].borrow();
                let y = if let ShimValue::F64(y) = arg2 {
                    *y as f32
                } else {
                    return Err(ShimError::Other(b"arg 2 should be F64"));
                };
                let arg3 = &*args[3].borrow();
                let size = if let ShimValue::F64(size) = arg3 {
                    *size as f32
                } else {
                    return Err(ShimError::Other(b"arg 3 should be F64"));
                };
                let arg4 = &*args[4].borrow();
                let color = if let libshim::ShimValue::Userdata(u) = arg4 {
                    let data = u.deref() as &dyn Any;

                    if let Some(ColorHandle { color }) = data.downcast_ref::<ColorHandle>() {
                        color
                    } else {
                        return Err(ShimError::Other(b"arg 4 should be ColorHandle Userdata"));
                    }
                } else {
                    return Err(ShimError::Other(b"arg 4 should be Userdata"));
                };

                draw_text(
                    &std::str::from_utf8(text).unwrap().to_string(),
                    x,
                    y,
                    size,
                    *color,
                );

                Ok(interpreter.g.the_unit.clone())
            })),
        )
        .unwrap();

    interpreter
        .add_global(
            b"color",
            libshim::ShimValue::NativeFn(Box::new(move |args, interpreter| {
                if args.len() != 4 {
                    return Err(ShimError::Other(b"wrong arity"));
                }

                let arg0 = &*args[0].borrow();
                let r = if let ShimValue::F64(r) = arg0 {
                    *r as f32
                } else {
                    return Err(ShimError::Other(b"arg 0 should be SString"));
                };
                let arg1 = &*args[1].borrow();
                let g = if let ShimValue::F64(g) = arg1 {
                    *g as f32
                } else {
                    return Err(ShimError::Other(b"arg 1 should be F64"));
                };
                let arg2 = &*args[2].borrow();
                let b = if let ShimValue::F64(b) = arg2 {
                    *b as f32
                } else {
                    return Err(ShimError::Other(b"arg 2 should be F64"));
                };
                let arg3 = &*args[3].borrow();
                let a = if let ShimValue::F64(a) = arg3 {
                    *a as f32
                } else {
                    return Err(ShimError::Other(b"arg 3 should be F64"));
                };

                interpreter.new_value(ShimValue::Userdata(Box::new(ColorHandle {
                    color: Color::new(r, g, b, a),
                })))
            })),
        )
        .unwrap();

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

        loop_fn
            .borrow()
            .call(&AVec::new(allocator), &mut interpreter)
            .unwrap();

        next_frame().await
    }
}
