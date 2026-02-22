local M = {}

--- Field hook: trim whitespace from title.
---@param value any
---@param context crap.FieldHookContext
---@return any
function M.trim_title(value, context)
    if type(value) == "string" then
        return value:match("^%s*(.-)%s*$")
    end
    return value
end

---@param context crap.hook.Posts
function M.auto_slug(context)
    local data = context.data
    if data.title and (not data.slug or data.slug == "") then
        data.slug = crap.util.slugify(data.title)
        crap.log.info("Auto-generated slug: " .. data.slug)
    end
    return context
end

return M
