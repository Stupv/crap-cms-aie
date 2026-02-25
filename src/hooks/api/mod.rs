//! Registers the `crap.*` Lua API namespace (collections, globals, hooks, log, util,
//! crypto, schema).

pub mod parse;
mod util;
mod schema;

use anyhow::{Context, Result};
use mlua::{Lua, Table, Value, Function};
use std::path::Path;

use crate::config::CrapConfig;
use crate::core::SharedRegistry;

use parse::{parse_collection_definition, parse_global_definition};

/// Register the `crap` global table with sub-tables for collections, globals, log, util,
/// auth, env, http, config.
pub fn register_api(lua: &Lua, registry: SharedRegistry, _config_dir: &Path, config: &CrapConfig) -> Result<()> {
    let crap = lua.create_table().context("Failed to create crap table")?;

    register_collections(lua, &crap, registry.clone())?;
    register_globals(lua, &crap, registry.clone())?;
    register_log(lua, &crap)?;
    util::register_util(lua, &crap)?;
    register_crypto(lua, &crap, &config.auth.secret)?;
    schema::register_schema(lua, &crap, registry.clone())?;
    register_hooks(lua, &crap)?;
    register_auth(lua, &crap)?;
    register_env(lua, &crap)?;
    register_http(lua, &crap)?;
    register_config(lua, &crap, config)?;
    register_locale(lua, &crap, config)?;
    register_jobs(lua, &crap, registry.clone())?;
    register_email(lua, &crap, config)?;

    lua.globals().set("crap", crap)?;

    // Load pure Lua helpers onto crap.util (after crap global is set)
    util::load_lua_helpers(lua)?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Per-namespace registration helpers
// ---------------------------------------------------------------------------

/// Register `crap.collections` — define, config.get, config.list.
fn register_collections(lua: &Lua, crap: &Table, registry: SharedRegistry) -> Result<()> {
    let collections_table = lua.create_table()?;
    let reg_clone = registry.clone();
    let define_collection = lua.create_function(move |lua, (slug, config): (String, Table)| {
        let def = parse_collection_definition(lua, &slug, &config)
            .map_err(|e| mlua::Error::RuntimeError(format!(
                "Failed to parse collection '{}': {}", slug, e
            )))?;
        let mut reg = reg_clone.write()
            .map_err(|e| mlua::Error::RuntimeError(format!("Registry lock poisoned: {}", e)))?;
        reg.register_collection(def);
        Ok(())
    })?;
    collections_table.set("define", define_collection)?;

    let reg_clone = registry.clone();
    let get_collection = lua.create_function(move |lua, slug: String| -> mlua::Result<Value> {
        let reg = reg_clone.read()
            .map_err(|e| mlua::Error::RuntimeError(format!("Registry lock poisoned: {}", e)))?;
        match reg.get_collection(&slug) {
            Some(def) => Ok(Value::Table(collection_config_to_lua(lua, def)?)),
            None => Ok(Value::Nil),
        }
    })?;
    let collections_config_table = lua.create_table()?;
    collections_config_table.set("get", get_collection)?;

    let reg_clone = registry.clone();
    let list_collections = lua.create_function(move |lua, ()| -> mlua::Result<Table> {
        let reg = reg_clone.read()
            .map_err(|e| mlua::Error::RuntimeError(format!("Registry lock poisoned: {}", e)))?;
        let map = lua.create_table()?;
        for (slug, def) in reg.collections.iter() {
            map.set(slug.as_str(), collection_config_to_lua(lua, def)?)?;
        }
        Ok(map)
    })?;
    collections_config_table.set("list", list_collections)?;
    collections_table.set("config", collections_config_table)?;

    crap.set("collections", collections_table)?;
    Ok(())
}

/// Register `crap.globals` — define, config.get, config.list.
fn register_globals(lua: &Lua, crap: &Table, registry: SharedRegistry) -> Result<()> {
    let globals_table = lua.create_table()?;
    let reg_clone = registry.clone();
    let define_global = lua.create_function(move |lua, (slug, config): (String, Table)| {
        let def = parse_global_definition(lua, &slug, &config)
            .map_err(|e| mlua::Error::RuntimeError(format!(
                "Failed to parse global '{}': {}", slug, e
            )))?;
        let mut reg = reg_clone.write()
            .map_err(|e| mlua::Error::RuntimeError(format!("Registry lock poisoned: {}", e)))?;
        reg.register_global(def);
        Ok(())
    })?;
    globals_table.set("define", define_global)?;

    let reg_clone = registry.clone();
    let get_global = lua.create_function(move |lua, slug: String| -> mlua::Result<Value> {
        let reg = reg_clone.read()
            .map_err(|e| mlua::Error::RuntimeError(format!("Registry lock poisoned: {}", e)))?;
        match reg.get_global(&slug) {
            Some(def) => Ok(Value::Table(global_config_to_lua(lua, def)?)),
            None => Ok(Value::Nil),
        }
    })?;
    let globals_config_table = lua.create_table()?;
    globals_config_table.set("get", get_global)?;

    let reg_clone = registry.clone();
    let list_globals = lua.create_function(move |lua, ()| -> mlua::Result<Table> {
        let reg = reg_clone.read()
            .map_err(|e| mlua::Error::RuntimeError(format!("Registry lock poisoned: {}", e)))?;
        let map = lua.create_table()?;
        for (slug, def) in reg.globals.iter() {
            map.set(slug.as_str(), global_config_to_lua(lua, def)?)?;
        }
        Ok(map)
    })?;
    globals_config_table.set("list", list_globals)?;
    globals_table.set("config", globals_config_table)?;

    crap.set("globals", globals_table)?;
    Ok(())
}

/// Register `crap.log` — info, warn, error.
fn register_log(lua: &Lua, crap: &Table) -> Result<()> {
    let log_table = lua.create_table()?;
    let log_info = lua.create_function(|_, msg: String| {
        tracing::info!("[lua] {}", msg);
        Ok(())
    })?;
    let log_warn = lua.create_function(|_, msg: String| {
        tracing::warn!("[lua] {}", msg);
        Ok(())
    })?;
    let log_error = lua.create_function(|_, msg: String| {
        tracing::error!("[lua] {}", msg);
        Ok(())
    })?;
    log_table.set("info", log_info)?;
    log_table.set("warn", log_warn)?;
    log_table.set("error", log_error)?;
    crap.set("log", log_table)?;
    Ok(())
}

/// Register `crap.crypto` — sha256, hmac, base64, AES-GCM encrypt/decrypt, random_bytes.
fn register_crypto(lua: &Lua, crap: &Table, auth_secret: &str) -> Result<()> {
    let crypto_table = lua.create_table()?;

    let sha256_fn = lua.create_function(|_, data: String| -> mlua::Result<String> {
        use ring::digest;
        let hash = digest::digest(&digest::SHA256, data.as_bytes());
        Ok(hex_encode(hash.as_ref()))
    })?;
    crypto_table.set("sha256", sha256_fn)?;

    let hmac_sha256_fn = lua.create_function(|_, (data, key): (String, String)| -> mlua::Result<String> {
        use ring::hmac;
        let k = hmac::Key::new(hmac::HMAC_SHA256, key.as_bytes());
        let tag = hmac::sign(&k, data.as_bytes());
        Ok(hex_encode(tag.as_ref()))
    })?;
    crypto_table.set("hmac_sha256", hmac_sha256_fn)?;

    let b64_encode_fn = lua.create_function(|_, data: String| -> mlua::Result<String> {
        use base64::Engine;
        Ok(base64::engine::general_purpose::STANDARD.encode(data.as_bytes()))
    })?;
    crypto_table.set("base64_encode", b64_encode_fn)?;

    let b64_decode_fn = lua.create_function(|_, data: String| -> mlua::Result<String> {
        use base64::Engine;
        let bytes = base64::engine::general_purpose::STANDARD.decode(data.as_bytes())
            .map_err(|e| mlua::Error::RuntimeError(format!("base64 decode error: {}", e)))?;
        String::from_utf8(bytes)
            .map_err(|e| mlua::Error::RuntimeError(format!("base64 decode utf8 error: {}", e)))
    })?;
    crypto_table.set("base64_decode", b64_decode_fn)?;

    let secret = auth_secret.to_string();
    let encrypt_fn = lua.create_function(move |_, plaintext: String| -> mlua::Result<String> {
        use aes_gcm::{Aes256Gcm, KeyInit, aead::Aead};
        use aes_gcm::Nonce;
        use ring::digest;
        use base64::Engine;
        use rand::RngCore;

        let key_hash = digest::digest(&digest::SHA256, secret.as_bytes());
        let cipher = Aes256Gcm::new_from_slice(key_hash.as_ref())
            .map_err(|e| mlua::Error::RuntimeError(format!("cipher init: {}", e)))?;

        let mut nonce_bytes = [0u8; 12];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher.encrypt(nonce, plaintext.as_bytes())
            .map_err(|e| mlua::Error::RuntimeError(format!("encrypt error: {}", e)))?;

        let mut combined = nonce_bytes.to_vec();
        combined.extend_from_slice(&ciphertext);
        Ok(base64::engine::general_purpose::STANDARD.encode(&combined))
    })?;
    crypto_table.set("encrypt", encrypt_fn)?;

    let secret2 = auth_secret.to_string();
    let decrypt_fn = lua.create_function(move |_, encoded: String| -> mlua::Result<String> {
        use aes_gcm::{Aes256Gcm, KeyInit, aead::Aead};
        use aes_gcm::Nonce;
        use ring::digest;
        use base64::Engine;

        let combined = base64::engine::general_purpose::STANDARD.decode(encoded.as_bytes())
            .map_err(|e| mlua::Error::RuntimeError(format!("base64 decode: {}", e)))?;
        if combined.len() < 12 {
            return Err(mlua::Error::RuntimeError("ciphertext too short".into()));
        }
        let (nonce_bytes, ciphertext) = combined.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);

        let key_hash = digest::digest(&digest::SHA256, secret2.as_bytes());
        let cipher = Aes256Gcm::new_from_slice(key_hash.as_ref())
            .map_err(|e| mlua::Error::RuntimeError(format!("cipher init: {}", e)))?;

        let plaintext = cipher.decrypt(nonce, ciphertext)
            .map_err(|e| mlua::Error::RuntimeError(format!("decrypt error: {}", e)))?;

        String::from_utf8(plaintext)
            .map_err(|e| mlua::Error::RuntimeError(format!("decrypt utf8: {}", e)))
    })?;
    crypto_table.set("decrypt", decrypt_fn)?;

    let random_bytes_fn = lua.create_function(|_, n: usize| -> mlua::Result<String> {
        use rand::RngCore;
        let mut buf = vec![0u8; n];
        rand::thread_rng().fill_bytes(&mut buf);
        Ok(hex_encode(&buf))
    })?;
    crypto_table.set("random_bytes", random_bytes_fn)?;

    crap.set("crypto", crypto_table)?;
    Ok(())
}

