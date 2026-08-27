//! Open http(s)/mailto URLs with an Android ACTION_VIEW intent.

use jni::objects::{JObject, JValue};
use jni::JNIEnv;

const HELPER_CLASS: &str = "com/qingj01/pebble/PebbleIntents";

pub fn open_url(url: &str) -> Result<(), String> {
    let ctx = ndk_context::android_context();
    let vm = ctx.vm().cast();
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
    call_open(&mut env, context, url)
}

fn call_open(env: &mut JNIEnv<'_>, context: JObject<'_>, url: &str) -> Result<(), String> {
    let class = env
        .find_class(HELPER_CLASS)
        .map_err(|e| format!("PebbleIntents class: {e}"))?;
    let jurl = env
        .new_string(url)
        .map_err(|e| format!("PebbleIntents url: {e}"))?;
    env.call_static_method(
        class,
        "openUrl",
        "(Landroid/content/Context;Ljava/lang/String;)V",
        &[JValue::Object(&context), JValue::Object(&jurl)],
    )
    .map_err(|e| format!("openUrl: {e}"))?;
    if env.exception_check().unwrap_or(false) {
        let _ = env.exception_describe();
        let _ = env.exception_clear();
        return Err("Android could not open the URL".into());
    }
    Ok(())
}
