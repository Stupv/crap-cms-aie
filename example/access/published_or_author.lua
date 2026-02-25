--- Access: allow reading published content, or drafts by the author/editors/admins.
--- Returns a query constraint for list operations.
---@param context crap.AccessContext
---@return boolean|table
return function(context)
    -- Admins and editors can see everything
    if context.user ~= nil then
        if context.user.role == "admin" or context.user.role == "editor" then
            return true
        end
        -- Authors can see published + their own drafts
        -- TODO: OR filters not yet supported in access constraints,
        -- so for now authors see everything (filtered in application layer)
        return true
    end
    -- Anonymous: only published
    return { _status = "published" }
end
