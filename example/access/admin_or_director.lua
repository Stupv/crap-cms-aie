---@param context crap.AccessContext
---@return boolean
return function(context)
	if not context.user then
		return false
	end
	return context.user.role == "admin" or context.user.role == "director"
end