/// Register `crap.hooks` — register/remove global event hooks, plus `_crap_event_hooks` storage.
fn register_hooks(lua: &Lua, crap: &Table) -> Result<()> {
    // _crap_event_hooks — Lua-side storage for registered global hooks
    let event_hooks = lua.create_table()?;
    lua.globals().set("_crap_event_hooks", event_hooks)?;

    let hooks_table = lua.create_table()?;

    let register_fn = lua.create_function(|lua, (event, func): (String, Function)| {
        let globals = lua.globals();
        let event_hooks: Table = globals.get("_crap_event_hooks")?;
        let list: Table = match event_hooks.get::<Value>(event.as_str())? {
            Value::Table(t) => t,
            _ => {
                let t = lua.create_table()?;
                event_hooks.set(event.as_str(), t.clone())?;
                t
            }
        };
        let len = list.raw_len();
        list.set(len + 1, func)?;
        Ok(())
    })?;
    hooks_table.set("register", register_fn)?;

    let remove_fn = lua.create_function(|lua, (event, func): (String, Function)| {
        let globals = lua.globals();
        let event_hooks: Table = globals.get("_crap_event_hooks")?;
        let list: Table = match event_hooks.get::<Value>(event.as_str())? {
            Value::Table(t) => t,
            _ => return Ok(()),
        };
        let rawequal: Function = globals.get("rawequal")?;
        let len = list.raw_len();
        let mut remove_idx = None;
        for i in 1..=len {
            let entry: Value = list.raw_get(i)?;
            let eq: bool = rawequal.call((entry, func.clone()))?;
            if eq {
                remove_idx = Some(i);
                break;
            }
        }
        if let Some(idx) = remove_idx {
            let table_remove: Function = lua.load("table.remove").eval()?;
            table_remove.call::<()>((list, idx))?;
        }
        Ok(())
    })?;
    hooks_table.set("remove", remove_fn)?;

    crap.set("hooks", hooks_table)?;
    Ok(())
}

