use boa_engine::{
    Context, JsArgs, JsResult, JsValue, Source, js_string, native_function::NativeFunction,
    property::Attribute,
};
use scraper::{Html, Selector};
use std::sync::{Mutex, OnceLock};

use crate::fetch;
use crate::resolve;

static WRITE_BUFFER: OnceLock<Mutex<String>> = OnceLock::new();

pub struct JsExecutionResult {
    pub document_write_buffer: String,
}

pub fn execute_scripts(html_text: &str, base_path: &str) -> JsExecutionResult {
    let buffer = WRITE_BUFFER.get_or_init(|| Mutex::new(String::new()));
    buffer.lock().unwrap().clear();

    let document = Html::parse_document(html_text);
    let mut context = Context::default();

    register_console(&mut context);
    register_document(&mut context);
    register_window(&mut context);

    let script_selector = match Selector::parse("script") {
        Ok(s) => s,
        Err(_) => {
            return JsExecutionResult {
                document_write_buffer: String::new(),
            };
        }
    };

    for element in document.select(&script_selector) {
        if let Some(src) = element.value().attr("src") {
            let resolved = resolve::resolve_href_public(src, base_path);
            let code = if resolved.starts_with("http://") || resolved.starts_with("https://") {
                fetch::fetch_url(&resolved).unwrap_or_default()
            } else {
                std::fs::read_to_string(&resolved).unwrap_or_default()
            };
            if !code.is_empty() {
                run_script(&code, &mut context);
            }
        } else {
            let code = element.text().collect::<String>();
            if !code.trim().is_empty() {
                run_script(&code, &mut context);
            }
        }
    }

    JsExecutionResult {
        document_write_buffer: buffer.lock().unwrap().clone(),
    }
}

fn run_script(code: &str, context: &mut Context) {
    match context.eval(Source::from_bytes(code)) {
        Ok(_) => {}
        Err(e) => {
            eprintln!("[carmine-js] error: {}", e);
        }
    }
}

