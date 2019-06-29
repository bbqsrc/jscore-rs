use javascriptcore_sys::*;
use std::convert::TryFrom;
use std::ffi::CString;
use std::ops::Deref;
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

unsafe impl Send for GlobalContext {}
unsafe impl Sync for GlobalContext {}
unsafe impl Send for Context {}
unsafe impl Sync for Context {}
unsafe impl Send for String {}
unsafe impl Sync for String {}
unsafe impl Send for Object {}
unsafe impl Sync for Object {}
unsafe impl Send for ContextGroup {}
unsafe impl Sync for ContextGroup {}
unsafe impl Send for Value {}
unsafe impl Sync for Value {}

#[derive(Copy, Clone, Debug)]
pub struct Context(pub(crate) JSContextRef);
pub struct ContextGroup(pub(crate) JSContextGroupRef);
pub struct GlobalContext(pub(crate) JSGlobalContextRef);
pub struct Object(pub(crate) Context, pub(crate) JSObjectRef);
pub struct String(pub(crate) JSStringRef);

use std::fmt;

impl fmt::Debug for Object {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut s = f.debug_struct("Object");

        unsafe {
            let array = JSObjectCopyPropertyNames(*self.0, self.1);
            let size = JSPropertyNameArrayGetCount(array);
            for i in 0..size {
                let js_ref = JSPropertyNameArrayGetNameAtIndex(array, i);
                let prop_name = std::string::String::from(&String(js_ref));
                let prop_value = Value::from(
                    self.0,
                    JSObjectGetPropertyAtIndex(*self.0, self.1, i as u32, null_mut()),
                );
                s.field(&prop_name, &format!("{:?}", prop_value));
            }
        }

        s.finish()
    }
}

impl fmt::Debug for Exception {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Exception")
            .field("stack", &self.stack())
            .field("message", &self.message())
            .finish()
    }
}

impl fmt::Display for Exception {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "Message: {}", &self.message())?;
        writeln!(f, "Stack:")?;
        write!(f, "{}", self.stack())
    }
}

#[derive(Debug, Copy, Clone)]
pub enum ValueType {
    Undefined,
    Null,
    Boolean,
    Number,
    String,
    Object,
    Symbol,
}

#[derive(Debug)]
pub struct Value(
    pub(crate) JSValueRef,
    pub(crate) ValueType,
    pub(crate) Context,
);

pub trait ContextType {
    unsafe fn as_ptr(&self) -> JSContextRef;

    fn undefined(&self) -> Value {
        let ptr = unsafe { self.as_ptr() };
        let value = unsafe { JSValueMakeUndefined(ptr) };
        Value(value, ValueType::Undefined, Context(ptr))
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

impl Deref for Context {
    type Target = JSContextRef;

    fn deref(&self) -> &JSContextRef {
        &self.0
    }
}

impl Deref for Object {
    type Target = JSObjectRef;

    fn deref(&self) -> &JSObjectRef {
        &self.1
    }
}

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

pub struct Exception(Object);

impl Exception {
    pub fn stack(&self) -> std::string::String {
        let stack_val = self
            .0
            .get_property(&String::new("stack").unwrap())
            .expect("no `stack` property found");
        let stack_str = String::try_from(&stack_val).expect("no string property found for `stack`");
        std::string::String::from(&stack_str)
    }

    pub fn message(&self) -> std::string::String {
        let message_val = self
            .0
            .get_property(&String::new("message").unwrap())
            .expect("no `message` property found");
        let message_str =
            String::try_from(&message_val).expect("no string property found for `message`");
        std::string::String::from(&message_str)
    }
}

impl GlobalContext {
    pub fn global_object(&self) -> Object {
        let ptr = unsafe { JSContextGetGlobalObject(self.0) };
        Object(Context(self.0), ptr)
    }

    pub fn evaluate_script_sync(&self, script: &String) -> Result<Value, Exception> {
        let mut exception = null();
        let ret = unsafe {
            JSEvaluateScript(self.0, **script, null_mut(), null_mut(), 0, &mut exception)
        };
        if exception == null_mut() {
            Ok(Value::from(Context(self.0), ret))
        } else {
            let value = Value::from(Context(self.0), exception);
            let obj = Object::try_from(&value).unwrap();
            Err(Exception(obj))
        }
    }

    pub async fn evaluate_script<'a>(&'a self, script: &'a String) -> Result<Value, Exception> {
        self.evaluate_script_sync(script)
    }

    pub fn add_function(
        &self,
        name: &str,
        callback: JsCallback,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let name = String::new(name).unwrap();
        let obj = self.global_object();
        let fn_obj = obj.make_function_with_callback(&name, callback);
        obj.set_property(&name, Value::from(Context(self.0), *fn_obj));
        Ok(())
    }
}

type JsCallback =
    fn(Context, /*thisObject*/ Object, /*arguments*/ Vec<Value>) -> Result<Value, String>;

extern "C" fn callback_trampoline(
    ctx: JSContextRef,
    function: JSObjectRef,
    this_object: JSObjectRef,
    argument_count: usize,
    arguments: *const JSValueRef,
    exception: *mut JSValueRef,
) -> JSValueRef {
    let callback = unsafe {
        std::mem::transmute::<*mut ::std::os::raw::c_void, JsCallback>(JSObjectGetPrivate(function))
    };

    let ctx = Context(ctx);

    let args = unsafe {
        std::slice::from_raw_parts(arguments, argument_count)
            .into_iter()
            .map(|v| Value::from(ctx, *v))
            .collect::<Vec<_>>()
    };

    match callback(ctx, Object(ctx, this_object), args) {
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

impl Value {
    fn from(ctx: Context, value_ref: JSValueRef) -> Value {
        Value(value_ref, unsafe { ValueType::from(ctx, value_ref) }, ctx)
    }

    pub fn to_string(&self) -> std::string::String {
        match self.js_type() {
            ValueType::String => {
                let js_str = String::try_from(self).expect("string");
                std::string::String::from(&js_str)
            }
            ValueType::Number => {
                let n = f64::try_from(self).expect("f64");
                format!("{}", n)
            }
            ValueType::Boolean => {
                let v = bool::try_from(self).expect("bool");
                format!("{}", v)
            }
            ValueType::Null => "null".into(),
            ValueType::Undefined => "undefined".into(),
            ValueType::Symbol => "Symbol(...)".into(),
            ValueType::Object => {
                let obj = Object::try_from(self).expect("object");
                format!("{:?}", obj)
            }
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

impl Value {
    pub fn js_type(&self) -> ValueType {
        self.1
    }
}

impl Object {
    pub fn make_function_with_callback(&self, name: &String, callback: JsCallback) -> Object {
        let cls = unsafe { JSClassCreate(&rust_function_defn(name)) };
        let ptr = unsafe { JSObjectMake(*self.0, cls, callback as _) };
        if unsafe { JSObjectGetPrivate(ptr) } == null_mut() {
            panic!("No private");
        }
        Object(self.0, ptr)
    }

    pub fn set_property(&self, name: &String, value: Value) {
        unsafe { JSObjectSetProperty(*self.0, self.1, **name, value.0, 0, null_mut()) };
    }

    pub fn get_property(&self, name: &String) -> Result<Value, Value> {
        let mut exception = null();
        let ret = unsafe { JSObjectGetProperty(*self.0, self.1, **name, &mut exception) };
        if exception == null() {
            Ok(Value::from(self.0, ret))
        } else {
            Err(Value::from(self.0, exception))
        }
    }

    pub fn to_js_value(&self) -> Value {
        Value(self.1, ValueType::Object, self.0)
    }
}

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