/// Register `crap.auth` — hash_password, verify_password.
fn register_auth(lua: &Lua, crap: &Table) -> Result<()> {
    let auth_table = lua.create_table()?;
    let hash_pw_fn = lua.create_function(|_, password: String| {
        crate::core::auth::hash_password(&password)
            .map_err(|e| mlua::Error::RuntimeError(format!("hash_password error: {}", e)))
    })?;
    let verify_pw_fn = lua.create_function(|_, (password, hash): (String, String)| {
        crate::core::auth::verify_password(&password, &hash)
            .map_err(|e| mlua::Error::RuntimeError(format!("verify_password error: {}", e)))
    })?;
    auth_table.set("hash_password", hash_pw_fn)?;
    auth_table.set("verify_password", verify_pw_fn)?;
    crap.set("auth", auth_table)?;
    Ok(())
}

/// Register `crap.env` — read-only env var access.
fn register_env(lua: &Lua, crap: &Table) -> Result<()> {
    let env_table = lua.create_table()?;
    let env_get_fn = lua.create_function(|_, key: String| -> mlua::Result<Option<String>> {
        match std::env::var(&key) {
            Ok(val) => Ok(Some(val)),
            Err(_) => Ok(None),
        }
    })?;
    env_table.set("get", env_get_fn)?;
    crap.set("env", env_table)?;
    Ok(())
}

