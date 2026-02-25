//! `crap.schema` namespace — read-only schema introspection.

use anyhow::Result;
use mlua::{Lua, Table, Value};

use crate::core::SharedRegistry;

/// Register `crap.schema` — read-only collection/global introspection.
pub(super) fn register_schema(lua: &Lua, crap: &Table, registry: SharedRegistry) -> Result<()> {
    let schema_table = lua.create_table()?;

    let reg = registry.clone();
    let get_collection_fn = lua.create_function(move |lua, slug: String| -> mlua::Result<Value> {
        let r = reg.read().map_err(|e| mlua::Error::RuntimeError(
            format!("Registry lock: {}", e)
        ))?;
        match r.get_collection(&slug) {
            Some(def) => Ok(Value::Table(collection_def_to_lua_table(lua, def)?)),
            None => Ok(Value::Nil),
        }
    })?;
    schema_table.set("get_collection", get_collection_fn)?;

    let reg = registry.clone();
    let get_global_fn = lua.create_function(move |lua, slug: String| -> mlua::Result<Value> {
        let r = reg.read().map_err(|e| mlua::Error::RuntimeError(
            format!("Registry lock: {}", e)
        ))?;
        match r.get_global(&slug) {
            Some(def) => {
                let tbl = lua.create_table()?;
                tbl.set("slug", def.slug.as_str())?;
                let labels = lua.create_table()?;
                if let Some(ref s) = def.labels.singular {
                    labels.set("singular", s.resolve_default())?;
                }
                if let Some(ref s) = def.labels.plural {
                    labels.set("plural", s.resolve_default())?;
                }
                tbl.set("labels", labels)?;
                let fields_arr = lua.create_table()?;
                for (i, f) in def.fields.iter().enumerate() {
                    fields_arr.set(i + 1, field_def_to_lua_table(lua, f)?)?;
                }
                tbl.set("fields", fields_arr)?;
                Ok(Value::Table(tbl))
            }
            None => Ok(Value::Nil),
        }
    })?;
    schema_table.set("get_global", get_global_fn)?;

    let reg = registry.clone();
    let list_collections_fn = lua.create_function(move |lua, ()| -> mlua::Result<Table> {
        let r = reg.read().map_err(|e| mlua::Error::RuntimeError(
            format!("Registry lock: {}", e)
        ))?;
        let tbl = lua.create_table()?;
        let mut i = 0;
        for def in r.collections.values() {
            i += 1;
            let item = lua.create_table()?;
            item.set("slug", def.slug.as_str())?;
            let labels = lua.create_table()?;
            if let Some(ref s) = def.labels.singular {
                labels.set("singular", s.resolve_default())?;
            }
            if let Some(ref s) = def.labels.plural {
                labels.set("plural", s.resolve_default())?;
            }
            item.set("labels", labels)?;
            tbl.set(i, item)?;
        }
        Ok(tbl)
    })?;
    schema_table.set("list_collections", list_collections_fn)?;

    let reg = registry.clone();
    let list_globals_fn = lua.create_function(move |lua, ()| -> mlua::Result<Table> {
        let r = reg.read().map_err(|e| mlua::Error::RuntimeError(
            format!("Registry lock: {}", e)
        ))?;
        let tbl = lua.create_table()?;
        let mut i = 0;
        for def in r.globals.values() {
            i += 1;
            let item = lua.create_table()?;
            item.set("slug", def.slug.as_str())?;
            let labels = lua.create_table()?;
            if let Some(ref s) = def.labels.singular {
                labels.set("singular", s.resolve_default())?;
            }
            if let Some(ref s) = def.labels.plural {
                labels.set("plural", s.resolve_default())?;
            }
            item.set("labels", labels)?;
            tbl.set(i, item)?;
        }
        Ok(tbl)
    })?;
    schema_table.set("list_globals", list_globals_fn)?;

    crap.set("schema", schema_table)?;

    Ok(())
}

/// Convert a CollectionDefinition to a Lua table for crap.schema.get_collection().
fn collection_def_to_lua_table(lua: &Lua, def: &crate::core::CollectionDefinition) -> mlua::Result<Table> {
    let tbl = lua.create_table()?;
    tbl.set("slug", def.slug.as_str())?;
    let labels = lua.create_table()?;
    if let Some(ref s) = def.labels.singular {
        labels.set("singular", s.resolve_default())?;
    }
    if let Some(ref s) = def.labels.plural {
        labels.set("plural", s.resolve_default())?;
    }
    tbl.set("labels", labels)?;
    tbl.set("timestamps", def.timestamps)?;
    tbl.set("has_auth", def.is_auth_collection())?;
    tbl.set("has_upload", def.is_upload_collection())?;
    tbl.set("has_versions", def.has_versions())?;
    tbl.set("has_drafts", def.has_drafts())?;

    let fields_arr = lua.create_table()?;
    for (i, f) in def.fields.iter().enumerate() {
        fields_arr.set(i + 1, field_def_to_lua_table(lua, f)?)?;
    }
    tbl.set("fields", fields_arr)?;
    Ok(tbl)
}

/// Convert a FieldDefinition to a Lua table for schema introspection.
fn field_def_to_lua_table(lua: &Lua, f: &crate::core::field::FieldDefinition) -> mlua::Result<Table> {
    let tbl = lua.create_table()?;
    tbl.set("name", f.name.as_str())?;
    tbl.set("type", f.field_type.as_str())?;
    tbl.set("required", f.required)?;
    tbl.set("localized", f.localized)?;
    tbl.set("unique", f.unique)?;

    if let Some(ref rc) = f.relationship {
        let rel = lua.create_table()?;
        rel.set("collection", rc.collection.as_str())?;
        rel.set("has_many", rc.has_many)?;
        if let Some(md) = rc.max_depth {
            rel.set("max_depth", md)?;
        }
        tbl.set("relationship", rel)?;
    }

    if !f.options.is_empty() {
        let opts = lua.create_table()?;
        for (i, opt) in f.options.iter().enumerate() {
            let o = lua.create_table()?;
            o.set("label", opt.label.resolve_default())?;
            o.set("value", opt.value.as_str())?;
            opts.set(i + 1, o)?;
        }
        tbl.set("options", opts)?;
    }

    // Recurse into sub-fields (array, group)
    if !f.fields.is_empty() {
        let sub = lua.create_table()?;
        for (i, sf) in f.fields.iter().enumerate() {
            sub.set(i + 1, field_def_to_lua_table(lua, sf)?)?;
        }
        tbl.set("fields", sub)?;
    }

    // Blocks
    if !f.blocks.is_empty() {
        let blocks = lua.create_table()?;
        for (i, b) in f.blocks.iter().enumerate() {
            let bt = lua.create_table()?;
            bt.set("type", b.block_type.as_str())?;
            if let Some(ref lbl) = b.label {
                bt.set("label", lbl.resolve_default())?;
            }
            let bf = lua.create_table()?;
            for (j, sf) in b.fields.iter().enumerate() {
                bf.set(j + 1, field_def_to_lua_table(lua, sf)?)?;
            }
            bt.set("fields", bf)?;
            blocks.set(i + 1, bt)?;
        }
        tbl.set("blocks", blocks)?;
    }

    Ok(tbl)
}
