--- Collection hook: auto-set published_at when a post is first published.
--- Attach via: hooks = { before_change = { "hooks.set_published_at" } }
---@param context crap.HookContext
---@return crap.HookContext
return function(context)
    local data = context.data
    if not data then return context end

    -- Only set published_at if not already set and document is being published
    if not data.published_at or data.published_at == "" then
        if data._status == "published" then
            data.published_at = os.date("!%Y-%m-%dT%H:%M:%SZ")
        end
    end

    return context
end