fn console_log_fn(_this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let msg: String = args
        .iter()
        .map(|v| {
            v.to_string(ctx)
                .map(|s| s.to_std_string_escaped())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ");
    println!("[carmine-js] {}", msg);
    Ok(JsValue::undefined())
}

fn noop_fn(_this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    Ok(JsValue::undefined())
}

fn return_null_fn(_this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    Ok(JsValue::null())
}

fn return_empty_array_fn(
    _this: &JsValue,
    _args: &[JsValue],
    ctx: &mut Context,
) -> JsResult<JsValue> {
    Ok(JsValue::from(
        boa_engine::object::JsObject::with_object_proto(&ctx.intrinsics()),
    ))
}

fn document_write_fn(_this: &JsValue, args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    if let Some(arg) = args.get_or_undefined(0).as_string() {
        let text = arg.to_std_string_escaped();
        if let Some(buffer) = WRITE_BUFFER.get() {
            buffer.lock().unwrap().push_str(&text);
        }
    }
    Ok(JsValue::undefined())
}

fn register_console(context: &mut Context) {
    let console = boa_engine::object::ObjectInitializer::new(context)
        .function(
            NativeFunction::from_fn_ptr(console_log_fn),
            js_string!("log"),
            0,
        )
        .function(
            NativeFunction::from_fn_ptr(console_log_fn),
            js_string!("warn"),
            0,
        )
        .function(
            NativeFunction::from_fn_ptr(console_log_fn),
            js_string!("error"),
            0,
        )
        .function(
            NativeFunction::from_fn_ptr(console_log_fn),
            js_string!("info"),
            0,
        )
        .build();
    let _ = context.register_global_property(js_string!("console"), console, Attribute::all());
}

fn register_document(context: &mut Context) {
    let intrinsics = context.intrinsics();
    let dummy_style = boa_engine::object::JsObject::with_object_proto(&intrinsics);
    let dummy_element = boa_engine::object::ObjectInitializer::new(context)
        .property(js_string!("style"), dummy_style, Attribute::all())
        .function(
            NativeFunction::from_fn_ptr(noop_fn),
            js_string!("appendChild"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(noop_fn),
            js_string!("removeChild"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(noop_fn),
            js_string!("insertBefore"),
            2,
        )
        .function(
            NativeFunction::from_fn_ptr(noop_fn),
            js_string!("setAttribute"),
            2,
        )
        .function(
            NativeFunction::from_fn_ptr(noop_fn),
            js_string!("addEventListener"),
            2,
        )
        .function(
            NativeFunction::from_fn_ptr(return_null_fn),
            js_string!("getAttribute"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(return_null_fn),
            js_string!("querySelector"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(return_empty_array_fn),
            js_string!("querySelectorAll"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(return_empty_array_fn),
            js_string!("getElementsByTagName"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(return_empty_array_fn),
            js_string!("getElementsByClassName"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(return_null_fn),
            js_string!("getElementById"),
            1,
        )
        .build();

    let document = boa_engine::object::ObjectInitializer::new(context)
        .function(
            NativeFunction::from_fn_ptr(document_write_fn),
            js_string!("write"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(document_write_fn),
            js_string!("writeln"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(noop_fn),
            js_string!("addEventListener"),
            2,
        )
        .function(
            NativeFunction::from_fn_ptr(noop_fn),
            js_string!("createElement"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(noop_fn),
            js_string!("createTextNode"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(return_null_fn),
            js_string!("getElementById"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(return_null_fn),
            js_string!("querySelector"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(return_empty_array_fn),
            js_string!("querySelectorAll"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(return_empty_array_fn),
            js_string!("getElementsByTagName"),
            1,
        )
        .property(
            js_string!("documentElement"),
            dummy_element.clone(),
            Attribute::all(),
        )
        .property(js_string!("head"), dummy_element.clone(), Attribute::all())
        .property(js_string!("body"), dummy_element, Attribute::all())
        .property(
            js_string!("readyState"),
            js_string!("complete"),
            Attribute::all(),
        )
        .build();

    let _ = context.register_global_property(js_string!("document"), document, Attribute::all());
}

fn register_window(context: &mut Context) {
    let location = boa_engine::object::ObjectInitializer::new(context)
        .property(js_string!("href"), js_string!(""), Attribute::all())
        .property(js_string!("hostname"), js_string!(""), Attribute::all())
        .property(js_string!("origin"), js_string!(""), Attribute::all())
        .build();

    let navigator = boa_engine::object::ObjectInitializer::new(context)
        .property(
            js_string!("userAgent"),
            js_string!("Carmine/0.1"),
            Attribute::all(),
        )
        .property(
            js_string!("platform"),
            js_string!("Scarlet"),
            Attribute::all(),
        )
        .property(js_string!("language"), js_string!("en"), Attribute::all())
        .build();

    let window = boa_engine::object::ObjectInitializer::new(context)
        .property(js_string!("location"), location.clone(), Attribute::all())
        .property(js_string!("navigator"), navigator.clone(), Attribute::all())
        .property(
            js_string!("innerWidth"),
            JsValue::from(1000),
            Attribute::all(),
        )
        .property(
            js_string!("innerHeight"),
            JsValue::from(700),
            Attribute::all(),
        )
        .function(
            NativeFunction::from_fn_ptr(noop_fn),
            js_string!("addEventListener"),
            2,
        )
        .function(
            NativeFunction::from_fn_ptr(noop_fn),
            js_string!("removeEventListener"),
            2,
        )
        .build();

    let _ = context.register_global_property(js_string!("window"), window, Attribute::all());
    let _ = context.register_global_property(js_string!("location"), location, Attribute::all());
    let _ = context.register_global_property(js_string!("navigator"), navigator, Attribute::all());
    let _ = context.register_global_property(
        js_string!("innerWidth"),
        JsValue::from(1000),
        Attribute::all(),
    );
    let _ = context.register_global_property(
        js_string!("innerHeight"),
        JsValue::from(700),
        Attribute::all(),
    );

    let _ = context.register_global_callable(
        js_string!("setTimeout"),
        1,
        NativeFunction::from_fn_ptr(|_, _args, _ctx| Ok(JsValue::from(0))),
    );
    let _ = context.register_global_callable(
        js_string!("clearTimeout"),
        1,
        NativeFunction::from_fn_ptr(noop_fn),
    );
    let _ = context.register_global_callable(
        js_string!("setInterval"),
        1,
        NativeFunction::from_fn_ptr(|_, _args, _ctx| Ok(JsValue::from(0))),
    );
    let _ = context.register_global_callable(
        js_string!("clearInterval"),
        1,
        NativeFunction::from_fn_ptr(noop_fn),
    );
    let _ = context.register_global_callable(
        js_string!("requestAnimationFrame"),
        1,
        NativeFunction::from_fn_ptr(|_, _args, _ctx| Ok(JsValue::from(0))),
    );
    let _ = context.register_global_callable(
        js_string!("addEventListener"),
        2,
        NativeFunction::from_fn_ptr(noop_fn),
    );
}
