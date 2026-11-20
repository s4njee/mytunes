//! Installs the Java context CPAL uses when opening Android audio.

use std::sync::OnceLock;

use jni::EnvUnowned;
use jni::errors::{Error, Result, ThrowRuntimeExAndDefault};
use jni::objects::{Global, JObject};

static APP_CONTEXT: OnceLock<Global<JObject<'static>>> = OnceLock::new();

pub(crate) fn audio_ready() -> bool {
    APP_CONTEXT.get().is_some()
}

/// Receive the process-wide application context from MainActivity.
#[unsafe(no_mangle)]
extern "system" fn Java_io_github_s4njee_mytunes_1pod_MainActivity_initializeNativeAudio<'frame>(
    mut env: EnvUnowned<'frame>,
    _activity: JObject<'frame>,
    context: JObject<'frame>,
) {
    env.with_env(|env| -> Result<()> {
        if !audio_ready() {
            if context.is_null() {
                return Err(Error::NullPtr("Android application context"));
            }
            let vm = env.get_java_vm()?;
            let context = env.new_global_ref(context)?;
            APP_CONTEXT.get_or_init(move || {
                // SAFETY: The JVM and its application context live for the process.
                // OnceLock keeps the global reference alive and publishes it exactly once.
                unsafe {
                    ndk_context::initialize_android_context(
                        vm.get_raw().cast(),
                        context.as_obj().as_raw().cast(),
                    );
                }
                context
            });
        }
        Ok(())
    })
    .resolve::<ThrowRuntimeExAndDefault>();
}
