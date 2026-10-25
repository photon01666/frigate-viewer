mod config;
mod pages;
mod server;

use std::sync::{Arc, Mutex};
use crate::config::{is_reachable, ConfigStore};

struct AppState {
    store: Arc<ConfigStore>,
    settings_url: String,
    overlay_script: String,
}

static APP_STATE: Mutex<Option<AppState>> = Mutex::new(None);

fn state_slot() -> &'static Mutex<Option<AppState>> {
    &APP_STATE
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "system" fn Java_com_frigateviewer_app_MainActivity_nativeStart<'local>(
    mut env: jni::JNIEnv<'local>,
    _class: jni::objects::JClass<'local>,
    data_dir: jni::objects::JString<'local>,
) -> jni::objects::JString<'local> {
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(if cfg!(debug_assertions) {
                log::LevelFilter::Debug
            } else {
                log::LevelFilter::Info
            })
            .with_tag("FrigateViewer"),
    );
    std::panic::set_hook(Box::new(|info| {
        log::error!("panic in Frigate Viewer: {info}");
    }));

    let data_dir = match env.get_string(&data_dir) {
        Ok(value) => value.to_string_lossy().into_owned(),
        Err(err) => {
            log::error!("could not read data directory: {err}");
            return env
                .new_string("")
                .unwrap_or_else(|_| unsafe { jni::objects::JString::from_raw(std::ptr::null_mut()) });
        }
    };

    let config_path = std::path::Path::new(&data_dir).join("config.json");
    log::info!("config file: {}", config_path.display());

    let store = Arc::new(ConfigStore::new(config_path));
    let local = match server::start(Arc::clone(&store)) {
        Ok(local) => local,
        Err(err) => {
            log::error!("could not start the local setup server: {err}");
            return env
                .new_string("about:blank")
                .unwrap_or_else(|_| unsafe { jni::objects::JString::from_raw(std::ptr::null_mut()) });
        }
    };

    let existing = store.get();
    let initial_url = match &existing {
        Some(config) => match is_reachable(&config.server_url, std::time::Duration::from_millis(2500)) {
            Ok(()) => config.server_url.clone(),
            Err(err) => {
                log::info!("saved Frigate server is not reachable at startup: {err}");
                format!("{}?offline=1", local.base_url())
            }
        },
        None => local.base_url().to_string(),
    };
    log::info!(
        "initial page: {}",
        if existing.is_some() { "Frigate" } else { "setup" }
    );

    let settings_url = local.settings_url().to_string();
    let overlay_script = if settings_url.is_empty() {
        String::new()
    } else {
        include_str!("../assets/overlay.js").replace("__SETTINGS_URL__", &settings_url)
    };

    {
        let mut slot = state_slot()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *slot = Some(AppState {
            store,
            settings_url,
            overlay_script,
        });
    }

    env.new_string(initial_url)
        .unwrap_or_else(|_| unsafe { jni::objects::JString::from_raw(std::ptr::null_mut()) })
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "system" fn Java_com_frigateviewer_app_MainActivity_nativeStop(
    _env: jni::JNIEnv,
    _class: jni::objects::JClass,
) {
    let mut slot = state_slot()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    *slot = None;
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "system" fn Java_com_frigateviewer_app_MainActivity_nativeAllowInvalidCertificates(
    _env: jni::JNIEnv,
    _class: jni::objects::JClass,
) -> jni::sys::jboolean {
    if crate::config::allow_invalid_certificates() {
        jni::sys::JNI_TRUE
    } else {
        jni::sys::JNI_FALSE
    }
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "system" fn Java_com_frigateviewer_app_MainActivity_nativeKeepScreenOn(
    _env: jni::JNIEnv,
    _class: jni::objects::JClass,
) -> jni::sys::jboolean {
    if crate::config::keep_screen_on() {
        jni::sys::JNI_TRUE
    } else {
        jni::sys::JNI_FALSE
    }
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "system" fn Java_com_frigateviewer_app_MainActivity_nativeGetOverlayScript<'local>(
    mut env: jni::JNIEnv<'local>,
    _class: jni::objects::JClass<'local>,
) -> jni::objects::JString<'local> {
    let script = {
        let slot = state_slot()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        slot
            .as_ref()
            .map(|state| state.overlay_script.clone())
            .unwrap_or_default()
    };

    env.new_string(script)
        .unwrap_or_else(|_| unsafe { jni::objects::JString::from_raw(std::ptr::null_mut()) })
}

#[cfg(target_os = "android")]
fn data_url(html: &str) -> String {
    let mut out = String::from("data:text/html;charset=utf-8,");
    for byte in html.bytes() {
        if byte.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&byte) {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}
