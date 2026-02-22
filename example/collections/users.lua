-- Example with custom auth strategies (uncomment to enable):
--
-- crap.collections.define("users", {
--     auth = {
--         strategies = {
--             {
--                 name = "api-key",
--                 authenticate = "hooks.auth.api_key_check",
--             },
--         },
--         -- disable_local = true,  -- set to disable password login
--     },
--     ...
-- })

crap.collections.define("users", {
    auth = true,
    labels = {
        singular = "User",
        plural = "Users",
    },
    timestamps = true,
    admin = {
        use_as_title = "name",
        default_sort = "-created_at",
        list_searchable_fields = { "name", "email" },
    },
    fields = {
        -- email is auto-injected when auth = true (if not defined here)
        {
            name = "name",
            type = "text",
            required = true,
            admin = {
                placeholder = "Full name",
            },
        },
        {
            name = "role",
            type = "select",
            required = true,
            default_value = "editor",
            options = {
                { label = "Admin", value = "admin" },
                { label = "Editor", value = "editor" },
            },
        },
    },
})