/// Register `crap.http` — outbound HTTP via ureq (blocking, safe in spawn_blocking context).
fn register_http(lua: &Lua, crap: &Table) -> Result<()> {
    let http_table = lua.create_table()?;
    let http_request_fn = lua.create_function(|lua, opts: Table| -> mlua::Result<Table> {
        let url: String = opts.get("url")?;
        let method: String = opts.get::<Option<String>>("method")?
            .unwrap_or_else(|| "GET".to_string())
            .to_uppercase();
        let timeout: u64 = opts.get::<Option<u64>>("timeout")?.unwrap_or(30);
        let body: Option<String> = opts.get("body")?;

        let agent = ureq::AgentBuilder::new()
            .timeout(std::time::Duration::from_secs(timeout))
            .build();

        let mut req = match method.as_str() {
            "GET" => agent.get(&url),
            "POST" => agent.post(&url),
            "PUT" => agent.put(&url),
            "PATCH" => agent.request("PATCH", &url),
            "DELETE" => agent.delete(&url),
            "HEAD" => agent.head(&url),
            _ => return Err(mlua::Error::RuntimeError(
                format!("unsupported HTTP method: {}", method)
            )),
        };

        // Set request headers
        if let Ok(headers_tbl) = opts.get::<Table>("headers") {
            for pair in headers_tbl.pairs::<String, String>() {
                let (k, v) = pair?;
                req = req.set(&k, &v);
            }
        }

        // Send request
        let response = if let Some(body_str) = body {
            req.send_string(&body_str)
        } else {
            req.call()
        };

        let result = lua.create_table()?;
        match response {
            Ok(resp) => {
                result.set("status", resp.status() as i64)?;
                let headers_out = lua.create_table()?;
                for name in resp.headers_names() {
                    if let Some(val) = resp.header(&name) {
                        headers_out.set(name.as_str(), val)?;
                    }
                }
                result.set("headers", headers_out)?;
                let body_str = resp.into_string()
                    .map_err(|e| mlua::Error::RuntimeError(
                        format!("failed to read response body: {}", e)
                    ))?;
                result.set("body", body_str)?;
            }
            Err(ureq::Error::Status(code, resp)) => {
                result.set("status", code as i64)?;
                let headers_out = lua.create_table()?;
                for name in resp.headers_names() {
                    if let Some(val) = resp.header(&name) {
                        headers_out.set(name.as_str(), val)?;
                    }
                }
                result.set("headers", headers_out)?;
                let body_str = resp.into_string().unwrap_or_default();
                result.set("body", body_str)?;
            }
            Err(ureq::Error::Transport(e)) => {
                return Err(mlua::Error::RuntimeError(
                    format!("HTTP transport error: {}", e)
                ));
            }
        }

        Ok(result)
    })?;
    http_table.set("request", http_request_fn)?;
    crap.set("http", http_table)?;
    Ok(())
}

/// Register `crap.config` — read-only config access with dot notation.
fn register_config(lua: &Lua, crap: &Table, config: &CrapConfig) -> Result<()> {
    let config_table = lua.create_table()?;
    let config_json = serde_json::to_value(config)
        .map_err(|e| anyhow::anyhow!("Failed to serialize config: {}", e))?;
    let config_lua = json_to_lua(lua, &config_json)?;
    lua.globals().set("_crap_config", config_lua)?;

    let config_get_fn = lua.create_function(|lua, key: String| -> mlua::Result<Value> {
        let config_val: Value = lua.globals().get("_crap_config")?;
        let mut current = config_val;
        for part in key.split('.') {
            match current {
                Value::Table(tbl) => {
                    current = tbl.get(part)?;
                }
                _ => return Ok(Value::Nil),
            }
        }
        Ok(current)
    })?;
    config_table.set("get", config_get_fn)?;
    crap.set("config", config_table)?;
    Ok(())
}

/// Register `crap.locale` — locale configuration access.
fn register_locale(lua: &Lua, crap: &Table, config: &CrapConfig) -> Result<()> {
    let locale_table = lua.create_table()?;

    let default_locale = config.locale.default_locale.clone();
    let get_default_fn = lua.create_function(move |_, ()| -> mlua::Result<String> {
        Ok(default_locale.clone())
    })?;
    locale_table.set("get_default", get_default_fn)?;

    let locales = config.locale.locales.clone();
    let get_all_fn = lua.create_function(move |lua, ()| -> mlua::Result<Table> {
        let tbl = lua.create_table()?;
        for (i, l) in locales.iter().enumerate() {
            tbl.set(i + 1, l.as_str())?;
        }
        Ok(tbl)
    })?;
    locale_table.set("get_all", get_all_fn)?;

    let enabled = config.locale.is_enabled();
    let is_enabled_fn = lua.create_function(move |_, ()| -> mlua::Result<bool> {
        Ok(enabled)
    })?;
    locale_table.set("is_enabled", is_enabled_fn)?;

    crap.set("locale", locale_table)?;
    Ok(())
}

