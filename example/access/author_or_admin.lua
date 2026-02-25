--- Access: allow the post author or admins to delete.
---@param context crap.AccessContext
---@return boolean
return function(context)
    if context.user == nil then return false end
    if context.user.role == "admin" then return true end
    -- Authors can delete their own posts
    if context.id and context.user.role == "author" then
        local doc = crap.collections.find_by_id("posts", context.id)
        return doc ~= nil and doc.author == context.user.id
    end
    return false
end
