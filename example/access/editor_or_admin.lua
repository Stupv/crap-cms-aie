--- Access: allow editors and admins.
---@param context crap.AccessContext
---@return boolean
return function(context)
    if context.user == nil then return false end
    return context.user.role == "admin" or context.user.role == "editor"
end
