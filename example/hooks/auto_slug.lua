--- Field hook: auto-generate a slug from the title field.
--- Attach to slug fields via: hooks = { before_validate = { "hooks.auto_slug" } }
---@param value string|nil
---@param context crap.FieldHookContext
---@return string
return function(value, context)
    -- If slug is already set, keep it
    if value and value ~= "" then
        return value
    end

    -- Get title from the document data
    local title = context.data and context.data.title
    if not title or title == "" then
        return value
    end

    -- Convert title to URL-safe slug
    local slug = title:lower()
        :gsub("[^%w%s-]", "")  -- remove non-alphanumeric (keep spaces and hyphens)
        :gsub("%s+", "-")      -- spaces to hyphens
        :gsub("-+", "-")       -- collapse multiple hyphens
        :gsub("^-|-$", "")     -- trim leading/trailing hyphens

    return slug
end
