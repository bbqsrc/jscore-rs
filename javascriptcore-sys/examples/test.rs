use javascriptcore_sys::*;
use std::ptr::{null, null_mut};

macro_rules! retain_release {
    ($name:ident, $ffi_ref:ty, $retain_fn:tt, $drop_fn:tt) => {
        impl Drop for $name {
            fn drop(&mut self) {
                unsafe { $drop_fn(self.0) };
            }
        }

        impl Clone for $name {
            fn clone(&self) -> $name {
                let x = unsafe { $retain_fn(self.0) };
                $name(x)
            }
        }

        impl Deref for $name {
            type Target = $ffi_ref;

            fn deref(&self) -> &$ffi_ref {
                &self.0
            }
        }
    };
}

use std::ops::Deref;

struct ContextGroup(JSContextGroupRef);
struct GlobalContext(JSGlobalContextRef);

#[derive(Copy, Clone)]
struct Context(JSContextRef);

impl Deref for Context {
    type Target = JSContextRef;

    fn deref(&self) -> &JSContextRef {
        &self.0
    }
}

struct Object(Context, JSObjectRef);

impl Deref for Object {
    type Target = JSObjectRef;

    fn deref(&self) -> &JSObjectRef {
        &self.1
    }
}

struct String(JSStringRef);

retain_release!(
    ContextGroup,
    JSContextGroupRef,
    JSContextGroupRetain,
    JSContextGroupRelease
);
retain_release!(
    GlobalContext,
    JSGlobalContextRef,
    JSGlobalContextRetain,
    JSGlobalContextRelease
);
retain_release!(String, JSStringRef, JSStringRetain, JSStringRelease);

impl ContextGroup {
    pub fn new() -> ContextGroup {
        let ptr = unsafe { JSContextGroupCreate() };
        ContextGroup(ptr)
    }

    pub fn create_global_context(&self) -> GlobalContext {
        let ptr = unsafe { JSGlobalContextCreateInGroup(self.0, null_mut()) };
        GlobalContext(ptr)
    }
}

impl GlobalContext {
    pub fn global_object(&self) -> Object {
        let ptr = unsafe { JSContextGetGlobalObject(self.0) };
        Object(Context(self.0), ptr)
    }

    pub fn evaluate_script(&self, script: &String) -> JSValueRef {
        unsafe { JSEvaluateScript(self.0, **script, null_mut(), null_mut(), 0, null_mut()) }
    }
}

impl ContextType for GlobalContext {
    unsafe fn as_ptr(&self) -> JSContextRef {
        self.0
    }
}

impl ContextType for Context {
    unsafe fn as_ptr(&self) -> JSContextRef {
        self.0
    }
}

trait ContextType {
    unsafe fn as_ptr(&self) -> JSContextRef;

    fn undefined(&self) -> Value {
        let ptr = unsafe { self.as_ptr() };
        let value = unsafe { JSValueMakeUndefined(ptr) };
        Value(value, ValueType::Undefined, Context(ptr))
    }
}

type JsCallback =
    fn(Context, /*thisObject*/ Object, /*arguments*/ Vec<Value>) -> Result<Value, String>;

extern "C" fn callback_trampoline(
    ctx: JSContextRef,
    function: JSObjectRef,
    thisObject: JSObjectRef,
    argumentCount: usize,
    arguments: *const JSValueRef,
    exception: *mut JSValueRef,
) -> JSValueRef {
    // let callback: &JsCallback = unsafe { JSObjectGetPrivate(function) as _ };
    let callback = unsafe {
        std::mem::transmute::<*mut ::std::os::raw::c_void, JsCallback>(JSObjectGetPrivate(function))
    };
    println!("Callback {:?}", callback);
    let ctx = Context(ctx);

    let args = unsafe {
        std::slice::from_raw_parts(arguments, argumentCount)
            .into_iter()
            .map(|v| Value(*v, ValueType::from(ctx, *v), ctx))
            .collect::<Vec<_>>()
    };

    match callback(ctx, Object(ctx, thisObject), args) {
        Ok(v) => v.0,
        Err(e) => unsafe {
            *exception = e.to_js_value(&ctx);
            JSValueMakeUndefined(ctx.0)
        },
    }
}