/// Register `crap.jobs` — job definition.
fn register_jobs(lua: &Lua, crap: &Table, registry: SharedRegistry) -> Result<()> {
    let jobs_table = lua.create_table()?;
    let reg_clone = registry.clone();
    let define_job = lua.create_function(move |_lua, (slug, config): (String, Table)| {
        let def = parse::parse_job_definition(&slug, &config)
            .map_err(|e| mlua::Error::RuntimeError(format!(
                "Failed to parse job '{}': {}", slug, e
            )))?;
        let mut reg = reg_clone.write()
            .map_err(|e| mlua::Error::RuntimeError(format!("Registry lock poisoned: {}", e)))?;
        reg.register_job(def);
        Ok(())
    })?;
    jobs_table.set("define", define_job)?;
    crap.set("jobs", jobs_table)?;
    Ok(())
}

/// Register `crap.email` — outbound email sending via SMTP.
fn register_email(lua: &Lua, crap: &Table, config: &CrapConfig) -> Result<()> {
    let email_table = lua.create_table()?;
    let email_config = config.email.clone();
    let email_send_fn = lua.create_function(move |_, opts: Table| -> mlua::Result<bool> {
        let to: String = opts.get("to")?;
        let subject: String = opts.get("subject")?;
        let html: String = opts.get("html")?;
        let text: Option<String> = opts.get("text")?;

        crate::core::email::send_email(
            &email_config,
            &to,
            &subject,
            &html,
            text.as_deref(),
        ).map_err(|e| mlua::Error::RuntimeError(format!("email send error: {}", e)))?;

        Ok(true)
    })?;
    email_table.set("send", email_send_fn)?;
    crap.set("email", email_table)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Encode bytes as lowercase hex string.
fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Convert a LocalizedString to a Lua value (string or locale table).
fn localized_string_to_lua(lua: &Lua, ls: &crate::core::field::LocalizedString) -> mlua::Result<Value> {
    match ls {
        crate::core::field::LocalizedString::Plain(s) => {
            Ok(Value::String(lua.create_string(s)?))
        }
        crate::core::field::LocalizedString::Localized(map) => {
            let tbl = lua.create_table()?;
            for (k, v) in map {
                tbl.set(k.as_str(), v.as_str())?;
            }
            Ok(Value::Table(tbl))
        }
    }
}

/// Convert a CollectionDefinition to a full Lua table compatible with parse_collection_definition().
/// Unlike collection_def_to_lua_table (used by crap.schema), this produces a round-trip compatible
/// table that can be passed back to crap.collections.define().
fn collection_config_to_lua(lua: &Lua, def: &crate::core::CollectionDefinition) -> mlua::Result<Table> {
    let tbl = lua.create_table()?;

    // labels
    let labels = lua.create_table()?;
    if let Some(ref s) = def.labels.singular {
        labels.set("singular", localized_string_to_lua(lua, s)?)?;
    }
    if let Some(ref s) = def.labels.plural {
        labels.set("plural", localized_string_to_lua(lua, s)?)?;
    }
    tbl.set("labels", labels)?;

    tbl.set("timestamps", def.timestamps)?;

    // admin
    let admin = lua.create_table()?;
    if let Some(ref s) = def.admin.use_as_title {
        admin.set("use_as_title", s.as_str())?;
    }
    if let Some(ref s) = def.admin.default_sort {
        admin.set("default_sort", s.as_str())?;
    }
    if def.admin.hidden {
        admin.set("hidden", true)?;
    }
    if !def.admin.list_searchable_fields.is_empty() {
        let lsf = lua.create_table()?;
        for (i, f) in def.admin.list_searchable_fields.iter().enumerate() {
            lsf.set(i + 1, f.as_str())?;
        }
        admin.set("list_searchable_fields", lsf)?;
    }
    tbl.set("admin", admin)?;

    // fields
    let fields_arr = lua.create_table()?;
    for (i, f) in def.fields.iter().enumerate() {
        fields_arr.set(i + 1, field_config_to_lua(lua, f)?)?;
    }
    tbl.set("fields", fields_arr)?;

    // hooks
    let hooks = collection_hooks_to_lua(lua, &def.hooks)?;
    tbl.set("hooks", hooks)?;

    // access
    let access = lua.create_table()?;
    if let Some(ref s) = def.access.read { access.set("read", s.as_str())?; }
    if let Some(ref s) = def.access.create { access.set("create", s.as_str())?; }
    if let Some(ref s) = def.access.update { access.set("update", s.as_str())?; }
    if let Some(ref s) = def.access.delete { access.set("delete", s.as_str())?; }
    tbl.set("access", access)?;

    // auth
    if let Some(ref auth) = def.auth {
        if auth.enabled {
            if auth.strategies.is_empty()
                && !auth.disable_local
                && !auth.verify_email
                && auth.forgot_password
                && auth.token_expiry == 7200
            {
                tbl.set("auth", true)?;
            } else {
                let auth_tbl = lua.create_table()?;
                auth_tbl.set("token_expiry", auth.token_expiry)?;
                if auth.disable_local {
                    auth_tbl.set("disable_local", true)?;
                }
                if auth.verify_email {
                    auth_tbl.set("verify_email", true)?;
                }
                if !auth.forgot_password {
                    auth_tbl.set("forgot_password", false)?;
                }
                if !auth.strategies.is_empty() {
                    let strats = lua.create_table()?;
                    for (i, s) in auth.strategies.iter().enumerate() {
                        let st = lua.create_table()?;
                        st.set("name", s.name.as_str())?;
                        st.set("authenticate", s.authenticate.as_str())?;
                        strats.set(i + 1, st)?;
                    }
                    auth_tbl.set("strategies", strats)?;
                }
                tbl.set("auth", auth_tbl)?;
            }
        }
    }

    // upload
    if let Some(ref upload) = def.upload {
        if upload.enabled {
            if upload.mime_types.is_empty()
                && upload.max_file_size.is_none()
                && upload.image_sizes.is_empty()
                && upload.admin_thumbnail.is_none()
                && upload.format_options.webp.is_none()
                && upload.format_options.avif.is_none()
            {
                tbl.set("upload", true)?;
            } else {
                let u = lua.create_table()?;
                if !upload.mime_types.is_empty() {
                    let mt = lua.create_table()?;
                    for (i, m) in upload.mime_types.iter().enumerate() {
                        mt.set(i + 1, m.as_str())?;
                    }
                    u.set("mime_types", mt)?;
                }
                if let Some(max) = upload.max_file_size {
                    u.set("max_file_size", max)?;
                }
                if !upload.image_sizes.is_empty() {
                    let sizes = lua.create_table()?;
                    for (i, s) in upload.image_sizes.iter().enumerate() {
                        let st = lua.create_table()?;
                        st.set("name", s.name.as_str())?;
                        st.set("width", s.width)?;
                        st.set("height", s.height)?;
                        let fit_str = match s.fit {
                            crate::core::upload::ImageFit::Cover => "cover",
                            crate::core::upload::ImageFit::Contain => "contain",
                            crate::core::upload::ImageFit::Inside => "inside",
                            crate::core::upload::ImageFit::Fill => "fill",
                        };
                        st.set("fit", fit_str)?;
                        sizes.set(i + 1, st)?;
                    }
                    u.set("image_sizes", sizes)?;
                }
                if let Some(ref thumb) = upload.admin_thumbnail {
                    u.set("admin_thumbnail", thumb.as_str())?;
                }
                if upload.format_options.webp.is_some() || upload.format_options.avif.is_some() {
                    let fo = lua.create_table()?;
                    if let Some(ref webp) = upload.format_options.webp {
                        let w = lua.create_table()?;
                        w.set("quality", webp.quality)?;
                        fo.set("webp", w)?;
                    }
                    if let Some(ref avif) = upload.format_options.avif {
                        let a = lua.create_table()?;
                        a.set("quality", avif.quality)?;
                        fo.set("avif", a)?;
                    }
                    u.set("format_options", fo)?;
                }
                tbl.set("upload", u)?;
            }
        }
    }

    // live
    match &def.live {
        None => { tbl.set("live", true)?; }
        Some(crate::core::collection::LiveSetting::Disabled) => { tbl.set("live", false)?; }
        Some(crate::core::collection::LiveSetting::Function(s)) => { tbl.set("live", s.as_str())?; }
    }

    // versions
    if let Some(ref v) = def.versions {
        if v.drafts && v.max_versions == 0 {
            tbl.set("versions", true)?;
        } else {
            let vt = lua.create_table()?;
            vt.set("drafts", v.drafts)?;
            if v.max_versions > 0 {
                vt.set("max_versions", v.max_versions)?;
            }
            tbl.set("versions", vt)?;
        }
    }

    Ok(tbl)
}

/// Convert a GlobalDefinition to a full Lua table compatible with parse_global_definition().
fn global_config_to_lua(lua: &Lua, def: &crate::core::collection::GlobalDefinition) -> mlua::Result<Table> {
    let tbl = lua.create_table()?;

    // labels
    let labels = lua.create_table()?;
    if let Some(ref s) = def.labels.singular {
        labels.set("singular", localized_string_to_lua(lua, s)?)?;
    }
    if let Some(ref s) = def.labels.plural {
        labels.set("plural", localized_string_to_lua(lua, s)?)?;
    }
    tbl.set("labels", labels)?;

    // fields
    let fields_arr = lua.create_table()?;
    for (i, f) in def.fields.iter().enumerate() {
        fields_arr.set(i + 1, field_config_to_lua(lua, f)?)?;
    }
    tbl.set("fields", fields_arr)?;

    // hooks
    tbl.set("hooks", collection_hooks_to_lua(lua, &def.hooks)?)?;

    // access
    let access = lua.create_table()?;
    if let Some(ref s) = def.access.read { access.set("read", s.as_str())?; }
    if let Some(ref s) = def.access.create { access.set("create", s.as_str())?; }
    if let Some(ref s) = def.access.update { access.set("update", s.as_str())?; }
    if let Some(ref s) = def.access.delete { access.set("delete", s.as_str())?; }
    tbl.set("access", access)?;

    // live
    match &def.live {
        None => { tbl.set("live", true)?; }
        Some(crate::core::collection::LiveSetting::Disabled) => { tbl.set("live", false)?; }
        Some(crate::core::collection::LiveSetting::Function(s)) => { tbl.set("live", s.as_str())?; }
    }

    Ok(tbl)
}

/// Convert collection-level hooks to a Lua table.
fn collection_hooks_to_lua(lua: &Lua, hooks: &crate::core::collection::CollectionHooks) -> mlua::Result<Table> {
    let tbl = lua.create_table()?;
    let pairs: &[(&str, &[String])] = &[
        ("before_validate", &hooks.before_validate),
        ("before_change", &hooks.before_change),
        ("after_change", &hooks.after_change),
        ("before_read", &hooks.before_read),
        ("after_read", &hooks.after_read),
        ("before_delete", &hooks.before_delete),
        ("after_delete", &hooks.after_delete),
        ("before_broadcast", &hooks.before_broadcast),
    ];
    for (key, list) in pairs {
        if !list.is_empty() {
            let arr = lua.create_table()?;
            for (i, s) in list.iter().enumerate() {
                arr.set(i + 1, s.as_str())?;
            }
            tbl.set(*key, arr)?;
        }
    }
    Ok(tbl)
}

/// Convert a FieldDefinition to a full Lua table compatible with parse_fields().
fn field_config_to_lua(lua: &Lua, f: &crate::core::field::FieldDefinition) -> mlua::Result<Table> {
    let tbl = lua.create_table()?;
    tbl.set("name", f.name.as_str())?;
    tbl.set("type", f.field_type.as_str())?;

    if f.required { tbl.set("required", true)?; }
    if f.unique { tbl.set("unique", true)?; }
    if f.localized { tbl.set("localized", true)?; }
    if let Some(ref v) = f.validate { tbl.set("validate", v.as_str())?; }

    if let Some(ref dv) = f.default_value {
        match dv {
            serde_json::Value::Bool(b) => { tbl.set("default_value", *b)?; }
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    tbl.set("default_value", i)?;
                } else if let Some(f_val) = n.as_f64() {
                    tbl.set("default_value", f_val)?;
                }
            }
            serde_json::Value::String(s) => { tbl.set("default_value", s.as_str())?; }
            _ => {}
        }
    }

    if let Some(ref pa) = f.picker_appearance {
        tbl.set("picker_appearance", pa.as_str())?;
    }

    // options (select fields)
    if !f.options.is_empty() {
        let opts = lua.create_table()?;
        for (i, opt) in f.options.iter().enumerate() {
            let o = lua.create_table()?;
            o.set("label", localized_string_to_lua(lua, &opt.label)?)?;
            o.set("value", opt.value.as_str())?;
            opts.set(i + 1, o)?;
        }
        tbl.set("options", opts)?;
    }

    // admin
    {
        let admin = lua.create_table()?;
        let mut has_any = false;
        if let Some(ref l) = f.admin.label {
            admin.set("label", localized_string_to_lua(lua, l)?)?;
            has_any = true;
        }
        if let Some(ref p) = f.admin.placeholder {
            admin.set("placeholder", localized_string_to_lua(lua, p)?)?;
            has_any = true;
        }
        if let Some(ref d) = f.admin.description {
            admin.set("description", localized_string_to_lua(lua, d)?)?;
            has_any = true;
        }
        if f.admin.hidden { admin.set("hidden", true)?; has_any = true; }
        if f.admin.readonly { admin.set("readonly", true)?; has_any = true; }
        if let Some(ref w) = f.admin.width {
            admin.set("width", w.as_str())?;
            has_any = true;
        }
        if f.admin.collapsed { admin.set("collapsed", true)?; has_any = true; }
        if has_any {
            tbl.set("admin", admin)?;
        }
    }

    // hooks
    {
        let hooks = lua.create_table()?;
        let mut has_any = false;
        let pairs: &[(&str, &[String])] = &[
            ("before_validate", &f.hooks.before_validate),
            ("before_change", &f.hooks.before_change),
            ("after_change", &f.hooks.after_change),
            ("after_read", &f.hooks.after_read),
        ];
        for (key, list) in pairs {
            if !list.is_empty() {
                let arr = lua.create_table()?;
                for (i, s) in list.iter().enumerate() {
                    arr.set(i + 1, s.as_str())?;
                }
                hooks.set(*key, arr)?;
                has_any = true;
            }
        }
        if has_any {
            tbl.set("hooks", hooks)?;
        }
    }

    // access
    {
        let access = lua.create_table()?;
        let mut has_any = false;
        if let Some(ref s) = f.access.read { access.set("read", s.as_str())?; has_any = true; }
        if let Some(ref s) = f.access.create { access.set("create", s.as_str())?; has_any = true; }
        if let Some(ref s) = f.access.update { access.set("update", s.as_str())?; has_any = true; }
        if has_any {
            tbl.set("access", access)?;
        }
    }

    // relationship
    if let Some(ref rc) = f.relationship {
        let rel = lua.create_table()?;
        rel.set("collection", rc.collection.as_str())?;
        if rc.has_many { rel.set("has_many", true)?; }
        if let Some(md) = rc.max_depth { rel.set("max_depth", md)?; }
        tbl.set("relationship", rel)?;
    }

    // sub-fields (array, group)
    if !f.fields.is_empty() {
        let sub = lua.create_table()?;
        for (i, sf) in f.fields.iter().enumerate() {
            sub.set(i + 1, field_config_to_lua(lua, sf)?)?;
        }
        tbl.set("fields", sub)?;
    }

    // blocks
    if !f.blocks.is_empty() {
        let blocks = lua.create_table()?;
        for (i, b) in f.blocks.iter().enumerate() {
            let bt = lua.create_table()?;
            bt.set("type", b.block_type.as_str())?;
            if let Some(ref lbl) = b.label {
                bt.set("label", localized_string_to_lua(lua, lbl)?)?;
            }
            let bf = lua.create_table()?;
            for (j, sf) in b.fields.iter().enumerate() {
                bf.set(j + 1, field_config_to_lua(lua, sf)?)?;
            }
            bt.set("fields", bf)?;
            blocks.set(i + 1, bt)?;
        }
        tbl.set("blocks", blocks)?;
    }

    Ok(tbl)
}

