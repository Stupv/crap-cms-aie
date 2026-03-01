---@param context crap.AccessContext
---@return boolean
return function(context)
	return context.user ~= nil and context.user.role == "admin"
end
