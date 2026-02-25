crap.collections.define("users", {
    labels = {
        singular = "User",
        plural = "Users",
    },
    timestamps = true,
    auth = true,
    admin = {
        use_as_title = "email",
        default_sort = "-created_at",
    },
    fields = {
        {
            name = "name",
            type = "text",
            required = true,
            admin = {
                description = "Display name shown on posts",
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
                { label = "Author", value = "author" },
            },
        },
        {
            name = "avatar",
            type = "upload",
            relationship = {
                collection = "media",
            },
            admin = {
                description = "Profile picture",
                width = "half",
            },
        },
        {
            name = "bio",
            type = "textarea",
            admin = {
                description = "Short author biography",
                width = "full",
            },
        },
    },
    access = {
        read = "access.anyone",
        create = "access.admin_only",
        update = "access.self_or_admin",
        delete = "access.admin_only",
    },
})
