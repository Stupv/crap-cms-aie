--- Display condition for posts (client-evaluated).
---
--- Returns a condition table — evaluated instantly in the browser,
--- no server round-trip. Prefer this over boolean returns when possible.
---
--- Condition table operators:
---   equals, not_equals, in, not_in, is_truthy, is_falsy
---   Array of conditions = AND (all must be true).
---
---@param data table Current form field values.
---@return table
return function(data)
    return { field = "post_type", ["in"] = { "link", "video" } }
end
