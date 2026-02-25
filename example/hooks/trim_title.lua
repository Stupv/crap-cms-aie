--- Collection hook: trim whitespace from the title field.
--- Attach via: hooks = { before_change = { "hooks.trim_title" } }
---@param context crap.HookContext
---@return crap.HookContext
return function(context)
    if context.data and context.data.title then
        context.data.title = context.data.title:match("^%s*(.-)%s*$")
    end
    return context
end
