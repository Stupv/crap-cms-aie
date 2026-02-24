crap.collections.define("users", {
    auth = true,
    labels = {
        singular = "User",
        plural = "Users",
    },
    timestamps = true,
    admin = {
        use_as_title = "name",
    },
    fields = {
        {
            name = "name",
            type = "text",
            required = true,
        },
        {
            name = "role",
            type = "select",
            default_value = "editor",
            options = {
                { label = "Admin", value = "admin" },
                { label = "Editor", value = "editor" },
            },
        },
    },
})
