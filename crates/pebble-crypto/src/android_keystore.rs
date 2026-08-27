//! Android Keystore-backed DEK storage.
//!
//! `android-native-keyring-store` 1.x requires Rust 1.88 / edition 2024 and
//! cannot be resolved by the desktop toolchain used here. The desktop `keyring`
//! 3 crate also has no Android Keystore backend. This module talks to a Kotlin
//! helper (`com.qingj01.pebble.PebbleKeystore`) that wraps the DEK with an
//! AES/GCM key stored in Android Keystore and persists the ciphertext in
//! SharedPreferences.

use jni::objects::{JObject, JString, JValue};
use jni::JNIEnv;
use pebble_core::{PebbleError, Result};

use super::keystore::{DekStoreError, KEY_ENTRY, SERVICE_NAME};

const HELPER_CLASS: &str = "com/qingj01/pebble/PebbleKeystore";

pub struct AndroidKeystoreCredential;

fn with_env<T>(
    f: impl FnOnce(&mut JNIEnv<'_>, JObject<'_>) -> std::result::Result<T, DekStoreError>,
) -> std::result::Result<T, DekStoreError> {
    let ctx = ndk_context::android_context();
    let vm = ctx.vm().cast();
    if vm.is_null() {
        return Err(DekStoreError::Other(
            "Android NDK VM is not initialized".into(),
        ));
    }
    let java_vm = unsafe { jni::JavaVM::from_raw(vm) }
        .map_err(|e| DekStoreError::Other(format!("JNI VM: {e}")))?;
    let mut env = java_vm
        .attach_current_thread()
        .map_err(|e| DekStoreError::Other(format!("JNI attach: {e}")))?;
    let context = unsafe { JObject::from_raw(ctx.context() as jni::sys::jobject) };
    if context.is_null() {
        return Err(DekStoreError::Other(
            "Android application context is not initialized".into(),
        ));
    }
    f(&mut env, context)
}

fn throw_if_java_exception(env: &mut JNIEnv<'_>) -> std::result::Result<(), DekStoreError> {
    if !env.exception_check().unwrap_or(false) {
        return Ok(());
    }
    let _ = env.exception_describe();
    let _ = env.exception_clear();
    Err(DekStoreError::Other(
        "Android Keystore JNI helper threw an exception".into(),
    ))
}

impl super::keystore::DekCredential for AndroidKeystoreCredential {
    fn get_secret(&self) -> std::result::Result<Vec<u8>, DekStoreError> {
        with_env(|env, context| {
            let class = env
                .find_class(HELPER_CLASS)
                .map_err(|e| DekStoreError::Other(format!("PebbleKeystore class: {e}")))?;
            let service = env
                .new_string(SERVICE_NAME)
                .map_err(|e| DekStoreError::Other(e.to_string()))?;
            let entry = env
                .new_string(KEY_ENTRY)
                .map_err(|e| DekStoreError::Other(e.to_string()))?;
            let value = env
                .call_static_method(
                    class,
                    "getSecret",
                    "(Landroid/content/Context;Ljava/lang/String;Ljava/lang/String;)Ljava/lang/String;",
                    &[
                        JValue::Object(&context),
                        JValue::Object(&service),
                        JValue::Object(&entry),
                    ],
                )
                .map_err(|e| DekStoreError::Other(format!("getSecret: {e}")))?;
            throw_if_java_exception(env)?;
            let jobject = value.l().map_err(|e| DekStoreError::Other(e.to_string()))?;
            if jobject.is_null() {
                return Err(DekStoreError::NoEntry);
            }
            let jstring = JString::from(jobject);
            let rust = env
                .get_string(&jstring)
                .map_err(|e| DekStoreError::Other(e.to_string()))?;
            Ok(rust.to_string_lossy().into_owned().into_bytes())
        })
    }

    fn set_secret(&self, secret: &[u8]) -> std::result::Result<(), DekStoreError> {
        let text = std::str::from_utf8(secret)
            .map_err(|_| DekStoreError::Other("DEK secret must be UTF-8 hex".into()))?;
        with_env(|env, context| {
            let class = env
                .find_class(HELPER_CLASS)
                .map_err(|e| DekStoreError::Other(format!("PebbleKeystore class: {e}")))?;
            let service = env
                .new_string(SERVICE_NAME)
                .map_err(|e| DekStoreError::Other(e.to_string()))?;
            let entry = env
                .new_string(KEY_ENTRY)
                .map_err(|e| DekStoreError::Other(e.to_string()))?;
            let value = env
                .new_string(text)
                .map_err(|e| DekStoreError::Other(e.to_string()))?;
            env.call_static_method(
                class,
                "setSecret",
                "(Landroid/content/Context;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)V",
                &[
                    JValue::Object(&context),
                    JValue::Object(&service),
                    JValue::Object(&entry),
                    JValue::Object(&value),
                ],
            )
            .map_err(|e| DekStoreError::Other(format!("setSecret: {e}")))?;
            throw_if_java_exception(env)
        })
    }
}

pub fn delete_dek() -> Result<()> {
    AndroidKeystoreCredential
        .delete_secret()
        .or_else(|error| match error {
            DekStoreError::NoEntry => Ok(()),
            other => Err(PebbleError::Auth(format!("Failed to delete DEK: {other}"))),
        })
}

impl AndroidKeystoreCredential {
    fn delete_secret(&self) -> std::result::Result<(), DekStoreError> {
        with_env(|env, context| {
            let class = env
                .find_class(HELPER_CLASS)
                .map_err(|e| DekStoreError::Other(format!("PebbleKeystore class: {e}")))?;
            let service = env
                .new_string(SERVICE_NAME)
                .map_err(|e| DekStoreError::Other(e.to_string()))?;
            let entry = env
                .new_string(KEY_ENTRY)
                .map_err(|e| DekStoreError::Other(e.to_string()))?;
            env.call_static_method(
                class,
                "deleteSecret",
                "(Landroid/content/Context;Ljava/lang/String;Ljava/lang/String;)V",
                &[
                    JValue::Object(&context),
                    JValue::Object(&service),
                    JValue::Object(&entry),
                ],
            )
            .map_err(|e| DekStoreError::Other(format!("deleteSecret: {e}")))?;
            throw_if_java_exception(env)
        })
    }
}
