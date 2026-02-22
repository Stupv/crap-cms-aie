--- Access control functions for collection and field-level permissions.
--- Reference these from collection definitions:
---   access = { read = "hooks.access.public_read", ... }
local M = {}

--- Allow everyone (including anonymous users).
--- @param ctx crap.AccessContext
--- @return boolean
function M.public_read(ctx)
    return true
end

--- Allow only authenticated users.
--- @param ctx crap.AccessContext
--- @return boolean
function M.authenticated(ctx)
    return ctx.user ~= nil
end

--- Allow only users with role "admin".
--- @param ctx crap.AccessContext
--- @return boolean
function M.admin_only(ctx)
    return ctx.user ~= nil and ctx.user.role == "admin"
end

--- Allow admins to see everything; non-admins only see their own documents.
--- Returns a filter table (query constraint) for non-admin users.
--- @param ctx crap.AccessContext
--- @return boolean|table
function M.own_or_admin(ctx)
    if ctx.user == nil then return false end
    if ctx.user.role == "admin" then return true end
    -- Return a Where table — merged into the query as additional AND clauses
    return { created_by = ctx.user.id }
end

return M
