//! Shared Android JNI helpers.
//!
//! `JNIEnv::find_class` on a native thread attached from Rust uses the system
//! classloader and cannot see `com.qingj01.pebble.*`. Load app classes through
//! `Context.getClassLoader().loadClass(...)`.

use jni::objects::{JClass, JObject, JValue};
use jni::JNIEnv;

pub fn with_env<T>(
    f: impl FnOnce(&mut JNIEnv<'_>, JObject<'_>) -> Result<T, String>,
) -> Result<T, String> {
    let ctx = ndk_context::android_context();
    let vm: *mut jni::sys::JavaVM = ctx.vm().cast();
    if vm.is_null() {
        return Err("Android NDK VM is not initialized".into());
    }
    let java_vm = unsafe { jni::JavaVM::from_raw(vm) }.map_err(|e| format!("JNI VM: {e}"))?;
    let mut env = java_vm
        .attach_current_thread()
        .map_err(|e| format!("JNI attach: {e}"))?;
    let context = unsafe { JObject::from_raw(ctx.context() as jni::sys::jobject) };
    if context.is_null() {
        return Err("Android application context is not initialized".into());
    }
    f(&mut env, context)
}

pub fn throw_if_exception(env: &mut JNIEnv<'_>) -> Result<(), String> {
    if !env.exception_check().unwrap_or(false) {
        return Ok(());
    }
    let _ = env.exception_describe();
    let _ = env.exception_clear();
    Err("Android JNI helper threw an exception".into())
}

pub fn load_app_class<'local>(
    env: &mut JNIEnv<'local>,
    context: &JObject<'local>,
    dotted_name: &str,
) -> Result<JClass<'local>, String> {
    let loader = env
        .call_method(context, "getClassLoader", "()Ljava/lang/ClassLoader;", &[])
        .map_err(|e| format!("getClassLoader: {e}"))?;
    throw_if_exception(env)?;
    let loader = loader.l().map_err(|e| e.to_string())?;
    if loader.is_null() {
        return Err("Context.getClassLoader() returned null".into());
    }

    let name = env
        .new_string(dotted_name)
        .map_err(|e| format!("class name string: {e}"))?;
    let class = env
        .call_method(
            &loader,
            "loadClass",
            "(Ljava/lang/String;)Ljava/lang/Class;",
            &[JValue::Object(&name)],
        )
        .map_err(|e| format!("loadClass({dotted_name}): {e}"))?;
    throw_if_exception(env)?;
    let class = class.l().map_err(|e| e.to_string())?;
    if class.is_null() {
        return Err(format!(
            "ClassLoader.loadClass returned null for {dotted_name}"
        ));
    }
    Ok(JClass::from(class))
}
