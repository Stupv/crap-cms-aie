--- Access: allow admins or the user themselves.
---@param context crap.AccessContext
---@return boolean
return function(context)
    if context.user == nil then return false end
    if context.user.role == "admin" then return true end
    -- Users can edit their own profile
    return context.id ~= nil and context.id == context.user.id
end