/// Convert a Lua value to a serde_json::Value.
pub fn lua_to_json(_lua: &Lua, value: &Value) -> mlua::Result<serde_json::Value> {
    match value {
        Value::Nil => Ok(serde_json::Value::Null),
        Value::Boolean(b) => Ok(serde_json::Value::Bool(*b)),
        Value::Integer(i) => Ok(serde_json::Value::Number((*i).into())),
        Value::Number(n) => {
            serde_json::Number::from_f64(*n)
                .map(serde_json::Value::Number)
                .ok_or_else(|| mlua::Error::RuntimeError("Invalid float value".into()))
        }
        Value::String(s) => Ok(serde_json::Value::String(s.to_str()?.to_string())),
        Value::Table(t) => {
            let len = t.raw_len();
            if len > 0 {
                let mut arr = Vec::new();
                for i in 1..=len {
                    let v: Value = t.raw_get(i)?;
                    arr.push(lua_to_json(_lua, &v)?);
                }
                Ok(serde_json::Value::Array(arr))
            } else {
                let mut map = serde_json::Map::new();
                for pair in t.clone().pairs::<String, Value>() {
                    let (k, v) = pair?;
                    map.insert(k, lua_to_json(_lua, &v)?);
                }
                Ok(serde_json::Value::Object(map))
            }
        }
        _ => Ok(serde_json::Value::Null),
    }
}

/// Convert a serde_json::Value to a Lua value.
pub fn json_to_lua(lua: &Lua, value: &serde_json::Value) -> mlua::Result<Value> {
    match value {
        serde_json::Value::Null => Ok(Value::Nil),
        serde_json::Value::Bool(b) => Ok(Value::Boolean(*b)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(Value::Integer(i))
            } else if let Some(f) = n.as_f64() {
                Ok(Value::Number(f))
            } else {
                Ok(Value::Nil)
            }
        }
        serde_json::Value::String(s) => {
            Ok(Value::String(lua.create_string(s)?))
        }
        serde_json::Value::Array(arr) => {
            let tbl = lua.create_table()?;
            for (i, v) in arr.iter().enumerate() {
                tbl.set(i + 1, json_to_lua(lua, v)?)?;
            }
            Ok(Value::Table(tbl))
        }
        serde_json::Value::Object(map) => {
            let tbl = lua.create_table()?;
            for (k, v) in map {
                tbl.set(k.as_str(), json_to_lua(lua, v)?)?;
            }
            Ok(Value::Table(tbl))
        }
    }
}