impl ValueType {
    unsafe fn from(ctx: Context, value_ref: JSValueRef) -> ValueType {
        let raw_ty = JSValueGetType(ctx.0, value_ref);

        match raw_ty {
            0 => ValueType::Undefined,
            1 => ValueType::Null,
            2 => ValueType::Boolean,
            3 => ValueType::Number,
            4 => ValueType::String,
            5 => ValueType::Object,
            6 => ValueType::Symbol,
            _ => unreachable!(),
        }
    }
}

fn rust_function_defn(name: &String) -> JSClassDefinition {
    JSClassDefinition {
        version: 0,
        attributes: 0,
        className: **name as *const _,
        parentClass: null_mut(),
        staticValues: null(),
        staticFunctions: null(),
        initialize: None,
        finalize: None,
        hasProperty: None,
        getProperty: None,
        setProperty: None,
        deleteProperty: None,
        getPropertyNames: None,
        callAsFunction: Some(callback_trampoline),
        callAsConstructor: None,
        hasInstance: None,
        convertToType: None,
    }
}

#[derive(Debug, Copy, Clone)]
enum ValueType {
    Undefined,
    Null,
    Boolean,
    Number,
    String,
    Object,
    Symbol,
}

struct Value(JSValueRef, ValueType, Context);

impl Value {
    pub fn js_type(&self) -> ValueType {
        self.1
    }
}

use std::convert::TryFrom;

impl TryFrom<&Value> for String {
    type Error = Box<dyn std::error::Error>;

    fn try_from(value: &Value) -> Result<String, Self::Error> {
        match value.js_type() {
            ValueType::String => unsafe {
                let string = JSValueToStringCopy(*value.2, value.0, null_mut());
                if string == null_mut() {
                    panic!()
                }
                Ok(String(string))
            },
            _ => unimplemented!(),
        }
    }
}

impl TryFrom<&Value> for std::string::String {
    type Error = Box<dyn std::error::Error>;

    fn try_from(value: &Value) -> Result<std::string::String, Self::Error> {
        let js_string = String::try_from(value)?;
        Ok(std::string::String::from(&js_string))
    }
}

impl Object {
    pub fn make_function_with_callback(&self, name: &String, callback: JsCallback) -> Object {
        let cls = unsafe { JSClassCreate(&rust_function_defn(name)) };
        println!("{:?}", callback);
        let ptr = unsafe { JSObjectMake(*self.0, cls, callback as _) };
        if unsafe { JSObjectGetPrivate(ptr) } == null_mut() {
            panic!("No private");
        }
        Object(self.0, ptr)
    }

    pub fn set_property(&self, name: &String, value: JSValueRef) {
        unsafe { JSObjectSetProperty(*self.0, self.1, **name, value, 0, null_mut()) };
    }
}

use std::ffi::CString;

impl String {
    pub fn new(s: &str) -> Result<String, Box<dyn std::error::Error>> {
        let s = CString::new(s)?;
        let v = unsafe { JSStringCreateWithUTF8CString(s.as_ptr() as *const i8) };
        Ok(String(v))
    }

    pub fn to_js_value(&self, ctx: &Context) -> JSValueRef {
        unsafe { JSValueMakeString(**ctx, self.0) }
    }
}

impl From<&String> for std::string::String {
    fn from(string: &String) -> std::string::String {
        let size = unsafe { JSStringGetMaximumUTF8CStringSize(string.0) };
        let mut buffer = vec![0u8; size];
        let written = unsafe { JSStringGetUTF8CString(string.0, buffer.as_mut_ptr() as _, size) };
        buffer.truncate(written);

        let c_str = unsafe { std::ffi::CStr::from_bytes_with_nul_unchecked(&*buffer) };
        c_str.to_str().unwrap().to_string()
    }
}

fn log(ctx: Context, this: Object, arguments: Vec<Value>) -> Result<Value, String> {
    let v = std::string::String::try_from(&arguments[0]).unwrap();
    println!("Hello world, {}!", v);
    Ok(ctx.undefined())
}

fn main() {
    let ctx_group = ContextGroup::new();
    let ctx = ctx_group.create_global_context();
    let global = ctx.global_object();
    let fn_name = &String::new("log").unwrap();
    let fn_obj = global.make_function_with_callback(fn_name, log);
    global.set_property(fn_name, *fn_obj);
    ctx.evaluate_script(&String::new("log(\"you derp\")").unwrap());
}
