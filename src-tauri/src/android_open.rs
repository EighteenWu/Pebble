//! Android JNI entry points used during setup and later URL opens.

use jni::objects::JValue;

use crate::android_jni::{load_app_class, throw_if_exception, with_env};

const INTENTS_CLASS: &str = "com.qingj01.pebble.PebbleIntents";

pub fn open_url(url: &str) -> Result<(), String> {
    with_env(|env, context| {
        let class = load_app_class(env, &context, INTENTS_CLASS)
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
        throw_if_exception(env).map_err(|_| "Android could not open the URL".into())?;
        Ok(())
    })
}

/// Show a Toast / dialog so setup failures are not a silent native abort.
pub fn show_startup_error(message: &str) -> Result<(), String> {
    with_env(|env, context| {
        let class = load_app_class(env, &context, INTENTS_CLASS)
            .map_err(|e| format!("PebbleIntents class: {e}"))?;
        let jmessage = env
            .new_string(message)
            .map_err(|e| format!("startup error string: {e}"))?;
        env.call_static_method(
            class,
            "showStartupError",
            "(Landroid/content/Context;Ljava/lang/String;)V",
            &[JValue::Object(&context), JValue::Object(&jmessage)],
        )
        .map_err(|e| format!("showStartupError: {e}"))?;
        throw_if_exception(env)?;
        Ok(())
    })
}
