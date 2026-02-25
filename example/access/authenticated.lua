--- Access: allow any logged-in user.
---@param context crap.AccessContext
---@return boolean
return function(context)
    return context.user ~= nil
end
