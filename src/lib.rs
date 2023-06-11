use std::collections::HashMap;
use std::sync::Mutex;

use redis_module::{redis_module, Context, RedisError, RedisResult, RedisString, Status, RedisValue};

use wasmtime::*;
use lazy_static::lazy_static;

lazy_static! {
    static ref GLOBAL_ENGINE: Engine = Engine::default();
    static ref GLOBAL_STORE: Mutex<Store<()>> = Mutex::new(Store::new(&GLOBAL_ENGINE, ()));
    static ref INSTANCE_MAP: Mutex<HashMap<String, Instance>> = Mutex::new(HashMap::new());
}

fn init(_ctx: &Context, _args: &[RedisString]) -> Status {

    Status::Ok
}

fn deinit(_: &Context) -> Status {

    Status::Ok
}

fn load_file(_ctx: &Context, args: Vec<RedisString>) -> RedisResult {
    if args.len() < 3 {
        return Err(RedisError::WrongArity);
    }
    let namespace = args[1].to_string();
    let path = args[2].to_string();
    let module = Module::from_file(&GLOBAL_ENGINE, path)
        .map_err(|e| RedisError::String(format!("Failed to load wasm file: {:?}", e)))?;

    let instance = Instance::new(&mut *GLOBAL_STORE.lock().unwrap(), &module, &[])
        .map_err(|e| RedisError::String(format!("Failed to instantiate wasm: {:?}", e)))?;

    // insert instance into map
    INSTANCE_MAP.lock()?.insert(namespace, instance);

    Ok(RedisValue::Bool(true))
}

fn wasm_call(ctx: &Context, args: Vec<RedisString>) -> RedisResult {
    if args.len() < 3 {
        return Err(RedisError::WrongArity);
    }
    let namespace = args[1].to_string();
    let func_name = args[2].to_string();

    let binding = INSTANCE_MAP.lock()?;
    let instance = binding.get(&namespace)
        .ok_or_else(|| RedisError::String(format!("No module loaded for namespace: {}", namespace)))?;
    let func = instance.get_func(&mut *GLOBAL_STORE.lock().unwrap(), &func_name)
        .ok_or_else(|| RedisError::String(format!("No function named {} in module {}", func_name, namespace)))?;

    // need to know the function signature
    // let sig = func.ty(&mut *GLOBAL_STORE.lock().unwrap());
    // let wasm_input = sig.params().into_iter().enumerate().map(| (i, ty) | {
    //     match ty {
    //         ValType::I32 => args[i + 3].parse_integer().unwrap().to_owned(),
    //         ValType::I64 => args[i + 3].parse_integer().unwrap().into(),
    //         ValType::F32 => args[i + 3].parse_float().unwrap().into(),
    //         ValType::F64 => args[i + 3].parse_float().unwrap().into(),
    //         _ => panic!("Unsupported type"),
    //     }
    // }).collect::<Vec<_>>();

    let func = func.typed::<(), i32>(&mut *GLOBAL_STORE.lock().unwrap())
        .map_err(|e| RedisError::String(format!("Failed to type function: {:?}", e)))?;
    let result = func.call(&mut *GLOBAL_STORE.lock().unwrap(), ())
        .map_err(|e| RedisError::String(format!("Failed to call function: {:?}", e)))?;
    ctx.log(
        redis_module::LogLevel::Notice,
        &format!("======= call answer result: {result}",),
    );
    Ok(RedisValue::Integer(result.into()))
}

// fn hello_mul(_ctx: &Context, args: Vec<RedisString>) -> RedisResult {

//     // let module = Module::new(&ENGINE_INST, r#"(module
//     //     (func (export "answer") (result i32)
//     //        i32.const 42
//     //     )
//     //   )
//     // "#).unwrap();
//     // let mut store = Store::new(&ENGINE_INST, ());
//     // let instance = Instance::new(&mut store, &module, &[]).unwrap();
//     // if args.len() < 2 {
//     //     return Err(RedisError::WrongArity);
//     // }
//     // let answer = instance.get_func(&mut store, "answer")
//     // .expect("`answer` was not an exported function");
//     // let answer = answer.typed::<(), i32>(&store).unwrap();
//     // let result = answer.call(&mut store, ()).unwrap();
//     // ctx.log(
//     //     RedisLogLevel::Notice,
//     //     &format!("======= call answer result: {result}",),
//     // );
//     let nums = args
//         .into_iter()
//         .skip(1)
//         .map(|s| s.parse_integer())
//         .collect::<Result<Vec<i64>, RedisError>>()?;

//     let product = nums.iter().product();

//     let mut response = nums;
//     response.push(product);

//     Ok(response.into())
// }

//////////////////////////////////////////////////////

redis_module! {
    name: "wasm",
    version: 1,
    allocator: (redis_module::alloc::RedisAlloc, redis_module::alloc::RedisAlloc),
    data_types: [],
    init: init,
    deinit: deinit,
    commands: [
        ["wasm.load", load_file, "", 0, 0, 0],
        ["wasm.call", wasm_call, "", 0, 0, 0],
    ],
}
