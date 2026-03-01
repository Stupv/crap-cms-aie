---@param context crap.AccessContext
---@return boolean
return function(context)
	if not context.user then
		return false
	end
	local role = context.user.role
	return role == "admin" or role == "director" or role == "editor"
end
