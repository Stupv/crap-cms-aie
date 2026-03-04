--- Field before_validate hook: trim whitespace from the field value.
---@param value any
---@param context crap.FieldHookContext
---@return any
return function(value, context)
  if type(value) == "string" then
    return value:match("^%s*(.-)%s*$")
  end
  return value
end
