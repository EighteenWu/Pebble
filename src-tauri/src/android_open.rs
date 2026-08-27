//! Android JNI entry points used during setup and later URL opens.

use std::sync::OnceLock;

use jni::objects::JValue;

use crate::android_jni::{load_app_class, throw_if_exception, with_env};

const INTENTS_CLASS: &str = "com.qingj01.pebble.PebbleIntents";
const CRASH_CLASS: &str = "com.qingj01.pebble.PebbleCrash";

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
        throw_if_exception(env).map_err(|_| "Android could not open the URL".to_string())?;
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

/// Append a breadcrumb or panic text to filesDir/pebble-crash.log (and the
/// app-visible external files dir) through [PebbleCrash].
pub fn write_startup_log(message: &str) -> Result<(), String> {
    with_env(|env, context| {
        let class = load_app_class(env, &context, CRASH_CLASS)
            .map_err(|e| format!("PebbleCrash class: {e}"))?;
        let jmessage = env
            .new_string(message)
            .map_err(|e| format!("crash log string: {e}"))?;
        env.call_static_method(
            class,
            "append",
            "(Landroid/content/Context;Ljava/lang/String;)V",
            &[JValue::Object(&context), JValue::Object(&jmessage)],
        )
        .map_err(|e| format!("PebbleCrash.append: {e}"))?;
        throw_if_exception(env)?;
        Ok(())
    })
}

pub fn install_panic_hook() {
    static INSTALLED: OnceLock<()> = OnceLock::new();
    INSTALLED.get_or_init(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let message = format!("panic hook: {info}");
            let _ = write_startup_log(&message);
            let _ = show_startup_error(&format!("Pebble crashed while starting: {info}"));
            previous(info);
        }));
    });
}
